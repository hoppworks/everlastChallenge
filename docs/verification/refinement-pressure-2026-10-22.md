# Text refinement under memory pressure

This change follows `b9cbe23`. The app observes the public libdispatch memory
pressure source. Warning and Critical events unload the local text worker;
Normal permits later refinement again. The observer does not allocate memory to
probe the system or infer a minimum RAM requirement.

A pressure event revokes an active refinement job immediately. The runtime keeps
the original, reports the reason and rejects late output. Recording and ASR are
not cancelled by this event. A new refinement request while pressure remains
high retains its original rather than starting another model load. Returning to
Normal does not retry a discarded request.

The native callback only signals runtime work. Model shutdown runs on the worker
owner, with the existing cancellation grace and process-reaping rules. Native
callbacks are serialized; removal disables delivery before cancelling the source,
and callback panics do not unwind through the native boundary. The source reads
its event data only inside its own handler, without retaining itself through the
handler block.

## Verification and correction

Runtime tests cover original retention, rejection of late successful output,
idle unload, repeated pressure notifications, continued recording/ASR, blocked
refinement and a later explicit request after Normal. Existing runtime tests also
exercise user cancellation, sleep, stale results and shutdown.

Independent review found an ordering race: a Warning→Normal transition after the
pressure poll could leave an old unload pending when a new job was admitted. That
unload could cancel the new job at the next poll. Admission now drains pending
unload under the same lock before submitting the job. A deterministic actor test
checks this exact ordering and the new job's still-valid cancellation token.
Removing only the correction reproduced the failure; restoring it passed.

The runtime suite passed 51 tests before the additional interleaving regression;
that regression passed separately. One existing 300-second model replay was not
run. The desktop suite passed 34 tests. Three focused native tests cover flag
priority, creation/activation/removal and panic containment. Host/native Clippy,
Rust formatting and fast repository checks passed.

`dispatch2` 0.3.1 was already present and attributed in the dependency inventory;
it is now an explicit native dependency. The implementation follows the public
[memory-pressure source contract](https://github.com/swiftlang/swift-corelibs-libdispatch/blob/main/dispatch/source.h)
and Apple's [Critical event guidance](https://developer.apple.com/documentation/dispatch/dispatch_memorypressure_critical).
Sources are bound in the [manifest](refinement-pressure-sources-2026-10-22.json).

## Evidence limits

Native tests create the real dispatch source but inject pressure flags only at
the Rust delivery boundary. They do not prove receipt of real system-pressure
notifications, quantify memory released, measure a pressured GPU operation or
qualify ASR/model coexistence. No artificial system-wide pressure was generated.
The app relies on delivered OS events; source creation is not a measurement of
the current pressure level. Final bundle, coexistence and quality checks remain
separate. No performance or distribution approval follows from these tests.
