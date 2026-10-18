//! Whole-recording admission only: normalized scratch never becomes ASR audio.
use super::InferenceError;
use crate::model::{ModelError, VAD};
use dictation_core::audio::MAX_SAMPLES;
use sha2::{Digest, Sha256};
use std::{
    fs::{self, DirBuilder, File, OpenOptions},
    io::{Read, Write},
    os::unix::fs::{DirBuilderExt, OpenOptionsExt},
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};
use whisper_rs::{WhisperVadContext, WhisperVadContextParams};

const WINDOW: usize = 512;
const PEAK: f64 = 0.95;
const THRESHOLD: f32 = 0.5;
static SNAPSHOT_NONCE: AtomicU64 = AtomicU64::new(1);

// Native I/O boundary. Tests control probabilities while exercising the real
// normalization, validation and transcription admission path.
pub(super) trait Detector {
    fn probabilities(&mut self, samples: &[f32]) -> Result<&[f32], InferenceError>;
}
impl Detector for WhisperVadContext {
    fn probabilities(&mut self, samples: &[f32]) -> Result<&[f32], InferenceError> {
        self.detect_speech(samples)
            .map_err(|_| InferenceError::Vad)?;
        Ok(WhisperVadContext::probabilities(self))
    }
}

pub(super) struct SpeechGate<D = WhisperVadContext> {
    detector: D,
    // Retained, bounded high-water allocation: at most 4,800,000 f32 samples
    // (19,200,000 bytes requested capacity), no additional staging copy or disk audio.
    scratch: Vec<f32>,
}

impl SpeechGate {
    pub(super) fn load(path: &Path) -> Result<Self, InferenceError> {
        let snapshot = PrivateSnapshot::new(path, VAD.bytes(), VAD.sha256())?;
        let mut params = WhisperVadContextParams::new();
        params.set_n_threads(1);
        params.set_use_gpu(false);
        let detector = WhisperVadContext::new(
            snapshot.path.to_str().ok_or(InferenceError::ModelPath)?,
            params,
        )
        .map_err(|_| InferenceError::Vad)?;
        // The native initializer has consumed the snapshot; only warm weights
        // remain. RAII also cleans this path on initializer failure.
        drop(snapshot);
        Ok(Self::from_detector(detector))
    }
}

impl<D: Detector> SpeechGate<D> {
    pub(super) fn from_detector(detector: D) -> Self {
        Self {
            detector,
            scratch: Vec::new(),
        }
    }

    #[cfg(test)]
    pub(super) fn allows(&mut self, samples: &[f32]) -> Result<bool, InferenceError> {
        self.allows_observed(samples, &mut super::NoObservation)
    }
    pub(super) fn allows_observed(
        &mut self,
        samples: &[f32],
        observation: &mut impl super::ObservationSink,
    ) -> Result<bool, InferenceError> {
        observation.phase_start(super::Phase::VadNormalization);
        let normalized = (|| {
            if samples.is_empty()
                || samples.len() > MAX_SAMPLES
                || samples.iter().any(|v| !v.is_finite())
            {
                return Err(InferenceError::InvalidAudio);
            }
            let peak = samples
                .iter()
                .map(|v| f64::from(v.abs()))
                .fold(0.0, f64::max);
            let gain = if peak > 0.0 { PEAK / peak } else { 1.0 };
            self.scratch.clear();
            self.scratch
                .try_reserve_exact(samples.len())
                .map_err(|_| InferenceError::Memory)?;
            self.scratch
                .extend(samples.iter().map(|v| (f64::from(*v) * gain) as f32));
            Ok(())
        })();
        observation.phase_end(super::Phase::VadNormalization);
        normalized?;
        observation.phase_start(super::Phase::VadInference);
        let detected = (|| {
            let probabilities = self.detector.probabilities(&self.scratch)?;
            if probabilities.len() != samples.len().div_ceil(WINDOW)
                || probabilities
                    .iter()
                    .any(|p| !p.is_finite() || !(0.0..=1.0).contains(p))
            {
                return Err(InferenceError::Vad);
            }
            Ok(probabilities.iter().any(|p| *p >= THRESHOLD))
        })();
        observation.phase_end(super::Phase::VadInference);
        detected
    }
}

struct PrivateSnapshot {
    directory: PathBuf,
    path: PathBuf,
}

impl PrivateSnapshot {
    fn new(source: &Path, expected_bytes: u64, expected_sha: &str) -> Result<Self, InferenceError> {
        Self::copy(source, expected_bytes, expected_sha, &std::env::temp_dir())
            .map_err(InferenceError::VadModel)
    }

    fn copy(
        source: &Path,
        expected_bytes: u64,
        expected_sha: &str,
        temporary_root: &Path,
    ) -> Result<Self, ModelError> {
        let mut original = File::open(source)?;
        let metadata = original.metadata()?;
        if !metadata.is_file() || metadata.len() != expected_bytes {
            return Err(ModelError::Size);
        }
        let mut owned = None;
        for _ in 0..8 {
            let nonce = SNAPSHOT_NONCE.fetch_add(1, Ordering::Relaxed);
            let directory =
                temporary_root.join(format!("everlast-vad-{}-{nonce}", std::process::id()));
            match DirBuilder::new().mode(0o700).create(&directory) {
                Ok(()) => {
                    let path = directory.join("model.bin");
                    owned = Some(Self { directory, path });
                    break;
                }
                Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(e) => return Err(e.into()),
            }
        }
        let owned = owned.ok_or_else(|| {
            std::io::Error::new(
                std::io::ErrorKind::AlreadyExists,
                "VAD snapshot name collisions",
            )
        })?;
        let mut output = OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(&owned.path)?;
        let mut hash = Sha256::new();
        let mut count = 0u64;
        // Only this fixed 64-KiB transfer buffer, no model-sized heap copy.
        let mut buffer = [0u8; 65_536];
        loop {
            let read = original.read(&mut buffer)?;
            if read == 0 {
                break;
            }
            count += read as u64;
            if count > expected_bytes {
                return Err(ModelError::Size);
            }
            output.write_all(&buffer[..read])?;
            hash.update(&buffer[..read]);
        }
        if count != expected_bytes {
            return Err(ModelError::Size);
        }
        if format!("{:x}", hash.finalize()) != expected_sha {
            return Err(ModelError::Hash);
        }
        output.flush()?;
        output.sync_all()?;
        drop(output);
        Ok(owned)
    }
}

impl Drop for PrivateSnapshot {
    fn drop(&mut self) {
        for result in [fs::remove_file(&self.path), fs::remove_dir(&self.directory)] {
            if let Err(error) = result
                && error.kind() != std::io::ErrorKind::NotFound
            {
                eprintln!("vad-model-snapshot-cleanup-failed");
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt;

    #[test]
    fn warm_gate_reuse_does_not_leak_longer_audio_into_zero_or_shorter_jobs()
    -> Result<(), InferenceError> {
        struct Scripted<'a> {
            outputs: [&'static [f32]; 3],
            next: usize,
            seen: &'a mut Vec<Vec<f32>>,
        }
        impl Detector for Scripted<'_> {
            fn probabilities(&mut self, samples: &[f32]) -> Result<&[f32], InferenceError> {
                self.seen.push(samples.to_vec());
                let result = self.outputs[self.next];
                self.next += 1;
                Ok(result)
            }
        }
        let mut observed = Vec::new();
        let mut gate = SpeechGate::from_detector(Scripted {
            outputs: [&[0.8, 0.8], &[0.0], &[0.5]],
            next: 0,
            seen: &mut observed,
        });
        assert!(gate.allows(&[2.0; 513])?);
        assert!(!gate.allows(&[0.0])?);
        assert!(gate.allows(&[-0.0001; 512])?);
        assert_eq!(
            observed.iter().map(Vec::len).collect::<Vec<_>>(),
            [513, 1, 512]
        );
        assert!(observed[0].iter().all(|v| *v == 0.95));
        assert_eq!(observed[1], [0.0]);
        assert!(observed[2].iter().all(|v| *v == -0.95));
        Ok(())
    }

    #[test]
    fn verified_snapshot_is_private_independent_of_source_and_removed_on_drop()
    -> Result<(), Box<dyn std::error::Error>> {
        let root = tempfile::tempdir()?;
        let source = root.path().join("original.bin");
        let bytes = b"verified model fixture";
        fs::write(&source, bytes)?;
        let hash = format!("{:x}", Sha256::digest(bytes));
        let snapshot = PrivateSnapshot::copy(&source, bytes.len() as u64, &hash, root.path())?;
        assert_eq!(
            fs::metadata(&snapshot.directory)?.permissions().mode() & 0o777,
            0o700
        );
        assert_eq!(
            fs::metadata(&snapshot.path)?.permissions().mode() & 0o777,
            0o600
        );
        fs::write(&source, b"source replaced after verification")?;
        assert_eq!(fs::read(&snapshot.path)?, bytes);
        let directory = snapshot.directory.clone();
        drop(snapshot);
        assert!(!directory.exists());
        assert_eq!(fs::read(&source)?, b"source replaced after verification");
        Ok(())
    }

    #[test]
    fn invalid_model_copy_is_rejected_and_owned_paths_are_cleaned()
    -> Result<(), Box<dyn std::error::Error>> {
        let root = tempfile::tempdir()?;
        let source = root.path().join("original.bin");
        fs::write(&source, b"bad model")?;
        assert!(matches!(
            PrivateSnapshot::copy(&source, 9, &"0".repeat(64), root.path()),
            Err(ModelError::Hash)
        ));
        assert!(matches!(
            PrivateSnapshot::copy(&source, 10, &"0".repeat(64), root.path()),
            Err(ModelError::Size)
        ));
        assert_eq!(fs::read_dir(root.path())?.count(), 1);
        assert_eq!(fs::read(&source)?, b"bad model");
        Ok(())
    }
}
