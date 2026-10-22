use std::{io::Cursor, num::NonZeroU64};
use text_refinement_protocol::{JobKey, Language, Request, read_request, write_request};

type TestResult = Result<(), Box<dyn std::error::Error>>;

fn job() -> JobKey {
    JobKey {
        session: NonZeroU64::MIN,
        generation: NonZeroU64::MIN,
        request: NonZeroU64::MIN,
    }
}

#[test]
fn unicode_request_roundtrips_with_all_session_identity_fields() -> TestResult {
    let request = Request::Run {
        job: job(),
        language: Language::German,
        text: "Müller sagt: „Guten Tag!“".into(),
    };
    let mut pipe = Vec::new();
    write_request(&mut pipe, &request)?;
    let mut input = Cursor::new(pipe);
    let decoded = read_request(&mut input)?.ok_or("missing frame")?;
    assert!(decoded == request);
    assert!(read_request(&mut input)?.is_none());
    Ok(())
}

fn raw_frame(json: &str) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    let mut bytes = u32::try_from(json.len())?.to_be_bytes().to_vec();
    bytes.extend_from_slice(json.as_bytes());
    Ok(bytes)
}

#[test]
fn malformed_unknown_and_duplicate_fields_are_not_silently_accepted() -> TestResult {
    let bodies = [
        r#"{"version":1,"version":1,"payload":{"kind":"shutdown"}}"#,
        r#"{"version":1,"payload":{"kind":"shutdown"},"unexpected":0}"#,
        r#"{"version":1,"payload":{"kind":"shutdown","unexpected":0}}"#,
        r#"{"version":1,"payload":{"kind":"shutdown","kind":"shutdown"}}"#,
        r#"{"version":1,"payload":{"kind":"run","job":{"session":1,"generation":1,"request":1},"language":"german","text":"one","text":"two"}}"#,
        r#"{"version":1,"payload":{"kind":"run","job":{"session":0,"generation":1,"request":1},"language":"german","text":"one"}}"#,
        r#"{"version":1,"payload":{"kind":"run","job":{"session":1,"generation":1,"request":1},"language":"unknown","text":"one"}}"#,
    ];
    for body in bodies {
        assert!(
            read_request(&mut Cursor::new(raw_frame(body)?)).is_err(),
            "{body}"
        );
    }
    Ok(())
}

#[test]
fn incompatible_version_is_rejected() -> TestResult {
    use text_refinement_protocol::ProtocolError;
    let body = r#"{"version":2,"payload":{"kind":"shutdown"}}"#;
    assert!(matches!(
        read_request(&mut Cursor::new(raw_frame(body)?)),
        Err(ProtocolError::Version)
    ));
    Ok(())
}

#[test]
fn overlong_prefix_is_rejected_before_its_body_is_read() {
    use text_refinement_protocol::{MAX_FRAME_BYTES, ProtocolError};
    let mut input = Cursor::new(u32::MAX.to_be_bytes().to_vec());
    assert!(u32::MAX as u64 > MAX_FRAME_BYTES as u64);
    assert!(matches!(
        read_request(&mut input),
        Err(ProtocolError::TooLarge)
    ));
    assert_eq!(input.position(), 4);
}

#[test]
fn partial_header_or_body_is_not_clean_shutdown() {
    use text_refinement_protocol::ProtocolError;
    for bytes in [vec![0, 0], vec![0, 0, 0, 4, b'{']] {
        assert!(matches!(
            read_request(&mut Cursor::new(bytes)),
            Err(ProtocolError::Truncated)
        ));
    }
    assert!(matches!(
        read_request(&mut Cursor::new(vec![0, 0, 0, 0])),
        Err(ProtocolError::Malformed)
    ));
}

#[test]
fn size_failures_do_not_write_a_partial_frame() {
    use text_refinement_protocol::ProtocolError;
    let mut pipe = vec![17];
    for (text, oversized_input) in [
        ("x".repeat(32 * 1024 + 1), true),
        ("\0".repeat(32 * 1024), false),
    ] {
        let request = Request::Run {
            job: job(),
            language: Language::German,
            text,
        };
        let error = write_request(&mut pipe, &request);
        assert!(if oversized_input {
            matches!(error, Err(ProtocolError::InputLimit))
        } else {
            matches!(error, Err(ProtocolError::TooLarge))
        });
        assert_eq!(pipe, [17]);
    }
}

#[test]
fn decoded_text_limit_is_checked_before_dispatch() -> TestResult {
    use text_refinement_protocol::ProtocolError;
    let text = "x".repeat(32 * 1024 + 1);
    let body = format!(
        r#"{{"version":1,"payload":{{"kind":"run","job":{{"session":1,"generation":1,"request":1}},"language":"german","text":"{text}"}}}}"#
    );
    assert!(matches!(
        read_request(&mut Cursor::new(raw_frame(&body)?)),
        Err(ProtocolError::InputLimit)
    ));
    Ok(())
}

#[test]
fn adjacent_frames_keep_their_boundaries() -> TestResult {
    let mut bytes = Vec::new();
    write_request(&mut bytes, &Request::Cancel { job: job() })?;
    write_request(&mut bytes, &Request::Shutdown {})?;
    let mut input = Cursor::new(bytes);
    assert!(read_request(&mut input)? == Some(Request::Cancel { job: job() }));
    assert!(read_request(&mut input)? == Some(Request::Shutdown {}));
    assert!(read_request(&mut input)?.is_none());
    Ok(())
}

#[test]
fn candidate_schema_rejects_duplicate_keys_and_unknown_edits() -> TestResult {
    use text_refinement_protocol::read_response;
    for edit in [
        r#"{"kind":"filler","before":"uh","before":"um","after":""}"#,
        r#"{"kind":"filler","before":"uh","after":"","command":"run"}"#,
        r#"{"kind":"unknown","before":"uh","after":""}"#,
    ] {
        let body = format!(
            r#"{{"version":1,"payload":{{"kind":"completed","job":{{"session":1,"generation":1,"request":1}},"edits":[{edit}]}}}}"#
        );
        assert!(read_response(&mut Cursor::new(raw_frame(&body)?)).is_err());
    }
    Ok(())
}
