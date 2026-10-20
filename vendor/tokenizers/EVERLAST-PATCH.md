# Tokenizers 0.23.2 build macro dependency

Only the normalized Cargo manifest changes: the existing `paste` dependency name aliases the maintained `pastey` package, exactly0.2.3. RustSec RUSTSEC-2024-0436 records paste as unmaintained and names pastey as a drop-in alternative. No runtime, tokenization, normalization or serialization source changes. Upstream licenses and Cargo.toml.orig retained; revision/checksum in EVERLAST-PATCH.json. The two macro call sites compile unchanged. This local dependency patch needs maintenance; the existing advisory gate is not weakened.
