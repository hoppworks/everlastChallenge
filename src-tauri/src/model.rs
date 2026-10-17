//! Known model manifests and verified, bounded, atomic model installation.

use sha2::{Digest, Sha256};
use std::{
    fs::File,
    io::Read,
    path::{Path, PathBuf},
};

mod storage;
use storage::VerifiedFile;
pub const MODEL_REVISION: &str = "5359861c739e955e79d9a303bcbc70fb988958b1";

fn redirect_allowed(url: &reqwest::Url, previous_hops: usize) -> bool {
    previous_hops <= 5
        && url.scheme() == "https"
        && url.username().is_empty()
        && url.password().is_none()
        && url.port_or_known_default() == Some(443)
        && matches!(url.host_str(), Some("huggingface.co" | "us.aws.cdn.hf.co"))
}

#[derive(Debug, Clone, Copy, serde::Serialize)]
pub struct ModelDefinition {
    id: &'static str,
    file: &'static str,
    bytes: u64,
    sha256: &'static str,
}

pub const REFERENCE: ModelDefinition = ModelDefinition {
    id: "large-v3-turbo-q5_0",
    file: "ggml-large-v3-turbo-q5_0.bin",
    bytes: 574_041_195,
    sha256: "394221709cd5ad1f40c46e6031ca61bce88931e6e088c188294c6d5a55ffa7e2",
};
pub const SMALL: ModelDefinition = ModelDefinition {
    id: "small-q5_1",
    file: "ggml-small-q5_1.bin",
    bytes: 190_085_487,
    sha256: "ae85e4a935d7a567bd102fe55afc16bb595bdb618e11b2fc7591bc08120411bb",
};

impl ModelDefinition {
    pub fn id(self) -> &'static str {
        self.id
    }
    pub fn file_name(self) -> &'static str {
        self.file
    }
    pub fn bytes(self) -> u64 {
        self.bytes
    }
    pub fn sha256(self) -> &'static str {
        self.sha256
    }
    pub fn url(self) -> String {
        format!(
            "https://huggingface.co/ggerganov/whisper.cpp/resolve/{MODEL_REVISION}/{}",
            self.file
        )
    }
}

#[derive(Debug, thiserror::Error)]
pub enum ModelError {
    #[error("Model file I/O failed: {0}")]
    Io(#[from] std::io::Error),
    #[error("Model download has an unexpected byte count")]
    Size,
    #[error("Model checksum does not match the publisher manifest")]
    Hash,
    #[error("Model output file is closed")]
    Closed,
    #[error("Model download cancelled")]
    Cancelled,
    #[error("Model download exceeded its connection, inactivity or overall timeout")]
    Timeout,
    #[error("Model network or TLS request failed")]
    Network,
    #[error("Model source redirected outside the verified HTTPS publisher/CDN policy")]
    Redirect,
    #[error("Model source returned HTTP {0}")]
    Http(u16),
}

fn network_error(error: reqwest::Error) -> ModelError {
    if error.is_timeout() {
        ModelError::Timeout
    } else if error.is_redirect() {
        ModelError::Redirect
    } else {
        ModelError::Network
    }
}

/// Run only on a dedicated model I/O worker, never on the GUI/controller.
/// A completed file is a verified cache entry; readiness additionally requires
/// successfully loading it into the serial inference worker.
pub fn download_model(
    directory: &Path,
    definition: ModelDefinition,
    cancel: &tokio_util::sync::CancellationToken,
    mut progress: impl FnMut(u64),
) -> Result<PathBuf, ModelError> {
    if cancel.is_cancelled() {
        return Err(ModelError::Cancelled);
    }
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()?;
    let client = download_client_builder().build().map_err(network_error)?;
    runtime.block_on(transfer(
        directory,
        definition,
        cancel,
        &mut progress,
        &client,
        &definition.url(),
    ))
}

fn download_client_builder() -> reqwest::ClientBuilder {
    reqwest::Client::builder()
        .https_only(true)
        .connect_timeout(std::time::Duration::from_secs(15))
        .read_timeout(std::time::Duration::from_secs(60))
        .timeout(std::time::Duration::from_secs(1_800))
        .redirect(reqwest::redirect::Policy::custom(|attempt| {
            if redirect_allowed(attempt.url(), attempt.previous().len()) {
                attempt.follow()
            } else {
                attempt.error("model redirect policy")
            }
        }))
}

// The product caller always supplies the fixed manifest URL and HTTPS-only
// client above. Local transport fixtures exercise this same response/body path.
async fn transfer(
    directory: &Path,
    definition: ModelDefinition,
    cancel: &tokio_util::sync::CancellationToken,
    progress: &mut impl FnMut(u64),
    client: &reqwest::Client,
    source: &str,
) -> Result<PathBuf, ModelError> {
    let mut response = tokio::select! {
        _=cancel.cancelled()=>return Err(ModelError::Cancelled),
        response=client.get(source).send()=>response.map_err(network_error)?,
    };
    if !response.status().is_success() {
        return Err(ModelError::Http(response.status().as_u16()));
    }
    if response
        .content_length()
        .is_some_and(|bytes| bytes != definition.bytes)
    {
        return Err(ModelError::Size);
    }
    let mut output = VerifiedFile::new(directory, definition)?;
    progress(0);
    loop {
        let chunk = tokio::select! {
            _=cancel.cancelled()=>return Err(ModelError::Cancelled),
            chunk=response.chunk()=>chunk.map_err(network_error)?,
        };
        let Some(chunk) = chunk else {
            break;
        };
        output.write(&chunk)?;
        progress(output.received());
    }
    if cancel.is_cancelled() {
        return Err(ModelError::Cancelled);
    }
    output.finish()
}

fn hex_digest(digest: impl AsRef<[u8]>) -> String {
    digest
        .as_ref()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

pub fn verify_file(path: &Path, definition: ModelDefinition) -> Result<(), ModelError> {
    verify(path, definition, None)
}

pub fn verify_file_cancellable(
    path: &Path,
    definition: ModelDefinition,
    cancel: &tokio_util::sync::CancellationToken,
) -> Result<(), ModelError> {
    verify(path, definition, Some(cancel))
}

fn verify(
    path: &Path,
    definition: ModelDefinition,
    cancel: Option<&tokio_util::sync::CancellationToken>,
) -> Result<(), ModelError> {
    if cancel.is_some_and(|cancel| cancel.is_cancelled()) {
        return Err(ModelError::Cancelled);
    }
    let mut file = File::open(path)?;
    if file.metadata()?.len() != definition.bytes {
        return Err(ModelError::Size);
    }
    let mut hasher = Sha256::new();
    let mut buffer = [0_u8; 65_536];
    let mut received = 0_u64;
    loop {
        if cancel.is_some_and(|cancel| cancel.is_cancelled()) {
            return Err(ModelError::Cancelled);
        }
        let read = file.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        received += read as u64;
        if received > definition.bytes {
            return Err(ModelError::Size);
        }
        hasher.update(&buffer[..read]);
    }
    if received != definition.bytes {
        return Err(ModelError::Size);
    }
    if hex_digest(hasher.finalize()) != definition.sha256 {
        return Err(ModelError::Hash);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn already_cancelled_setup_performs_no_network_or_file_mutation()
    -> Result<(), Box<dyn std::error::Error>> {
        let directory = tempfile::tempdir()?;
        let cancel = tokio_util::sync::CancellationToken::new();
        cancel.cancel();
        assert!(matches!(
            download_model(directory.path(), REFERENCE, &cancel, |_| {}),
            Err(ModelError::Cancelled)
        ));
        assert_eq!(fs::read_dir(directory.path())?.count(), 0);
        Ok(())
    }

    pub(super) fn fixture() -> ModelDefinition {
        ModelDefinition {
            id: "fixture",
            file: "fixture.bin",
            bytes: 3,
            sha256: "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad",
        }
    }

    #[test]
    fn failed_downloads_keep_the_old_model_and_remove_only_their_part_file()
    -> Result<(), Box<dyn std::error::Error>> {
        let directory = tempfile::tempdir()?;
        let target = directory.path().join("fixture.bin");
        fs::write(&target, b"abc")?;
        verify_file(&target, fixture())?;
        let mut download = VerifiedFile::new(directory.path(), fixture())?;
        download.write(b"abd")?;
        assert!(matches!(download.finish(), Err(ModelError::Hash)));
        assert_eq!(fs::read(&target)?, b"abc");
        assert_eq!(fs::read_dir(directory.path())?.count(), 1);
        let mut download = VerifiedFile::new(directory.path(), fixture())?;
        assert!(matches!(download.write(b"abcd"), Err(ModelError::Size)));
        drop(download);
        let mut download = VerifiedFile::new(directory.path(), fixture())?;
        download.write(b"ab")?;
        assert!(matches!(download.finish(), Err(ModelError::Size)));
        assert_eq!(fs::read_dir(directory.path())?.count(), 1);
        assert_eq!(fs::read(&target)?, b"abc");
        fs::write(&target, b"abd")?;
        assert!(matches!(
            verify_file(&target, fixture()),
            Err(ModelError::Hash)
        ));
        Ok(())
    }

    #[test]
    fn redirect_policy_requires_verified_https_hosts_and_at_most_five_hops()
    -> Result<(), Box<dyn std::error::Error>> {
        for url in [
            "http://huggingface.co/model",
            "https://huggingface.co.evil.test/model",
            "https://example.com/model",
            "https://name:secret@huggingface.co/model",
            "https://huggingface.co:8443/model",
        ] {
            assert!(!redirect_allowed(&reqwest::Url::parse(url)?, 1));
        }
        assert!(redirect_allowed(
            &reqwest::Url::parse("https://us.aws.cdn.hf.co/model")?,
            5
        ));
        assert!(!redirect_allowed(
            &reqwest::Url::parse("https://huggingface.co/model")?,
            6
        ));
        Ok(())
    }

    #[test]
    fn model_is_published_only_after_complete_size_and_hash_verification()
    -> Result<(), Box<dyn std::error::Error>> {
        let directory = tempfile::tempdir()?;
        let fixture = ModelDefinition {
            id: "fixture",
            file: "fixture.bin",
            bytes: 3,
            sha256: "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad",
        };
        let mut download = VerifiedFile::new(directory.path(), fixture)?;
        download.write(b"ab")?;
        assert!(!directory.path().join("fixture.bin").exists());
        download.write(b"c")?;
        let path = download.finish()?;
        assert_eq!(std::fs::read(path)?, b"abc");
        assert_eq!(std::fs::read_dir(directory.path())?.count(), 1);
        Ok(())
    }
    #[test]
    fn cancelled_hash_verification_does_not_open_the_file() -> Result<(), Box<dyn std::error::Error>>
    {
        let directory = tempfile::tempdir()?;
        let cancel = tokio_util::sync::CancellationToken::new();
        cancel.cancel();
        assert!(matches!(
            verify_file_cancellable(&directory.path().join("absent.bin"), fixture(), &cancel),
            Err(ModelError::Cancelled)
        ));
        Ok(())
    }
}

#[cfg(test)]
mod transport_tests;
