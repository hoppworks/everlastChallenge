//! Measurement-only cumulative snapshots. The complete DSP journal stays owned
//! by AudioProcessor; only one granted immutable prefix may exist beside it.
use super::*;
use dictation_core::audio::pauses::ConfirmedPauses;

#[derive(Default)]
pub(super) struct PrefixLink {
    state: Mutex<State>,
}
#[derive(Default)]
struct State {
    slot: Option<pretranscription::SealedPrefix>,
    epoch: Option<u64>,
    permitted: bool,
    last_offered_cut: usize,
    started_delivered: bool,
}
impl PrefixLink {
    pub(super) fn allow(&self, epoch: u64) {
        let mut state = self.state.lock().unwrap_or_else(|p| p.into_inner());
        if state.epoch.is_none_or(|old| epoch > old) {
            state.epoch = Some(epoch);
            state.permitted = true;
        }
    }
    pub(super) fn poll(
        &self,
        session: u64,
        events: &Receiver<CaptureEvent>,
    ) -> Option<CaptureEvent> {
        // Serialize prefix publication and lifecycle dequeue: final full audio
        // cannot overtake a previously admitted prefix. No terminal/PCM queue.
        let mut state = self.state.lock().unwrap_or_else(|p| p.into_inner());
        if !state.started_delivered {
            let event = events.try_recv().ok()?;
            if matches!(event, CaptureEvent::Started { .. }) {
                state.started_delivered = true;
            }
            return Some(event);
        }
        if let Some(prefix) = state.slot.take() {
            return Some(CaptureEvent::Segment { session, prefix });
        }
        events.try_recv().ok()
    }
    pub(super) fn advance(
        &self,
        processor: &AudioProcessor,
        pauses: &mut ConfirmedPauses,
    ) -> Result<(), CaptureError> {
        let (base, samples) = processor.stable_samples();
        if base != 0 {
            return Err(CaptureError::Drain);
        }
        let Some(cut) = pauses.observe(base, samples) else {
            return Ok(());
        };
        let mut state = self.state.lock().map_err(|_| CaptureError::Worker)?;
        if !state.permitted || cut <= state.last_offered_cut {
            return Ok(());
        }
        if state.slot.is_some() || cut > samples.len() {
            return Err(CaptureError::EventOverflow);
        }
        // Consume before allocating: repeated actor polls cannot authorize a
        // third PCM allocation while this snapshot is being copied/submitted.
        state.permitted = false;
        state.last_offered_cut = cut;
        drop(state);
        let mut prefix = Vec::new();
        prefix
            .try_reserve_exact(cut)
            .map_err(|_| CaptureError::Audio(AudioError::Allocation))?;
        prefix.extend_from_slice(&samples[..cut]);
        self.state.lock().map_err(|_| CaptureError::Worker)?.slot =
            Some(pretranscription::SealedPrefix {
                timeline_range: 0..cut,
                samples: Arc::new(prefix),
            });
        Ok(())
    }
}
pub(super) fn record(
    owner: RecordingOwner<'_>,
    format: AudioFormat,
    ready: Sender<replay::ReplayInput>,
    link: &PrefixLink,
) -> Result<Vec<f32>, CaptureError> {
    replay::record_inner(
        owner,
        format,
        ready,
        Some(RecordingSegments::FullPrefix(link)),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn snapshot_handoff_retains_no_hidden_arc_and_stop_returns_the_full_journal()
    -> Result<(), Box<dyn std::error::Error>> {
        let (ready, receive) = crossbeam_channel::bounded(1);
        let handle = CaptureHandle::start_full_prefix_replay(
            1,
            AudioFormat {
                sample_rate: 16000,
                channels: 1,
            },
            ready,
        )?;
        let mut input = receive.recv_timeout(Duration::from_secs(2))?;
        assert!(matches!(
            handle.next_event(),
            Some(CaptureEvent::Started { .. })
        ));
        handle.allow_seal(0);
        let mut source = vec![0.1; 16000];
        source.resize(24000, 0.0);
        for chunk in source.chunks(320) {
            input.port.publish(chunk);
            thread::sleep(Duration::from_millis(1));
        }
        let deadline = Instant::now() + Duration::from_secs(2);
        let prefix = loop {
            if let Some(CaptureEvent::Segment { prefix, .. }) = handle.next_event() {
                break prefix;
            }
            if Instant::now() >= deadline {
                return Err("prefix not published".into());
            }
            thread::sleep(Duration::from_millis(1));
        };
        let weak = Arc::downgrade(&prefix.samples);
        assert_eq!(prefix.timeline_range, 0..19200);
        assert_eq!(
            Arc::strong_count(&prefix.samples),
            1,
            "slot/owner retain no hidden frozen allocation after transfer"
        );
        input.port.publish(&[0.3125; 137]);
        source.extend_from_slice(&[0.3125; 137]);
        handle.request_stop();
        let whole = loop {
            if let Some(CaptureEvent::Finished { result, .. }) = handle.next_event() {
                break result?;
            }
            if Instant::now() >= deadline {
                return Err("full journal not released".into());
            }
            thread::sleep(Duration::from_millis(1));
        };
        handle.join()?;
        assert_eq!(whole, source);
        assert_eq!(&prefix.samples[..], &source[..19200]);
        assert_eq!(Arc::strong_count(&prefix.samples), 1);
        drop(prefix);
        assert!(
            weak.upgrade().is_none(),
            "no capture owner keeps a retired snapshot alive"
        );
        Ok(())
    }
}
