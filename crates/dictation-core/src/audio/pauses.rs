//! Confirmed-pause descriptors on stable 16-kHz output; no PCM ownership.
use super::EnergyProfile;

#[derive(Default)]
pub struct ConfirmedPauses {
    consumed: usize,
    window_samples: usize,
    square_sum: f64,
    voice: bool,
    voice_end: Option<usize>,
    safe_cut: Option<usize>,
}
impl ConfirmedPauses {
    /// Each stable global sample is consumed once, including across journal
    /// seals. Partial 20-ms windows remain pending until more audio arrives.
    pub fn observe(&mut self, base: usize, stable: &[f32]) -> Option<usize> {
        let profile = EnergyProfile::default();
        for sample in stable.iter().skip(self.consumed.saturating_sub(base)) {
            self.square_sum += f64::from(*sample).powi(2);
            self.window_samples += 1;
            self.consumed += 1;
            if self.window_samples == 320 {
                let rms = (self.square_sum / 320.0).sqrt() as f32;
                if rms >= profile.start_rms || (self.voice && rms >= profile.end_rms) {
                    self.voice = true;
                    self.voice_end = Some(self.consumed);
                } else {
                    self.voice = false;
                }
                self.square_sum = 0.0;
                self.window_samples = 0;
            }
            if let Some(end) = self.voice_end
                && self.consumed >= end + 8000
            {
                self.safe_cut = Some(end + 3200);
            }
        }
        self.safe_cut
    }
}
