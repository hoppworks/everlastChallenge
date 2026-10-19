# Local developer test control

This is an explicit developer test interface, not a production command surface or evidence that a physical/global shortcut works. The Cargo feature `test-control` is **off by default** and propagates only to the matching native test boundary. Without that feature there is no endpoint; with it but without `EVERLAST_TEST_CONTROL_DIR`, there is still no endpoint or polling thread.

## Enable deliberately

Use an explicitly selected, separately built/signed developer app with `test-control` enabled; preserve the ordinary build before an authorized installation change. Set `EVERLAST_TEST_CONTROL_DIR` in that app process's startup environment to an **existing canonical absolute directory** owned by the same user, mode `0700`.

Before launch, create `config.json` as an owned regular file, mode `0600`, at most 256 bytes:

```json
{"recipientPid":12345}
```

Replace `12345` with the positive PID of the independently launched, disposable native recipient. Unknown config keys are rejected. No executable, text, bundle identifier, microphone device, or arbitrary command can be supplied through this file. The server requires a fresh `control.sock`; it never overwrites an existing socket or file. The socket is mode `0600`. Symlink/noncanonical directories and symlink config files are rejected.

Start admission additionally requires that the actual frontmost application's PID equals `recipientPid` and its actual bundle identifier is one of:

- `de.hoppworks.everlast-native-recipient-hotkey-v1`
- `de.hoppworks.everlast-native-test-recipient`
- `de.hoppworks.everlast-native-autonomous-recipient`

The check obtains native process identity; the configured PID alone is not target authority. It does not activate any app. Setup must be ready, the runtime idle, existing microphone and Accessibility permissions granted, Secure Input inactive, and the configured device the **system-default microphone**. Permission dialogs, OS settings changes, arbitrary foreign targets, configuration updates, text injection, and generic UI/AX commands are absent from this API. The ordinary capture/target/delivery checks still run.

## Manual build and launch on this Mac

This sequence is for the **current local workspace on the already prepared Mac**,
not a promise that a fresh checkout contains a certificate, models or prior
permissions. First complete the README dependencies (`npm run setup`). The
user-approved local Code Signing identity and `.everlast-signing.json` must
already exist; this sequence creates/imports no identity and changes no trust.
The existing model cache and OS permissions remain necessary.

Normal `npm run release` and `npm run release:local` leave `test-control` off.
The following commands explicitly build it in a **new private target directory**.
Run the blocks in the same terminal from the repository root; stop immediately
if any command fails. Before launching
the developer app, quit the currently running Everlast normally through its
menu or **⌘Q**. Closing its window is insufficient: the process would retain
the global hotkey. Keep the installed app and historical ZIP untouched; this
sequence launches its new bundle directly instead of copying it to `/Applications`.

Create a private workspace and build the disposable recipient:

```sh
umask 077
repo_root="$(pwd -P)"
mkdir -p "$repo_root/artifacts"
dev_root="$(mktemp -d "$repo_root/artifacts/manual-test-control.XXXXXX")"
CARGO_TARGET_DIR="$dev_root/target" MACOSX_DEPLOYMENT_TARGET=27.0 cargo build --locked --release \
  -p macos-integration --example native_text_recipient --target aarch64-apple-darwin
recipient_app="$dev_root/Everlast Native Recipient.app"
mkdir -p "$recipient_app/Contents/MacOS"
cp "$dev_root/target/aarch64-apple-darwin/release/examples/native_text_recipient" \
  "$recipient_app/Contents/MacOS/native_text_recipient"
cat > "$recipient_app/Contents/Info.plist" <<'PLIST'
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0"><dict>
<key>CFBundleIdentifier</key><string>de.hoppworks.everlast-native-autonomous-recipient</string>
<key>CFBundleExecutable</key><string>native_text_recipient</string>
<key>CFBundleName</key><string>Everlast Native Recipient</string>
<key>CFBundlePackageType</key><string>APPL</string>
<key>CFBundleVersion</key><string>1</string>
<key>LSMinimumSystemVersion</key><string>27.0</string>
</dict></plist>
PLIST
signing_id="$(node -p 'JSON.parse(require("node:fs").readFileSync(".everlast-signing.json", "utf8")).signingIdentity')"
codesign --force --sign "$signing_id" --timestamp=none --options runtime "$recipient_app"
codesign --verify --deep --strict "$recipient_app"
```

Build and sign the developer app. The explicit feature switch is the difference
from the normal release commands:

```sh
CARGO_TARGET_DIR="$dev_root/target" MACOSX_DEPLOYMENT_TARGET=27.0 npm run tauri -- build \
  --bundles app --target aarch64-apple-darwin --features test-control
dev_app="$dev_root/target/aarch64-apple-darwin/release/bundle/macos/Everlast Dictation.app"
node scripts/sign-local-bundle.mjs "$dev_app"
```

Start only the newly built recipient, record its actual PID, then start the
new developer app with the endpoint directory in **its own process environment**.
The short canonical `/private/tmp` path avoids the macOS Unix-socket path limit;
it is newly created, private, and contains only this test session's control files:

```sh
EVERLAST_RECIPIENT_REPORT_DIR="$dev_root/recipient-report" \
  "$recipient_app/Contents/MacOS/native_text_recipient" \
  > "$dev_root/recipient.log" 2>&1 &
recipient_pid=$!
control_dir="$(mktemp -d /private/tmp/everlast-control.XXXXXX)"
chmod 700 "$control_dir"
printf '{"recipientPid":%s}\n' "$recipient_pid" > "$control_dir/config.json"
chmod 600 "$control_dir/config.json"
EVERLAST_TEST_CONTROL_DIR="$control_dir" \
  "$dev_app/Contents/MacOS/everlast-dictation" > "$dev_root/app.log" 2>&1 &
dev_pid=$!
```

Wait for the developer app to finish setup and expose `control.sock`, then:

```sh
node scripts/test-control.mjs --control-dir "$control_dir" status
```

Setting `EVERLAST_TEST_CONTROL_DIR` only on this **Node client** cannot activate
an endpoint in an already running app. Finder/`open` launch is deliberately not
used above: the direct executable receives the specified environment and gives
an unambiguous process PID. If either process exits, inspect its private log;
do not reuse its PID or a stale socket. Quit both test apps normally after the
session; keep the private directory as evidence if needed.

For a real start, click the recipient's editable field and confirm its own
report has `frontmostIsSelf=true`; local `applicationActive`/`windowKey` alone
are insufficient. Typing a command in Terminal moves the foreground away from
the recipient. One manual way to allow the click before the command is:

```sh
(sleep 5; node scripts/test-control.mjs --control-dir "$control_dir" start \
  > "$dev_root/start.json") &
```

During those five seconds click the recipient field. Afterwards query `status`
and use only its non-null `controlledSessionId` for `stop` or `cancel`, never
a guessed `sessionId`. Terminal focus must again be returned to the recipient
before an intended delivery; the same five-second delay can be used for a
single `stop SID` command. No automatic retry follows a rejected start. This
manual sequence does not claim a successful recording, foreground assignment,
or delivery, and does not replace the separate physical-hotkey acceptance.

## Commands and replies

Each connection carries one UTF-8 newline-terminated JSON request (maximum 1,024 bytes including newline), then one newline-terminated JSON reply and EOF. Requests deny unknown keys and duplicate fields. `schemaVersion` must be `1`.

```json
{"schemaVersion":1,"op":"status"}
{"schemaVersion":1,"op":"start"}
{"schemaVersion":1,"op":"stop","session":7}
{"schemaVersion":1,"op":"cancel","session":7}
```

`session` is required and positive only for `stop` and `cancel`; it is forbidden for `status` and `start`. The Node client also requires a JavaScript-safe integer.

```sh
node scripts/test-control.mjs --control-dir /absolute/private/directory status
node scripts/test-control.mjs --control-dir /absolute/private/directory start
node scripts/test-control.mjs --control-dir /absolute/private/directory stop 7
node scripts/test-control.mjs --control-dir /absolute/private/directory cancel 7
```

The client can instead read the directory from `EVERLAST_TEST_CONTROL_DIR`; `--timeout-ms` defaults to 3,000 (accepted range 10–30,000). It does not automatically retry. A missing reply/connection timeout leaves the command's outcome unknown: inspect status before deciding on a subsequent action.

A successful envelope is `{ "schemaVersion":1, "ok":true, "op":"status", "snapshot":{...}, "error":null }`. Rejection uses `ok:false`, `snapshot:null`, and an error string; malformed requests may use `op:"invalid"`. The CLI sanitizes server error strings and returns a nonzero exit status on rejection or protocol/transport failure.

The snapshot contains exactly these scalar fields:

| Field | Meaning |
| --- | --- |
| `setupReady`, `microphoneGranted`, `accessibilityGranted` | Current readiness/permission booleans; not permission requests. |
| `phase` | `Idle`, `Starting`, `Recording`, `Finalizing`, `Delivering`, or `Cancelling`. |
| `sessionId` | Current positive runtime SID, or null. |
| `controlledSessionId` | SID belonging to this endpoint's actual admitted start, and still matching the current session; otherwise null. |
| `controlledStartPending` | The endpoint's asynchronous start receipt is outstanding. |
| `outcome` | null, `local`, `empty`, `sent`, `cancelled`, `held`, or `failed`. |
| `publicSentenceMatches` | Whether whitespace-collapsed transcript equals the fixed public FLEURS sentence below. |
| `transcriptCharacters` | Character count, not transcript content. |
| `actionError` | Whether a visible action error exists, not its contents. |

Fixed public sentence: “Forschung im Bereich KI umfasst die Herstellung von Maschinen zur Automatisierung von Aufgaben, die intelligentes Verhalten erfordern.” No raw transcript or arbitrary expected text is accepted/exported. A transcript match is not proof of target receipt; inspect the independently controlled recipient's own telemetry for that separate observation.

## Ownership and bounded end

Start is asynchronous acceptance, never a toggle. Poll status until the **controlled** SID is observed; do not infer ownership merely from a changed current SID. The Runtime actor returns the exact admitted SID before processing the next request. Desktop keeps pending/closed/session in one shared Mutex snapshot, so expiry and receipt cannot miss each other. Shutdown closes admission synchronously; a late successful receipt cancels that same SID.

Duplicate/overlapping starts are rejected. An asynchronously rejected start clears pending/deadline and permits a new **explicit** attempt; there is no automatic retry. Stop/cancel must name the endpoint-owned SID and also pass existing runtime stale-session checks. They cannot cancel an unrelated manually started session.

The endpoint expires after 15 minutes. A successful start has a 60-second limit measured from acceptance, including time waiting for its receipt. Expiry or that limit permanently closes test-start admission and requests cancellation of the owned session (including a late receipt); ordinary app shutdown does likewise. These are main-thread, event-loop-serviced deadlines, not hard real-time audio interruption guarantees. After the 60-second limit, status can remain readable until the endpoint's 15-minute expiry, but no new start is allowed. The socket is unlinked on expiry/cleanup only if its inode still matches the server's own socket. A crash may leave a stale socket; use a fresh private directory instead of overwriting it.

Background I/O has a 250-ms whole-request read deadline, a bounded single pending channel, a 2-second main-response deadline, and 250-ms write timeout. Main-thread polling never waits on a client. No recording starts merely because the app or endpoint starts.

## Evidence boundary

Focused transport tests exercise actual local Unix sockets, permissions/collisions, malformed requests, privacy projection, exact session ownership, failed asynchronous admission/retry, and bounded expiry with a controlled handler. Separate Desktop/Runtime tests cover the actor receipt and shutdown handshake. These tests do not claim microphone capture, foreign AX success, physical hotkey dispatch, or delivered target text. Actual signed-app runs, source/binary binding, recipient observations, and native acceptance remain separate evidence.

## Read-only field inspection

```sh
node scripts/test-control.mjs --control-dir /absolute/private/directory inspect
```

`inspect` accepts no session or target argument. It uses only the configured
recipient PID, verifies its kernel identity and one of the three existing
own-recipient bundle identifiers before and after reading, and requires the
already granted Accessibility permission. It does not require that recipient
to be frontmost. It never starts a recording, activates a window, captures a
delivery target, changes settings, or creates an approval. Results are diagnostic
observations only and cannot be replayed as delivery authority.

The `snapshot` contains 17 fixed scalar fields: kind, PID, a four-value role,
role/subrole/enabled read statuses and integer error codes, the interpreted
secure/enabled flags, the `AXValue` and `AXSelectedText` replacement capabilities
with error codes, and the current eligibility result/reason. It does not read
`AXValue`, selected text, titles or arbitrary strings. Unknown types/statuses
and extra client-response fields fail validation. The transport projection
cannot disclose additional internal fields. Reads are bounded and the existing
15-minute endpoint lifetime still applies.

The production field policy accepts a known ordinary text role with a positive
value or selected-text replacement capability when no explicit disabled state
is reported. A writable selection range alone never establishes editability.
Actual permission failures, secure fields, foreign processes, explicit negative
editability and wholly unknown capability remain rejected. This diagnostic is
separate from the new bounded production post-send content check: its optional
snapshot stays solely in the AX owner memory and is never part of this endpoint.

## Confirm independent recipient receipt

Actor Idle/outcome=sent is not itself recipient confirmation. The recipient's
250-ms publication may still precede that actor response. After a completed
controlled run, the optional read-only diagnostic below polls for a later
sequence and post-terminal timestamp from the same recipient:

```sh
node scripts/recipient-delivery.mjs /absolute/recipient/snapshot.json /absolute/attempt.json
```

The attempt is the preserved own-run record containing recipientPid, the
last recipient sequence, endedAtUtc and the completed actor snapshot. The
reader accepts bounded owned regular files and emits only fixed diagnostic
fields, boundaries and hashes. It starts no recording and grants no target
approval. Keep an exact copy of relevant telemetry if byte-for-byte future
readback is needed: the publisher replaces its current snapshot periodically.
An [actual successful run](verification/native-delivery-success-2026-10-19.md)
separates public-sentence recognition, one-time receiver text and global-hotkey
acceptance; only the first two were proved in that controlled run.
