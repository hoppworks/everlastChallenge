//! Complete text materialization and completion-aware whole-input retry.
use super::{InferenceError, synchronize};
use mlxrs::{
    Array,
    audio::stt::models::whisper::{
        backend::WhisperBackend,
        decoding::{self, TranscribeOptions, TranscribeResult},
        tokenizer::HFTokenizerWrapper,
    },
};

pub(super) struct Complete {
    pub result: TranscribeResult,
    pub native_timestamp_retry: bool,
    pub without_timestamps: bool,
}

pub(super) fn transcribe(
    backend: &WhisperBackend<'_>,
    tokenizer: &HFTokenizerWrapper<'_>,
    mel: &Array,
    content_frames: usize,
    options: &TranscribeOptions,
) -> Result<Complete, InferenceError> {
    let attempted = decoding::transcribe(backend, tokenizer, mel, content_frames, options);
    let (result, retried) = match attempted {
        Ok(result) => (result, false),
        Err(mlxrs::Error::CapExceeded(detail))
            if options.decode.without_timestamps
                && detail.context() == "Whisper text-only completion" =>
        {
            // No preliminary text escapes. Flush discarded speculative GPU work,
            // then start again over the same whole mel with fresh decoder caches.
            synchronize()?;
            let mut native = options.clone();
            native.decode.without_timestamps = false;
            let result = decoding::transcribe(backend, tokenizer, mel, content_frames, &native)
                .map_err(|_| InferenceError::Decode)?;
            (result, true)
        }
        Err(_) => return Err(InferenceError::Decode),
    };
    synchronize()?;
    Ok(Complete {
        result,
        native_timestamp_retry: retried,
        without_timestamps: options.decode.without_timestamps && !retried,
    })
}
