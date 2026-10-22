# Packaged local refinement checks

The locally signed Apple Silicon bundle built from `e31ff13` was copied outside
the repository with its signature metadata preserved. Bundle inventory,
architecture, linkage and signatures passed verification. The
[record](refinement-packaged-2026-10-22.json) binds the executable, worker, Metal
library, source tree and complete 422-commit history. It is not a notarization
or public-release approval.

The qualified raw worker and the packaged worker were compared after removing
signatures from disposable copies. The remaining difference was the virtual
allocation size of the `__LINKEDIT` segment, which changed with signing. All
other bytes matched. Both values exceeded the actual segment file size and were
page-aligned; the record retains both values and the normalized hash.

## Actual target receipt

A native shortcut driver started and stopped a real recording of the synthetic
long German fixture through BlackHole. The app transcribed and refined it and
inserted the result into a separate native text field. The received text contains
`am Freitag um 9 Uhr` and `Die geplante Menge beträgt 16 Kartons.`; the abandoned
weekday and quantity were absent. The native observer retained 9,616 observations
and a clean terminal record. The first text observation had the recipient in
focus. The complete received text and hashes are in the record.

The first stop attempt was rejected before dispatch because the recipient had
lost focus. After restoring that window, a second attempt stopped the recording.
The failed attempt remains recorded. This establishes functional receipt for one
case, not comparable stop timing or a latency measurement. The raw ASR original
was not captured separately in this run; the independent
[text-quality checks](refinement-numeric-context-2026-10-22.md) cover that boundary.

## Processing with network access denied

A separate launch of the same signed bundle used this test-only profile:

```scheme
(version 1)
(allow default)
(deny network*)
```

A loopback socket control connected outside the profile and received
`PermissionDenied` inside it. Read-only policy checks on the live app and its
live child text worker both denied `network-outbound`; an unsandboxed control
allowed it. The private diagnostic uses the sandbox SPI declaration retained by
[WebKit](https://github.com/WebKit/WebKit/blob/main/Source/WTF/wtf/spi/darwin/SandboxSPI.h),
not a product dependency.

With models already installed, the app recorded the short fixture, produced
`Wir treffen uns am Montag, nein, am Dienstag um 9 Uhr.` and refined it to
`Wir treffen uns am Dienstag um 9 Uhr.`. Both texts were observed in the native
app UI. The app and child exited cleanly afterward; the built-in microphone and
normal application launch were restored.

This direct sandbox launch did not retain Accessibility authorization, so it
could not prove automatic insertion under network denial. The normal-launch
receipt above and this offline processing test are separate observations. Raw
audio files are private authored fixtures; their hashes identify them but do not
make these recordings independently reproducible from this record alone.
