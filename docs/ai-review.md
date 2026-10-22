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
