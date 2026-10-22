#![forbid(unsafe_code)]
//! Versioned, bounded messages between the desktop host and its private worker.

use dictation_core::refinement::StagedEdits;
use std::{
    io::{Read, Write},
    num::NonZeroU64,
};

mod frame;

pub const VERSION: u16 = 2;
pub const TEXT_PROMPT_VERSION: u16 = 8;
pub const TEXT_POLICY_VERSION: u16 = 7;
pub const MAX_FRAME_BYTES: usize = 64 * 1024;

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct JobKey {
    pub session: NonZeroU64,
    pub generation: NonZeroU64,
    pub request: NonZeroU64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Language {
    German,
    English,
}

impl From<Language> for dictation_core::refinement::Language {
    fn from(value: Language) -> Self {
        match value {
            Language::German => Self::German,
            Language::English => Self::English,
        }
    }
}

#[derive(Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Request {
    Run {
        job: JobKey,
        language: Language,
        text: String,
    },
    Cancel {
        job: JobKey,
    },
    Shutdown {},
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FailureKind {
    NotReady,
    Busy,
    InvalidRequest,
    ModelLoad,
    InputLimit,
    Generation,
    Timeout,
    Incomplete,
    InvalidCandidate,
}

#[derive(Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Response {
    Ready {
        generation: NonZeroU64,
        model_manifest_sha256: String,
        prompt_version: u16,
        policy_version: u16,
    },
    Completed {
        job: JobKey,
        proposal: StagedEdits,
    },
    Failed {
        job: Option<JobKey>,
        code: FailureKind,
    },
    Cancelled {
        job: JobKey,
    },
}

#[derive(Debug, thiserror::Error)]
pub enum ProtocolError {
    #[error("Worker protocol I/O failed")]
    Io(#[source] std::io::Error),
    #[error("Worker protocol frame is incomplete")]
    Truncated,
    #[error("Worker protocol frame exceeds its size limit")]
    TooLarge,
    #[error("Worker protocol message is malformed")]
    Malformed,
    #[error("Worker protocol version is incompatible")]
    Version,
    #[error("Worker request exceeds its text limit")]
    InputLimit,
}

pub fn read_request(reader: &mut impl Read) -> Result<Option<Request>, ProtocolError> {
    let request = frame::read(reader)?;
    if let Some(request) = &request {
        check_request(request)?;
    }
    Ok(request)
}

pub fn write_request(writer: &mut impl Write, request: &Request) -> Result<(), ProtocolError> {
    check_request(request)?;
    frame::write(writer, request)
}

fn check_request(request: &Request) -> Result<(), ProtocolError> {
    if let Request::Run { text, .. } = request
        && text.len() > dictation_core::refinement::MAX_ORIGINAL_BYTES
    {
        return Err(ProtocolError::InputLimit);
    }
    Ok(())
}

pub fn read_response(reader: &mut impl Read) -> Result<Option<Response>, ProtocolError> {
    frame::read(reader)
}

pub fn write_response(writer: &mut impl Write, response: &Response) -> Result<(), ProtocolError> {
    frame::write(writer, response)
}
