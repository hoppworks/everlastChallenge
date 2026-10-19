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
