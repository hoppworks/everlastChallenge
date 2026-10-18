//! One frozen prefix slot, independently bounded from lifecycle events.
use super::*;
use dictation_core::audio::pauses::ConfirmedPauses;

pub use super::SealedPrefix;
#[derive(Default)]
pub(super) struct SegmentLink {
    state: Mutex<State>,
    base: AtomicUsize,
    #[cfg(test)]
    poll_barrier: Mutex<Option<PollBarrier>>,
}
#[cfg(test)]
struct PollBarrier {
    reached: Sender<()>,
    release: Receiver<()>,
}
#[derive(Default)]
struct State {
    slot: Option<SealedPrefix>,
    permitted: bool,
    started_delivered: bool,
    epoch: Option<u64>,
}
impl SegmentLink {
    #[cfg(test)]
    pub(super) fn pause_poll(&self) -> bool {
        let barrier = self
            .poll_barrier
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .take();
        barrier.is_none_or(|barrier| {
            barrier.reached.send(()).is_ok()
                && barrier.release.recv_timeout(Duration::from_secs(3)).is_ok()
        })
    }
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
        // One coherent observation: owner prefix publication cannot fall between
        // an empty slot check and dequeue of its later lifecycle completion.
        let mut state = self.state.lock().unwrap_or_else(|p| p.into_inner());
        let event = if !state.started_delivered {
            // Runtime grants only after it adopts Started. Preserve that same
            // transport order even for a low-level caller granting earlier.
            let event = events.try_recv().ok()?;
            if matches!(event, CaptureEvent::Started { .. }) {
                state.started_delivered = true;
            }
            event
        } else {
            if let Some(prefix) = state.slot.take() {
                return Some(CaptureEvent::Segment { session, prefix });
            }
            events.try_recv().ok()?
        };
        Some(match event {
            CaptureEvent::Finished {
                session,
                result,
                maximum,
            } => CaptureEvent::SegmentedFinished {
                session,
                base: self.base(),
                result,
                maximum,
            },
            other => other,
        })
    }
    pub(super) fn base(&self) -> usize {
        self.base.load(Ordering::Acquire)
    }
    pub(super) fn advance(
        &self,
        processor: &mut AudioProcessor,
        pauses: &mut ConfirmedPauses,
    ) -> Result<(), CaptureError> {
        let (base, samples) = processor.stable_samples();
        let Some(cut) = pauses.observe(base, samples).filter(|cut| *cut > base) else {
            return Ok(());
        };
        let mut state = self.state.lock().map_err(|_| CaptureError::Worker)?;
        if !state.permitted {
            return Ok(());
        }
        state.permitted = false;
        if state.slot.is_some() {
            return Err(CaptureError::EventOverflow);
        }
        // Admission is consumed before allocating/copying the suffix. Neither
        // the controller nor a repeated grant waits on that DSP work.
        drop(state);
        let samples = processor.seal_prefix(cut)?;
        let mut state = self.state.lock().map_err(|_| CaptureError::Worker)?;
        state.slot = Some(SealedPrefix {
            timeline_range: base..cut,
            samples: Arc::new(samples),
        });
        self.base.store(cut, Ordering::Release);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn first_poll_preserves_started_before_an_already_published_prefix_and_finish()
    -> Result<(), Box<dyn std::error::Error>> {
        let (ready, input) = crossbeam_channel::bounded(1);
        let handle = CaptureHandle::start_pretranscription_replay(
            1,
            AudioFormat {
                sample_rate: 16000,
                channels: 1,
            },
            ready,
        )?;
        let mut input = input.recv_timeout(Duration::from_secs(2))?;
        // Optional low-level transport strengthening: actual Runtime never
        // grants until Recording, after it has consumed Started.
        handle.allow_seal(0);
        let mut source = vec![0.1; 16000];
        source.resize(24000, 0.0);
        for chunk in source.chunks(320) {
            input.port.publish(chunk);
            thread::sleep(Duration::from_millis(1));
        }
        let link = handle.segmentation.as_ref().ok_or("missing segmentation")?;
        let deadline = Instant::now() + Duration::from_secs(2);
        while link.base() != 19200 {
            if Instant::now() >= deadline {
                return Err("prefix unavailable before first poll".into());
            }
            thread::sleep(Duration::from_millis(1));
        }
        handle.request_stop();
        while !handle.is_finished() {
            if Instant::now() >= deadline {
                return Err("finish unavailable before first poll".into());
            }
            thread::sleep(Duration::from_millis(1));
        }
        let first = handle.next_event();
        let second = handle.next_event();
        let third = handle.next_event();
        assert!(handle.next_event().is_none());
        handle.join()?;
        assert!(
            matches!(first, Some(CaptureEvent::Started { session: 1, .. })),
            "Started must precede pretranscription even after a delayed first poll"
        );
        let Some(CaptureEvent::Segment { prefix, .. }) = second else {
            return Err("missing ordered prefix".into());
        };
        let Some(CaptureEvent::SegmentedFinished { base, result, .. }) = third else {
            return Err("missing ordered finish".into());
        };
        let mut whole = prefix.samples[..prefix.timeline_range.len()].to_vec();
        assert_eq!(base, prefix.timeline_range.end);
        whole.extend(result?);
        assert_eq!(whole, source);
        Ok(())
    }

    #[test]
    fn finished_cannot_overtake_prefix_published_during_an_empty_poll()
    -> Result<(), Box<dyn std::error::Error>> {
        let (ready, input) = crossbeam_channel::bounded(1);
        let handle = Arc::new(CaptureHandle::start_pretranscription_replay(
            1,
            AudioFormat {
                sample_rate: 16000,
                channels: 1,
            },
            ready,
        )?);
        let mut input = input.recv_timeout(Duration::from_secs(2))?;
        assert!(matches!(
            handle.next_event(),
            Some(CaptureEvent::Started { .. })
        ));
        handle.allow_seal(0);
        let link = handle.segmentation.as_ref().ok_or("missing segmentation")?;
        let (reached, paused) = crossbeam_channel::bounded(1);
        let (release, released) = crossbeam_channel::bounded(1);
        *link
            .poll_barrier
            .lock()
            .map_err(|_| "poll barrier poisoned")? = Some(PollBarrier {
            reached,
            release: released,
        });
        let consumer = Arc::clone(&handle);
        let poller = thread::spawn(move || consumer.next_event());
        paused.recv_timeout(Duration::from_secs(2))?;
        let mut source = vec![0.1; 16000];
        source.resize(24000, 0.0);
        for chunk in source.chunks(320) {
            input.port.publish(chunk);
            thread::sleep(Duration::from_millis(1));
        }
        let deadline = Instant::now() + Duration::from_secs(2);
        while link.base() != 19200 {
            if Instant::now() >= deadline {
                return Err("owner did not publish prefix".into());
            }
            thread::sleep(Duration::from_millis(1));
        }
        handle.request_stop();
        while !handle.is_finished() {
            if Instant::now() >= deadline {
                return Err("owner did not finish".into());
            }
            thread::sleep(Duration::from_millis(1));
        }
        release.send(())?;
        let first = poller.join().map_err(|_| "poll thread panicked")?;
        let second = handle.next_event();
        let handle = Arc::try_unwrap(handle).map_err(|_| "capture consumer retained handle")?;
        handle.join()?;
        assert!(
            matches!(first, Some(CaptureEvent::Segment { .. })),
            "a prefix committed in the old empty-check/dequeue gap must precede Finished"
        );
        assert!(matches!(
            second,
            Some(CaptureEvent::SegmentedFinished {
                base: 19200,
                result: Ok(_),
                ..
            })
        ));
        Ok(())
    }

    #[test]
    fn repeated_grants_cannot_reseal_while_prefix_is_between_slot_and_decoder_admission()
    -> Result<(), Box<dyn std::error::Error>> {
        let (ready, input) = crossbeam_channel::bounded(1);
        let handle = CaptureHandle::start_pretranscription_replay(
            1,
            AudioFormat {
                sample_rate: 16000,
                channels: 1,
            },
            ready,
        )?;
        let mut input = input.recv_timeout(Duration::from_secs(2))?;
        assert!(matches!(
            handle.next_event(),
            Some(CaptureEvent::Started { .. })
        ));
        handle.allow_seal(0);
        let mut original = vec![0.1; 16000];
        original.resize(24000, 0.0);
        for chunk in original.chunks(320) {
            input.port.publish(chunk);
            thread::sleep(Duration::from_millis(1));
        }
        let deadline = Instant::now() + Duration::from_secs(2);
        let frozen = loop {
            if let Some(CaptureEvent::Segment { prefix, .. }) = handle.next_event() {
                break prefix;
            }
            if Instant::now() >= deadline {
                return Err("prefix unavailable".into());
            }
            thread::sleep(Duration::from_millis(1));
        };
        let weak = Arc::downgrade(&frozen.samples);
        assert_eq!(
            Arc::strong_count(&frozen.samples),
            1,
            "slot/owner retain no hidden frozen owner after transfer"
        );
        let mut backlog = vec![0.2; 8000];
        backlog.resize(48000, 0.0);
        for chunk in backlog.chunks(320) {
            handle.allow_seal(0); // repeated actor polls are not new completions
            input.port.publish(chunk);
            thread::sleep(Duration::from_millis(1));
        }
        original.extend(backlog);
        assert!(
            !handle.is_finished(),
            "duplicate permit must not cause slot overflow or owner failure"
        );
        assert!(
            handle.next_event().is_none(),
            "prefix held before admission still owns the only frozen budget"
        );
        handle.request_stop();
        let deadline = Instant::now() + Duration::from_secs(2);
        let (base, rest) = loop {
            if let Some(CaptureEvent::SegmentedFinished { base, result, .. }) = handle.next_event()
            {
                break (base, result?);
            }
            if Instant::now() >= deadline {
                return Err("finish unavailable".into());
            }
            thread::sleep(Duration::from_millis(1));
        };
        handle.join()?;
        assert_eq!(base, 19200);
        assert_eq!(&frozen.samples[..base], &original[..base]);
        assert_eq!(rest, original[base..]);
        drop(frozen);
        assert!(
            weak.upgrade().is_none(),
            "all prefix storage is reclaimed after sole consumer release"
        );
        Ok(())
    }
}
