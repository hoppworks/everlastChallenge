# Zeitgetreue Einspeisung vor Ring und DSP

Stand: 17.10.2026, Basis `d661cd6`. Implementiert ist das Eingangs-Fundament
für §13 des Plans. **Controller-Zeitstempel, Inferenz und vollständiges L sind
noch nicht angeschlossen.** Der neue Probe schreibt deshalb L und tText als null.

## Ein produktiver Audioowner, zwei Quellen

CaptureHandle verwendet einen gemeinsamen RecordingOwner für native Aufnahme
und ausdrücklich aktivierte Entwicklereinspeisung. Unverändert beteiligt sind
CapturePort/Ring, AudioProcessor/Resampler, RecordingLimits, Fortschritts-Slot,
Stop, abschließender Drain und Flush sowie die drei reservierten Lifecycle-Slots.
Die native Quelle behält CPAL-Setup und OwnedStream-RAII. Replay wählt kein
Gerät, fordert keine Rechte an und öffnet keinen Mikrofonstream.

Die Schnittstelle ist ausschließlich mit Cargo-Feature `measurement` verfügbar:
CaptureHandle::start_replay liefert einen normalen CaptureHandle; ein begrenzter
Ready-Kanal übergibt genau einen CapturePort, Format und das tatsächliche
Started-Instant. Der Producer bleibt vom DSP-Owner unabhängig. Full oder
Disconnected beim Ready-Kanal wird zum terminalen Fehler statt zum Blockieren.
Der normale App-Build aktiviert dieses Feature nicht; es gibt keinen IPC-Weg
zum Einspeisen von Dateien.

Ein gemeinsamer RecordingSource-RAII-Guard schließt die Publication-Gate,
zerstört die Quelle und wartet laufende Publikationen ab. Erst danach darf der
finale Drain beginnen. Dies gilt auch für Rust-Unwinding im Owner. Der Review
fand diese zusätzliche Panic-Lücke im ersten Replay-Kandidaten: Ein bereits
übergebener Port blieb offen. Ein tatsächlich roter Test mit gehaltenem Port
prüft nun CLOSED nach der injizierten Activate-Panic und Workerfehler beim Join;
spätere Publikationen bleiben geschlossen. Die vorhandene native Setup-RAII
wird dadurch nicht ersetzt. Keine Ausnahmebehandlung über eine FFI-Grenze und
kein harter Backendabbruch wurden ergänzt.

## Vorab festgelegte Audiozeitachse

ReplaySchedule zählt Eingangsframes, nicht interleavte Samples. Er akzeptiert
nur die sechs unterstützten Raten, 1–8 Kanäle und 1 Frame bis 300 Sekunden.
Ein konstanter Iterator liefert 20-ms-Chunks plus den unveränderten letzten
Rest; höchstens 15.000 Chunks. Zeitpunkt ist der exklusive Endframe geteilt
durch Eingangsrate. Integer-ceil auf Nanosekunden verhindert frühe Rundung,
auch bei einzelnen Restframes mit 44.100 Hz.

Die Jittertoleranz ist **10 ms**, vor dem Pilot festgelegt und nicht angepasst.
Ein einziges frühes Frame oder ein verspäteter Chunk außerhalb dieses Budgets
macht den Messaufbau ungültig. Alle Veröffentlichungsdaten bleiben erhalten.
Vor und nach CapturePort::publish werden relative monotone Zeitpunkte erfasst;
maßgeblich für die späteste Veröffentlichung ist der Zeitpunkt danach.
Der Stop erfolgt nach dem letzten Chunk; auch seine Abweichung wird geprüft.

Der Producer wartet ausschließlich auf absolute t0-basierte Audiozeit, niemals
auf Ringplatz, DSP, Decoder oder Segmentabschluss. Überlauf und verlorene Samples
werden als Fehler erhalten. Die finale Wartefolge schläft höchstens 1 ms am
Stück und spinnt in den letzten höchstens 250 µs bis zur Deadline. Dieser
CPU-Aufwand gehört zum Messaufbau; er ist keine kostenlose oder allgemein
jitterfreie Echtzeitgarantie.

## Fehlerberichte und Tests

Der Probe liefert nach Initialisierung auch bei Ready-/Format-/Finishfehlern
einen strukturierten Run zur JSON-Ausgabe. Ein Beobachtungstimeout verwirft keine
schon aufgezeichneten Chunks. Join wird nur bei beobachtetem is_finished
aufgerufen; sonst ist joinUnverified ausdrücklich wahr, und Drop fordert Stop an.
Die beim Review gefundene frühere Timeout-Datenverluststelle ist mit echter
Capturepipeline und Null-Beobachtungsbudget reproduzierbar geprüft. Das Budget
läuft deterministisch vor dem Drain ab; Publikationsliste und Fehler bleiben
serialisierbar erhalten. Weitere Probetests prüfen ungültige Samples und einen
nicht auf 20 ms passenden letzten Frame.

Prüfstand am integrierten Kandidaten:

- 23 Audiotests mit measurement: acht Replay-/Ownerfälle, fünf Zeitplantests,
  zehn bestehende Audiotests.
- Acht Probe-/Eingabetests einschließlich der drei neuen Runnerfälle.
- Standard-Check und npm test grün: 170 Hosttests, 197 Frontendtests und
  42 Python-Qualitätstests; übrige Workspaceprüfungen ebenfalls grün.
- Striktes Workspace-Clippy und gesamte Workspace-Tests mit measurement grün:
  183 Hosttests. Format-, Vertrags- und Secretprüfung grün.

```sh
cargo test --locked -p everlast-dictation --features measurement --lib audio::
cargo test --locked -p everlast-dictation --features measurement --example paced_audio_probe
cargo build --locked -p everlast-dictation --features measurement --example paced_audio_probe
EVERLAST_BENCHMARK_REVISION=d661cd6+paced-input-3e154b8f target/debug/examples/paced_audio_probe artifacts/replay/neue-aufnahme.wav
```

## Tatsächliche Piloten und unveränderte Fehlerhistorie

Die bereits autorisierte private WAV hat 131.072 Frames bei 16 kHz Mono,
8,192 Sekunden; kein Audio und kein Wortlaut werden committet. Alle drei
lokalen Berichte bleiben unverändert unter artifacts/replay erhalten.

| Lauf | Aufbau und Ergebnis | Bericht-SHA256 |
|---|---|---|
| paced-audio-v1.json | Erste lange Sleep-Intervalle; versehentlich parallel gestartete Builds/Tests. Audio identisch, acht Timingverletzungen, maximal 25,792833 ms. **Ungültig**, Exit 1; nicht ausgesondert. | `c1b9c865c0b309b6b6ad12ff8dafef9cd29315fc3d97465196f89a552f1c90ce` |
| paced-audio-v2-idle.json | Noch gleiche lange Sleep-Strategie; als ruhig beabsichtigt, aber ein zuvor gestarteter Agent-Cargo-Batch überlappte den Anfang. Audio identisch, null Verletzungen, maximal 9,998125 ms. Kein isolierter Beweis für die Ursache von V1. | `bd72b5c5eb9f94d63ebedde064aa1e16f52659182b24e8e2d7e91f722184387d` |
| paced-audio-v3.json | Finale kurze Sleep-/Spinfolge und korrigierte RAII-/Berichtspfade; nach terminalen Checks/Builds, ohne parallel gestartete Builds/Tests. **Bestanden**, Exit 0. | `bae190e0fd0c3c4ec6ff4219d12c99af34f1a16addba3b7d27f2e2bacfee9b63` |

V3 enthält genau 410 Publikationen, keine frühe und keine über dem Budget.
Maximale Abweichung nach Publish: **0,290916 ms**. Geplantes Ende:
8.192.000.000 ns; Stop-Anforderung: 8.192.179.625 ns; beobachtetes Finished:
8.196.787.125 ns. Started-Clock/Format stimmen überein, ein Stop und ein Finished,
Samples vollständig und exakt identisch, Join erfolgreich, keine Audio- oder
Beobachtungsfehler. Das ist ein einzelner Entwickler-Pilot im Debug-Build,
keine statistische Latenzqualifikation oder universelle Jitterzusage. Da
Wartefolge und Lastbedingungen gemeinsam wechselten, wird keine isolierte
kausale Effektgröße der Wartefolge aus V1–V3 behauptet.

Die fünf tatsächlich eingebetteten Quellhashes in V3:

| Quelle | SHA256 |
|---|---|
| audio.rs | `3e154b8f852885c0ffea0202ea86aa7493026713db8d65bd891acdfb153107af` |
| audio/replay.rs | `f5c45f69da2624bbdf40e1f75661405f0e3394db60b3879fb2a39efc697728a6` |
| audio/replay/schedule.rs | `8ed858523fcffd5804d93829c0cec40a21ba84df418c6e4ac67a4cde5176191b` |
| examples/paced_audio_probe.rs | `c9cccfcd327f76143d875505eedc1e5a22b5a6db924397f1b2b2e9f33636ad80` |
| examples/paced_audio_probe/runner.rs | `b17247e5bbb3b1b7a677060f1447dffbe52b6b51d9ff6679b1c9a803c3952f14` |

## Nächster erforderlicher Teil

Der Runtime-Adapter muss diese Quelle in denselben Controller aufnehmen,
t_stop am Host vor der Queue und t_text direkt bei finaler Textannahme erfassen,
und Freizustand/Reset vor warmen Läufen prüfen. Dann folgen vollständige
Konfigurations-/Variantenmetadaten, ein Warm-up und 30 gewertete Läufe pro
zentralem Fall sowie die Statistik gemäß §13. Dieser Probe allein liefert L nicht.
