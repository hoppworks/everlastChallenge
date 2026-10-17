//! Local HTTP fixtures exercise the production request/body path. Only tests
//! permit loopback plaintext; the public downloader keeps HTTPS and fixed URLs.
use super::*;
use std::{
    fs,
    io::{Read, Write},
    net::{TcpListener, TcpStream},
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};
use tokio_util::sync::CancellationToken;

type TestResult = Result<(), Box<dyn std::error::Error>>;
struct Server {
    url: String,
    worker: Option<JoinHandle<Result<(), String>>>,
    stop: std::sync::Arc<std::sync::atomic::AtomicBool>,
    accepted: std::sync::Arc<std::sync::atomic::AtomicBool>,
}
impl Server {
    fn new(
        reply: impl FnOnce(&mut TcpStream) -> std::io::Result<()> + Send + 'static,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        Self::raw(move |stream| {
            let mut request = Vec::new();
            while !request.ends_with(b"\r\n\r\n") && request.len() < 8192 {
                let mut byte = [0];
                stream.read_exact(&mut byte)?;
                request.push(byte[0]);
            }
            if !request.starts_with(b"GET /model HTTP/1.1\r\n") || !request.ends_with(b"\r\n\r\n") {
                return Err(std::io::Error::other("unexpected local fixture request"));
            }
            reply(stream)
        })
    }
    fn raw(
        reply: impl FnOnce(&mut TcpStream) -> std::io::Result<()> + Send + 'static,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        let listener = TcpListener::bind("127.0.0.1:0")?;
        listener.set_nonblocking(true)?;
        let url = format!("http://{}/model", listener.local_addr()?);
        let stop = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let accepted = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let stopping = std::sync::Arc::clone(&stop);
        let connected = std::sync::Arc::clone(&accepted);
        let worker = thread::spawn(move || {
            let deadline = Instant::now() + Duration::from_secs(3);
            let (mut stream, _) = loop {
                if stopping.load(std::sync::atomic::Ordering::Acquire) {
                    return Ok(());
                }
                match listener.accept() {
                    Ok(connection) => {
                        connected.store(true, std::sync::atomic::Ordering::Release);
                        break connection;
                    }
                    Err(error)
                        if error.kind() == std::io::ErrorKind::WouldBlock
                            && Instant::now() < deadline =>
                    {
                        thread::sleep(Duration::from_millis(2))
                    }
                    Err(error) => return Err(error.to_string()),
                }
            };
            stream
                .set_nonblocking(false)
                .map_err(|error| error.to_string())?;
            stream
                .set_read_timeout(Some(Duration::from_secs(2)))
                .map_err(|error| error.to_string())?;
            stream
                .set_write_timeout(Some(Duration::from_secs(2)))
                .map_err(|error| error.to_string())?;
            reply(&mut stream).map_err(|error| error.to_string())
        });
        Ok(Self {
            url,
            worker: Some(worker),
            stop,
            accepted,
        })
    }
    fn finish(mut self) -> TestResult {
        self.stop.store(true, std::sync::atomic::Ordering::Release);
        self.worker
            .take()
            .ok_or("missing server")?
            .join()
            .map_err(|_| "server panicked")??;
        Ok(())
    }
}
impl Drop for Server {
    fn drop(&mut self) {
        self.stop.store(true, std::sync::atomic::Ordering::Release);
        if let Some(worker) = self.worker.take()
            && worker.join().is_err()
        { /* assertion path already fails the test */ }
    }
}
fn client() -> Result<reqwest::Client, reqwest::Error> {
    download_client_builder()
        .https_only(false)
        .no_proxy()
        .read_timeout(Duration::from_millis(100))
        .build()
}
fn run(
    directory: &Path,
    source: &str,
    client: &reqwest::Client,
    cancel: &CancellationToken,
    mut progress: impl FnMut(u64),
) -> Result<PathBuf, ModelError> {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()?
        .block_on(transfer(
            directory,
            super::tests::fixture(),
            cancel,
            &mut progress,
            client,
            source,
        ))
}
fn old_model(directory: &Path) -> TestResult {
    fs::write(directory.join("fixture.bin"), b"abc")?;
    fs::write(directory.join("foreign.part"), b"foreign")?;
    Ok(())
}
fn assert_old_model_and_no_own_part(directory: &Path) -> TestResult {
    assert_eq!(fs::read(directory.join("fixture.bin"))?, b"abc");
    assert_eq!(fs::read(directory.join("foreign.part"))?, b"foreign");
    assert_eq!(fs::read_dir(directory)?.count(), 2);
    Ok(())
}

#[test]
fn chunked_transfer_reports_real_bounded_bytes_and_publishes_verified_model() -> TestResult {
    let directory = tempfile::tempdir()?;
    let server = Server::new(|stream| {
        stream.write_all(b"HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\nConnection: close\r\n\r\n1\r\na\r\n2\r\nbc\r\n0\r\n\r\n")
    })?;
    let mut progress = Vec::new();
    let path = run(
        directory.path(),
        &server.url,
        &client()?,
        &CancellationToken::new(),
        |bytes| progress.push(bytes),
    )?;
    assert_eq!(fs::read(path)?, b"abc");
    assert_eq!(progress.first(), Some(&0));
    assert_eq!(progress.last(), Some(&3));
    assert!(
        progress
            .windows(2)
            .all(|pair| pair[0] <= pair[1] && pair[1] <= 3)
    );
    assert_eq!(fs::read_dir(directory.path())?.count(), 1);
    server.finish()
}

#[test]
fn response_failures_never_replace_the_old_model_or_leave_own_parts() -> TestResult {
    let responses: &[(&[u8], &str)] = &[
        (b"HTTP/1.1 503 Service Unavailable\r\nContent-Length: 0\r\n\r\n", "http"),
        (b"HTTP/1.1 200 OK\r\nContent-Length: 4\r\n\r\nabcd", "size"),
        (b"HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\n4\r\nabcd\r\n0\r\n\r\n", "size"),
        (b"HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\n2\r\nab\r\n0\r\n\r\n", "size"),
        (b"HTTP/1.1 200 OK\r\nContent-Length: 3\r\n\r\nabd", "hash"),
        (b"HTTP/1.1 200 OK\r\nContent-Length: 3\r\n\r\nab", "network"),
        (b"HTTP/1.1 302 Found\r\nLocation: http://127.0.0.1:1/forbidden\r\nContent-Length: 0\r\n\r\n", "redirect"),
    ];
    for &(response, expected) in responses {
        let directory = tempfile::tempdir()?;
        old_model(directory.path())?;
        let server = Server::new(move |stream| stream.write_all(response))?;
        let error = run(
            directory.path(),
            &server.url,
            &client()?,
            &CancellationToken::new(),
            |_| {},
        )
        .err()
        .ok_or("bad response must fail")?;
        assert!(
            matches!(
                (&error, expected),
                (ModelError::Http(503), "http")
                    | (ModelError::Size, "size")
                    | (ModelError::Hash, "hash")
                    | (ModelError::Network, "network")
                    | (ModelError::Redirect, "redirect")
            ),
            "unexpected error: {error}"
        );
        assert_old_model_and_no_own_part(directory.path())?;
        server.finish()?;
    }
    Ok(())
}

#[test]
fn body_inactivity_is_a_timeout_and_preserves_the_old_model() -> TestResult {
    let directory = tempfile::tempdir()?;
    old_model(directory.path())?;
    let (release, held) = std::sync::mpsc::channel();
    let server = Server::new(move |stream| {
        stream.write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 3\r\n\r\na")?;
        // Keep the response open until the test observes the client outcome.
        // A TCP read is unsuitable here: HTTP clients may half-close writes.
        if held.recv_timeout(Duration::from_secs(2)).is_err() { /* bounded fixture cleanup */ }
        Ok(())
    })?;
    let result = run(
        directory.path(),
        &server.url,
        &client()?,
        &CancellationToken::new(),
        |_| {},
    );
    release.send(())?;
    assert!(
        matches!(result, Err(ModelError::Timeout)),
        "expected timeout, got {result:?}"
    );
    assert_old_model_and_no_own_part(directory.path())?;
    server.finish()
}

#[test]
fn cancellation_during_body_read_discards_partial_bytes_without_late_publish() -> TestResult {
    let directory = tempfile::tempdir()?;
    old_model(directory.path())?;
    let server = Server::new(|stream| {
        stream.write_all(b"HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\n1\r\na\r\n")?;
        let mut byte = [0];
        if stream.read(&mut byte).is_err() { /* fixture timeout also closes the response */ }
        Ok(())
    })?;
    let cancel = CancellationToken::new();
    assert!(matches!(
        run(
            directory.path(),
            &server.url,
            &client()?,
            &cancel,
            |bytes| {
                if bytes > 0 {
                    cancel.cancel();
                }
            }
        ),
        Err(ModelError::Cancelled)
    ));
    assert_old_model_and_no_own_part(directory.path())?;
    server.finish()
}

#[test]
fn production_client_refuses_plaintext_before_any_request() -> TestResult {
    let directory = tempfile::tempdir()?;
    let server = Server::new(|stream| {
        stream.write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 3\r\nConnection: close\r\n\r\nabc")
    })?;
    let production = download_client_builder().no_proxy().build()?;
    let result = run(
        directory.path(),
        &server.url,
        &production,
        &CancellationToken::new(),
        |_| {},
    );
    assert!(
        matches!(result, Err(ModelError::Network)),
        "plaintext was not rejected: {result:?}"
    );
    assert!(
        !server.accepted.load(std::sync::atomic::Ordering::Acquire),
        "plaintext connection reached the fixture"
    );
    assert_eq!(fs::read_dir(directory.path())?.count(), 0);
    server.finish()
}

#[test]
fn untrusted_tls_is_rejected_and_explicit_test_trust_is_a_positive_control() -> TestResult {
    const CERTIFICATE: &[u8] = include_bytes!("../../tests/fixtures/tls/untrusted-localhost.der");
    const KEY: &[u8] = include_bytes!("../../tests/fixtures/tls/public-test-key.der");
    let configuration = std::sync::Arc::new(
        rustls::ServerConfig::builder()
            .with_no_client_auth()
            .with_single_cert(
                vec![rustls::pki_types::CertificateDer::from(
                    CERTIFICATE.to_vec(),
                )],
                rustls::pki_types::PrivatePkcs8KeyDer::from(KEY.to_vec()).into(),
            )?,
    );
    for trust_fixture in [false, true] {
        let configuration = std::sync::Arc::clone(&configuration);
        let server = Server::raw(move |stream| {
            let connection =
                rustls::ServerConnection::new(configuration).map_err(std::io::Error::other)?;
            let mut tls = rustls::StreamOwned::new(connection, stream);
            let mut request = Vec::new();
            while !request.ends_with(b"\r\n\r\n") && request.len() < 8192 {
                let mut byte = [0];
                // A certificate rejection is expected for the ordinary client.
                if let Err(error) = tls.read_exact(&mut byte) {
                    return if !trust_fixture { Ok(()) } else { Err(error) };
                }
                request.push(byte[0]);
            }
            tls.write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 3\r\nConnection: close\r\n\r\nabc")?;
            tls.flush()
        })?;
        let mut builder = download_client_builder().no_proxy();
        if trust_fixture {
            builder = builder.add_root_certificate(reqwest::Certificate::from_der(CERTIFICATE)?);
        }
        let client = builder.build()?;
        let directory = tempfile::tempdir()?;
        old_model(directory.path())?;
        let result = run(
            directory.path(),
            &server.url.replacen("http:", "https:", 1),
            &client,
            &CancellationToken::new(),
            |_| {},
        );
        if trust_fixture {
            assert_eq!(fs::read(result?)?, b"abc");
        } else {
            assert!(
                matches!(result, Err(ModelError::Tls)),
                "product client must classify the untrusted certificate as TLS: {result:?}"
            );
        }
        assert_old_model_and_no_own_part(directory.path())?;
        server.finish()?;
    }
    Ok(())
}

#[test]
fn external_cancel_interrupts_an_outstanding_body_read() -> TestResult {
    let directory = tempfile::tempdir()?;
    old_model(directory.path())?;
    let (entered, reading) = std::sync::mpsc::channel();
    let (release, held) = std::sync::mpsc::channel();
    let server = Server::new(move |stream| {
        stream.write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 3\r\n\r\n")?;
        if held.recv_timeout(Duration::from_secs(2)).is_err() { /* bounded fixture cleanup */ }
        Ok(())
    })?;
    let source = server.url.clone();
    let path = directory.path().to_owned();
    let cancel = CancellationToken::new();
    let requested = cancel.clone();
    let (finished, result) = std::sync::mpsc::channel();
    let worker = thread::spawn(move || {
        let outcome = download_client_builder()
            .https_only(false)
            .no_proxy()
            .build()
            .map_err(network_error)
            .and_then(|client| {
                run(&path, &source, &client, &requested, |bytes| {
                    if bytes == 0 && entered.send(()).is_err() { /* assertion cleanup */ }
                })
            });
        if finished.send(outcome).is_err() { /* assertion cleanup may have dropped receiver */ }
    });
    reading.recv_timeout(Duration::from_secs(1))?;
    assert!(
        result.try_recv().is_err(),
        "request finished before cancellation"
    );
    cancel.cancel();
    let outcome = result.recv_timeout(Duration::from_secs(1))?;
    release.send(())?;
    worker.join().map_err(|_| "download worker panicked")?;
    assert!(matches!(outcome, Err(ModelError::Cancelled)));
    assert_old_model_and_no_own_part(directory.path())?;
    server.finish()
}

#[test]
fn connection_refused_stays_network_and_preserves_old_model_and_foreign_part() -> TestResult {
    // Reserve a kernel-chosen loopback port, then close its listener. There is
    // no TLS peer: refusal must not be mislabeled as a certificate failure.
    let listener = TcpListener::bind("127.0.0.1:0")?;
    let address = listener.local_addr()?;
    drop(listener);
    let directory = tempfile::tempdir()?;
    old_model(directory.path())?;
    let client = download_client_builder()
        .no_proxy()
        .connect_timeout(Duration::from_millis(200))
        .timeout(Duration::from_secs(1))
        .build()?;
    let result = run(
        directory.path(),
        &format!("https://{address}/model"),
        &client,
        &CancellationToken::new(),
        |_| {},
    );
    assert!(
        matches!(result, Err(ModelError::Network)),
        "refused TCP connection: {result:?}"
    );
    assert_old_model_and_no_own_part(directory.path())
}

#[test]
fn typed_tls_cause_survives_nested_io_wrappers_without_message_classification() {
    let certificate = rustls::Error::InvalidCertificate(rustls::CertificateError::UnknownIssuer);
    let nested = std::io::Error::other(std::io::Error::new(
        std::io::ErrorKind::InvalidData,
        certificate,
    ));
    assert!(has_rustls_cause(&nested));
    let alert = rustls::Error::AlertReceived(rustls::AlertDescription::HandshakeFailure);
    assert!(has_rustls_cause(&std::io::Error::other(alert)));
    assert!(!has_rustls_cause(&std::io::Error::from(
        std::io::ErrorKind::ConnectionRefused
    )));
    assert!(!has_rustls_cause(&std::io::Error::new(
        std::io::ErrorKind::UnexpectedEof,
        "tls handshake eof"
    )));
    assert!(!has_rustls_cause(&std::io::Error::other(
        "rustls: invalid certificate"
    )));
}
