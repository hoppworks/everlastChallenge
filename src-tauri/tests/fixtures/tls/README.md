# Public, untrusted TLS fixture

The certificate and PKCS#8 key were generated locally for loopback-only download
regression tests. The key is deliberately public test data, not a credential.
Nothing trusts this certificate outside the explicit positive-control test
client. It is never installed in a system trust store or shipped in the app.

The certificate has DNS localhost and IP 127.0.0.1 SANs and serverAuth usage.
Tests first show that the ordinary product client rejects it before publishing
any bytes, then explicitly trust this exact certificate in a test-only client to
show that the same server and model body otherwise succeed. This guards against
silently disabling certificate verification.
