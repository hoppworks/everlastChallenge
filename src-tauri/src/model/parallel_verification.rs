//! Full-byte verification against compiled, position-bound SHA-256 chunk pins.
//! Pins are generated only after the original whole-file SHA-256 matches. They
//! are not learned from the local cache. Unknown models retain sequential SHA.
use super::{ModelDefinition, ModelError};
use sha2::{Digest, Sha256};
use std::{
    fs::File,
    io,
    os::unix::fs::FileExt,
    sync::atomic::{AtomicUsize, Ordering},
};
use tokio_util::sync::CancellationToken;
mod pins;

struct Manifest {
    bytes: u64,
    sha256: &'static str,
    chunk_bytes: u64,
    digests: &'static [[u8; 32]],
}

fn manifest(definition: ModelDefinition) -> Option<&'static Manifest> {
    static MANIFESTS: [Manifest; 2] = [pins::REFERENCE, pins::COEFFICIENTS];
    #[cfg(test)]
    if definition.id() == "parallel-integrity-fixture"
        && definition.bytes() == 17
        && definition.sha256() == "918a954ac4dfb54ac39f068d9868227f69ab39bc362e2c9b0083bf6a109d6ad7"
    {
        return Some(&FIXTURE);
    }
    MANIFESTS
        .iter()
        .find(|pin| definition.bytes() == pin.bytes && definition.sha256() == pin.sha256)
}

#[cfg(test)]
const FIXTURE: Manifest = Manifest {
    bytes: 17,
    sha256: "918a954ac4dfb54ac39f068d9868227f69ab39bc362e2c9b0083bf6a109d6ad7",
    chunk_bytes: 5,
    digests: &[
        [
            54, 187, 229, 14, 217, 104, 65, 209, 4, 67, 188, 182, 112, 214, 85, 79, 10, 52, 183,
            97, 190, 103, 236, 156, 74, 138, 210, 192, 196, 76, 164, 44,
        ],
        [
            114, 114, 32, 126, 75, 156, 126, 146, 159, 66, 120, 210, 69, 199, 118, 131, 128, 181,
            78, 203, 151, 177, 251, 233, 65, 167, 204, 252, 39, 207, 1, 244,
        ],
        [
            32, 145, 184, 246, 158, 7, 18, 161, 206, 72, 49, 172, 106, 113, 161, 148, 140, 199, 89,
            72, 226, 75, 150, 101, 80, 45, 208, 48, 222, 239, 199, 255,
        ],
        [
            206, 78, 154, 13, 173, 30, 237, 141, 105, 199, 19, 105, 42, 116, 158, 107, 108, 72,
            111, 121, 21, 110, 202, 60, 46, 154, 230, 219, 34, 182, 51, 230,
        ],
    ],
};

pub(super) fn verify_if_pinned(
    file: &File,
    definition: ModelDefinition,
    cancel: Option<&CancellationToken>,
) -> Option<Result<(), ModelError>> {
    manifest(definition).map(|manifest| verify(file, manifest, cancel))
}

fn verify(
    file: &File,
    manifest: &Manifest,
    cancel: Option<&CancellationToken>,
) -> Result<(), ModelError> {
    if file.metadata()?.len() != manifest.bytes {
        return Err(ModelError::Size);
    }
    let next = AtomicUsize::new(0);
    let workers = std::thread::available_parallelism()
        .map_or(1, usize::from)
        .min(8)
        .min(manifest.digests.len());
    std::thread::scope(|scope| -> Result<(), ModelError> {
        let mut handles = Vec::with_capacity(workers);
        for _ in 0..workers {
            let next = &next;
            handles.push(
                std::thread::Builder::new()
                    .name("model-integrity".into())
                    .spawn_scoped(scope, move || {
                        let mut buffer = [0_u8; 65_536];
                        loop {
                            if cancel.is_some_and(CancellationToken::is_cancelled) {
                                return Err(ModelError::Cancelled);
                            }
                            let index = next.fetch_add(1, Ordering::Relaxed);
                            let Some(expected) = manifest.digests.get(index) else {
                                return Ok(());
                            };
                            let start = index as u64 * manifest.chunk_bytes;
                            let end = (start + manifest.chunk_bytes).min(manifest.bytes);
                            let mut offset = start;
                            let mut hasher = Sha256::new();
                            while offset < end {
                                if cancel.is_some_and(CancellationToken::is_cancelled) {
                                    return Err(ModelError::Cancelled);
                                }
                                let take = buffer.len().min((end - offset) as usize);
                                let read = file.read_at(&mut buffer[..take], offset)?;
                                if read == 0 {
                                    return Err(ModelError::Size);
                                }
                                hasher.update(&buffer[..read]);
                                offset += read as u64;
                            }
                            let actual: [u8; 32] = hasher.finalize().into();
                            if actual != *expected {
                                return Err(ModelError::Hash);
                            }
                        }
                    })?,
            );
        }
        // Join every worker before returning; no integrity task outlives load.
        let mut error = None;
        for handle in handles {
            let result = handle.join().unwrap_or_else(|_| {
                Err(ModelError::Io(io::Error::other(
                    "model integrity worker panicked",
                )))
            });
            if let Err(failure) = result {
                error.get_or_insert(failure);
            }
        }
        error.map_or(Ok(()), Err)
    })?;
    if cancel.is_some_and(CancellationToken::is_cancelled) {
        return Err(ModelError::Cancelled);
    }
    if file.metadata()?.len() != manifest.bytes {
        return Err(ModelError::Size);
    }
    Ok(())
}
