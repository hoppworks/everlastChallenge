# KI-Nutzung und technische Prüfung

KI-Werkzeuge wurden für Rust-/TypeScript-Implementierung, Testentwurf,
Fehlerdiagnose, Dokumentation, Designvarianten und Code-Reviews eingesetzt.
Die Änderungen wurden mit automatisierten Checks und gezielten nativen Tests geprüft.

## Beispiele

- [Technische Korrekturen](verification/release-fixes-2026-10-21.md): Fehlerbefunde,
  Regressionstests und überprüfte Laufzeitverträge.
- [Palette](verification/palette-2026-10-21.md): simulierte Browserzustände,
  native Datenanbindung und Bedienungstests sind getrennt beschrieben.
- [Entscheidungen](engineering-decisions.md): Begründungen und verworfene Alternativen.
- [Messungen](performance.md): revisionsgebundene Ergebnisse mit Grenzen.

Automatisierte Tests decken die geprüften Verträge ab, nicht jede Mikrofon-/App-
Kombination. Agentenreviews bleiben fehlbar. Historische Ergebnisse gelten nur
für ihre angegebenen Quellen, Modelle und Messbedingungen.

## Paket- und Werkzeugmigration 0.2.0

Codex unterstützte die Bibliotheksanpassungen und TypeScript-Migration. Ein
unabhängiges kombiniertes Code-Review prüfte Anforderungen, Fehlerpfade und
Projektregeln. Dabei wurden ein zweiter ungebundener Dateilesezugriff, zu
weiche Feldtypprüfung, unvollständige Fall-ID-Prüfung und fehlende Eingabehashes
korrigiert. Die TLS-Initialisierung wurde zusätzlich anhand der fehlschlagenden
Transporttests berichtigt. Gezielte Regressionen, der gemeinsame Softwarelauf
und die tatsächlich erreichten Abdeckungsgrenzen stehen im
[technischen Prüfbericht](verification/tooling-and-packages-2026-10-22.md).
Das Review ersetzt diese ausführbaren Nachweise nicht.

Der [native Prüfbericht 0.2.0](verification/release-0.2.0.md) ergänzt die
beobachtete Einstellungsübernahme, Aufnahme, Modellproben und Paketprüfung.
Er grenzt die fehlende neue Hotkey-/Empfangsprüfung ausdrücklich ab.

Beim lokalen CI-Lauf wurde außerdem ein zeitabhängiger Cache-Sperrenfehler
gefunden. Eine deterministische Reproduktion mit einem eigenen Kindprozess
scheiterte vor der Korrektur und bestand danach. Der
[Nachweis zur Sperrenlebensdauer](verification/cache-lock-lifetime-2026-10-22.md)
trennt diesen belegten Mechanismus von der nicht vollständig aufgezeichneten
ursprünglichen Prozessabfolge.

## Deterministische Textprüfung

Codex unterstützte die reinen Rust-Verträge und Tests für begrenzte Textänderungen.
Eine unabhängige Review entdeckte Eingriffe in Links, unvollständig erfasste Zahlen,
verwaiste Kommas und einen fehlenden positiven Selbstkorrekturfall. Die Befunde
wurden mit fehlschlagenden Regressionen reproduziert und korrigiert. Der
[Prüfbericht](verification/refinement-core-2026-10-22.md) trennt diese Regeln
ausdrücklich von noch ausstehenden Modell- und Integrationsnachweisen.

Für das getrennte Worker-Protokoll wurden ebenfalls Tests vor der jeweiligen
Implementierung und eine unabhängige Review eingesetzt. Ein negativer Test
zeigte, dass eine parameterlose Serde-Variante zusätzliche Felder akzeptierte;
eine explizite leere Struktur verhindert dies. Die Protokolltests prüfen
Speichergrenzen, Teilrahmen, Versionen und unbekannte/doppelte Felder in
kontrollierten Speicherstreams; sie ersetzen keine Prozess- oder Modellprüfung.

## Isolated local text worker

Codex assisted the MLX worker, bounded transport, model snapshot and focused
regression tests. Independent review identified blocked control paths and two
model-file ownership issues; targeted follow-up checked their corrections.
A real model smoke test still missed two required corrections. The
[worker report](verification/refinement-worker-2026-10-22.md) preserves that
limitation and does not claim integrated product or quality acceptance.

Rust-generated source options and their selection decoder were also developed
with AI assistance. Independent review produced three concrete boundary/limit
corrections, each backed by a regression. The
[candidate report](verification/refinement-candidates-2026-10-22.md) also records
an unresolved model-selection failure reproduced with the upstream runtime;
software test success is not presented as model-quality acceptance.

The host process supervisor was also AI-assisted and independently reviewed.
A real filesystem regression reproduced and corrected an inaccurate cleanup
error flag. Twelve child-process tests cover bounded transport, lifecycle and
rejection behavior; the [supervisor report](verification/refinement-supervisor-2026-10-22.md)
records source hashes and explicitly excludes GPU, delivery and crash-recovery
acceptance.

The local text worker now uses grammar-constrained generation through additive
MLX logits-processor entry points. AI-assisted tests cover token masking,
processor failure and exact decision counts through the real GPU/text stream.
Independent review found no concrete code blocker and prompted a correction to
an inherited tokenizer-version comment. The
[structured-output report](verification/refinement-structured-2026-10-22.md)
separates format enforcement from the still-failing model-quality criteria.

A later diagnostic exposed acceptance of a duplicated sentence-ending point.
Independent review found a mixed-edit bypass in the first correction. Both were
reproduced with failing public tests before the final source-provenance guard.
The [punctuation report](verification/refinement-punctuation-2026-10-22.md)
documents the corrected composition rule and its limited linguistic scope.

The fixed text-model installer was AI-assisted and independently reviewed.
The review caught a regression in the shared cache-lock namespace: the existing
Silero ID contains dots. A test using the actual VAD definition reproduced the
failure before the correction. The combined model test run also exposed a
nonblocking socket error in the new HTTP fixture; accepted sockets now use
bounded blocking I/O. The [installer report](verification/refinement-installer-2026-10-22.md)
keeps both failures and distinguishes local fixture coverage from the still
outstanding full download, setup and runtime acceptance.

The asynchronous text-worker owner was also AI-assisted. Failing process tests
preceded the initial owner, unload behavior, completion revocation, cleanup-error
propagation and clean idle-exit recovery. Independent combined review inspected
the service and real-child fixtures without repeating builds or model runs and
found no concrete blocker. The [owner report](verification/refinement-owner-2026-10-22.md)
limits this evidence to the service boundary; controller and UI integration are
still outstanding.

The controller's refinement state and retained-transcript contract were developed
with AI assistance and independently reviewed. Regressions exposed wrong-original
and wrong-language validation authority, stale clear behavior and an empty final
result retaining an intermediate text. The corrected typed result carries its
source and language. A paused-actor test checks that client cancellation revokes
the text-worker token directly. The [controller report](verification/refinement-controller-2026-10-22.md)
separates these contracts from the still-missing product owner hookup and UI.

The runtime-to-owner adapter was AI-assisted and independently reviewed. Runtime
tests exercise retained originals, delivery gating, cancellation, sleep, stale
results and independent cleanup. Review found an idle-crash race that could label
an internal failure as user cancellation. The race was reproduced with a real
process; its final regression fixes the exact state ordering deterministically.
Fatal closure now takes precedence over an unconsumed completion. The
[runtime report](verification/refinement-runtime-2026-10-22.md) records the
integration boundary; product setup, UI and model-quality acceptance remain open.

Text-model provisioning and preference migration were AI-assisted and independently
reviewed. Review found that an invalid optional runtime directory prevented ASR
startup; a failing then passing test now covers ready ASR with a foreign file or
symlink at that path. A separate regression exposed inconsistent legacy decoding
in preference rollback. The shared decoder now preserves the original bytes on
failed saves. The [provisioning report](verification/refinement-provisioning-2026-10-22.md)
separates these host contracts from UI, bundle and model-quality acceptance.

The refinement UI and generated IPC contracts were AI-assisted and independently
reviewed. Review caught an incorrect assumption that idle results retain an active
session ID. Failing UI tests were changed to real idle snapshots with a null ID;
the correction retains completed-result metadata and resets it on the next
successful start. The [UI report](verification/refinement-ui-2026-10-22.md) records
software tests and a separate visual review of simulated states. Neither proves
packaged inference, model quality or receipt by another application.

Bundled text-worker packaging was AI-assisted and independently reviewed. Review
found that the existing local signer rejected the new helper; a failing regression
preceded the allowlist and nested-signing correction. The
[packaging report](verification/refinement-package-2026-10-22.md) records focused
tests and a real relocated-worker smoke check, separately from model quality,
application delivery and notarization.

Text-worker crash recovery was AI-assisted and independently reviewed. A failing
regression showed an abandoned snapshot remaining after startup. Shared host/child
leases now protect live snapshots; an exclusive lease permits later recovery.
The [recovery report](verification/refinement-recovery-2026-10-22.md) separates
controlled process-crash evidence from normal native MLX startup/shutdown and
from the still-unqualified GPU-abort and memory-pressure cases.

Memory-pressure handling was AI-assisted and independently reviewed. Review found
that an old pending unload could cancel a newly admitted job after pressure had
already returned to Normal. The corrected admission order has a deterministic
regression, including a failing run with only the correction removed. The
[pressure report](verification/refinement-pressure-2026-10-22.md) distinguishes
runtime contracts and native source construction from actual OS-pressure delivery,
GPU behavior and memory measurements.

The atomic two-stage edit contract was AI-assisted and independently reviewed.
It keeps the existing validator intact, applies it to each stage's exact source
and binds the final result back to the original dictation. The
[staged-validation report](verification/refinement-stages-2026-10-22.md) states the
core-only scope; this contract alone does not qualify the model or application.

The two-stage worker/host integration was AI-assisted and independently reviewed.
The review corrected punctuation spans adjacent to preserved quotes. Native model
verification then exposed literal control-token encoding and long-grammar
compilation failures; deterministic regressions preceded both corrections. The
[pipeline report](verification/refinement-pipeline-2026-10-22.md) distinguishes
these checks from full product quality, delivery and performance acceptance.

An independently authored corpus exposed failures that the original text cases
missed. AI-assisted diagnosis and independent review corrected incomplete
self-correction spans and a predicate-consuming numeric option. Positive tests
also preserve valid spelled-number, time, repeated-name and repeated-quantity
corrections. The [context report](verification/refinement-context-2026-10-22.md)
records the structural limits; finite case results do not establish arbitrary
linguistic correctness.

Real host-supervisor verification also found that default temporary-directory
permissions prevented native model startup. The host now requests `0700` at
creation; a failing/passing supervisor regression and a real model
cancellation/restart check verify this path. The context report records sampled
MLX/Metal activity separately from final application and memory-pressure gates.

Further AI-assisted system checks used the signed packaged worker for hard child
loss, abrupt host loss and subsequent recovery, plus serial coexistence with the
real hybrid ASR. A network-denied application run exercised recording, ASR and
refinement. The [system observation report](verification/refinement-system-2026-10-22.md)
records the successful paths and the limits: recipient focus was not established
for delivery, and the OS pressure simulation failed for lack of permission.
Neither is counted as a pass; finite model cases remain separate from human
linguistic acceptance and comparative performance.

An AI-assisted comparison of the implementation plan with the verification
commands found missing explicit coverage scope for the new crates and extracted
host modules. The [coverage report](verification/refinement-coverage-2026-10-22.md)
records the added per-file checks, their failing/passing checker tests and two
asset-boundary tests. These line measurements do not replace real-model or
human-language acceptance.

The [long-correction report](verification/refinement-long-corrections-2026-10-22.md)
records an AI-assisted number-word boundary fix and a more explicit correction
prompt. Independent review found ordinary-negation and numeric-subject errors;
failing public regressions reproduced both before correction. A fresh production
worker and independent Rust validator exercised 66 text cases. Two erroneous
capitalization expectations were corrected transparently after diagnostic output
had been observed. The record preserves this limitation and does not substitute
automatic matching for human linguistic review or application-level acceptance.

The [activation report](verification/refinement-activation-2026-10-22.md) records
AI-assisted native setup activation. Independent review found a queued-save
cancellation race and missing publication of early errors; failing regressions
and corrected queue/operation tests cover both. A separate negative control
checks that setup admission uses current service state rather than delayed UI
state. Software evidence is distinguished from pending packaged interaction and
GPU startup verification.

The [startup-test report](verification/refinement-startup-probe-2026-10-22.md)
records AI-assisted setup probing with the actual bundled worker. Independent
review identified a file-only readiness bypass; a failing native settings test
preceded the separate probe-success flag and corresponding UI/IPC checks. Native
observations include a synthetic inference, cancellation and setup alongside a
warm worker, with child exit and scratch cleanup verified. These observations
do not establish full application memory bounds or comparative performance.

The [numeric-context report](verification/refinement-numeric-context-2026-10-22.md)
records an AI-assisted prompt correction for an omitted quantity correction in a
long ASR transcript. A rejected prompt candidate introduced a hesitation-removal
regression. The final production worker and independent Rust validator matched
all 68 declared outputs, including the 66 unchanged previous results. Independent
review retained the occurrence instruction for repeated spans. The change does
not alter model selection, serialization, validation or execution flow; linguistic
review, packaged delivery and performance remain separate requirements.

The [packaged refinement checks](verification/refinement-packaged-2026-10-22.md)
record AI-operated native target observation and a separate network-denied app
run. The first stop attempt was rejected on focus loss and is retained. Offline
processing passed, but the isolated launch lacked Accessibility authorization;
these two observations are not presented as a single offline delivery test.
