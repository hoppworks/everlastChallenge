# Own native Preferences close contract — 2026-10-19

The focused AppKit contract did **not** reproduce hiding or losing the separately owned main window when closing Preferences. This is not a conclusion about the installed Tauri application's window lifecycle or CUA's window discovery.

## Executed boundary

`crates/macos-integration/tests/native_preferences_close_contract.rs` runs on the actual main thread with a fresh `NSApplication`, a retained own `NSWindow`, and the actual `NativePreferences` component. Its controlled state contains no model/audio operation. It shows the own main window, then performs three Preferences show → native close-button `performClick` cycles. Each cycle asserts Preferences is no longer visible while the main window remains visible and present in `NSApplication.windows`. A separate `NativePreferences::close()` and `detach()` leave the main window visible as well. The harness finally closes its own main window.

Observed output for all three cycles:

```text
preferencesVisible=false; mainVisible=true; mainInOwnWindows=true
```

Recorded command:

```sh
cargo test --locked --offline -p macos-integration --test native_preferences_close_contract
```

The author execution handle `38169` completed with exit `0`. The saved log records successful compilation and the three observations. A focused strict Clippy check also completed with exit `0`; that statement is the author's tool result, not a separate independent native run.

## Bound evidence

| File | SHA-256 |
| --- | --- |
| `crates/macos-integration/tests/native_preferences_close_contract.rs` | `09c2b8e3160bc362c997e8dc226d44db0a55559598d4a29e925dcc26bfedfbe3` |
| `artifacts/offline-continuation-v1/preferences-close-contract.log` | `c5bfae3cac1ab7f27aeb7ed18acc5ffbbc294a5799d7c7b05b656cb163835689` |

The harness is registered as an independent `harness = false` native test in the native crate manifest. These hashes identify the test/log read for this report; they do not identify an installed app binary.

## What remains distinct

- The harness uses own AppKit windows, not the Tauri/Wry main window, installed bundle, or a running product's AppKit/Tauri delegate interaction.
- The close is an in-process action on the own native button. No CUA action, foreign Accessibility query, external app activation, keyboard injection, microphone, permission prompt, or model is involved.
- It does not test CUA timeouts, application reachability after an external close request, Window-menu reopening, or every installed-app close/hide route.
- Source inspection found the Tauri close-request/hide handler attached to the Tauri main window; this does not establish that a particular CUA event selected that window. No product fix is justified solely by this passing isolated contract.

The original report of an installed app becoming unreachable through CUA remains an integration observation requiring its own controlled reproduction. This contract narrows the hypothesis; it does not dismiss that observation or mark a full native acceptance gate passed.
