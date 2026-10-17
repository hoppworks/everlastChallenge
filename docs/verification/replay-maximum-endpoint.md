# Letzter Replaychunk an der Fünf-Minuten-Grenze

Stand: 17.10.2026; Ausgangsrevision `4893873`.

## Fehlerreproduktion vor der Korrektur

Eine beschleunigte Diagnose verwendet den echten CaptureHandle, RecordingOwner,
CapturePort und abschließenden DSP-Drain. Nur der letzte 20-ms-Chunk mit
320 Frames wird repräsentiert: Der Ownerstart wird um 300 s rückdatiert,
die Feedclock separat um 299,980 s verschoben. Frühere 299,980 s Audiodaten
werden nicht simuliert. Diese Probe ist keine reale 300-s-Laufzeitabnahme.

Bei kontrollierter Reihenfolge Publikation vor Maximum bleiben alle 320 Frames
erhalten. Bei Maximum vor Publikation bleibt das Ergebnis leer, obwohl die
Nachpublikationszeit des Diagnoseversuchs 21.546.375 ns beträgt: 1.546.375 ns
Abweichung vom geplanten letzten Chunk, innerhalb des festen 10-ms-Budgets.
Der Owner meldet tatsächlich `maximum: true`.

Die erste Diagnoseassertion erwartete 320 Frames auch nach schon beendetem
Owner und schlug mit 0 fehl. Sie lokalisiert die Gategrenze, ist aber keine
zulässige Fixanforderung: Eine geschlossene Quelle darf nicht wieder geöffnet
werden. Die fixfähige Regression muss deshalb einen schon begonnenen wartenden
Chunk über den Maximumzeitpunkt hinweg kontrollieren. Diese beiden Fälle sind
getrennt zu halten.

Die Hypothesen wurden in folgender Reihenfolge bewertet:

1. Der Owner schließt die Eintrittssperre, bevor der wartende Chunk publiziert;
   eine bereits laufende Publikation gehört hingegen zur bestehenden Drainbarriere.
2. Der abschließende Drain verliert bereits erfolgreich publizierte Samples.
   Der erfolgreiche Kontrollfall spricht dagegen.
3. Framebudget oder DSP-Flush verursachen den Verlust. Nur 320 Frames und der
   erfolgreiche Kontrollfall trennen dies vom Wallclockmaximum.

## Fixfähige Regression

`waiting_final_clockfeed_chunk_survives_the_actual_owner_deadline` beginnt
mit einem Owner, dessen reale 300-s-Deadline 20 ms in der Zukunft liegt.
Ein nur für Tests kompilierter Wartehook kontrolliert im echten Clockfeed den
Übertritt der Deadline: Erst nach geschlossenem Gate folgt die Publikation.
Der Feed wurde bereits vor der Deadline betreten. Vor der Korrektur war dieser
Test zweimal rot (0 statt 320 Frames); das lokale Log bleibt unter
`artifacts/replay/endpoint-owner-red.log` mit Modus 0600 erhalten.
`admitted_tail_is_drained_but_publication_after_completed_owner_stays_closed`
prüft daneben die erlaubte Vorher-/Nachhergrenze.

## Synthetischer Langlauffall

Vor dem Langlauf erzeugte lokale, unversionierte PCM16-WAV:
`artifacts/replay/endpoint-300s-pcm16.wav`. 4.800.000 Frames, 16 kHz Mono,
exakt 300 s, abwechselnd -1000 und +1000 als i16. SHA-256:
`adb18cd498070629cf8311da34ff6d5cf85e9330284838bda9b8eab67679703a`.
Die Datei prüft Übertragung und Samples, keine natürliche Sprache oder Qualität.

## Geltungsbereich der Reservierung

Der Schutz gilt für einen Chunk, dessen Reservierung schon vor dem Maximum-Close
angenommen wurde. Publikation bleibt auf oder nach dem absoluten Zielzeitpunkt;
Record-Limits und Samplebudget werden nicht verlängert. Ein bereits vollständig
geschlossener Port nimmt nichts mehr an. Falls der Producer zwischen vorletzter
Publikation und letzter Reservierung so lange nicht ausgeführt wird, dass die
Reservierung erst nach dem Maximum erfolgt, kann der letzte Chunk weiterhin
fehlen, selbst wenn seine geplante Publikation nur wenige Millisekunden verspätet
wäre. Das Jitterbudget allein beweist daher keine universelle Endpunktgarantie.
Der exakte Input-/Output-Samplevergleich bleibt zusätzlich zwingend; solche
Läufe dürfen nicht als erfolgreiche Übertragung zählen.

## Korrektur und Prüfstand

Der nur mit `measurement` kompilierte Borrow-Guard reserviert vor dem absoluten
Warten eine Publikation. Die native Publikation und der Guard benutzen denselben
extrahierten Ring-Schreibkörper. Es werden vor dem Zielzeitpunkt keine Samples
geschrieben. Auch bei einer künstlich in der Zukunft liegenden Feedclock beginnt
die Reservierung frühestens 20 ms vor dem jeweiligen Chunkziel. Damit bindet sie
die Abschlussbarriere nur an den laufenden Chunk, nicht die ganze Aufnahme.
Tatsächliche OS-Deschedulingzeiten können diese Wartephase verlängern; daraus
folgt keine harte Echtzeitgarantie.

Der Wait-Hook existiert ausschließlich im Testbuild. Der Owner wartet in seiner
Aktivierung auf diesen Hook, damit die Regression zuverlässig einen schon
begonnenen Feed statt zufällig einen bereits geschlossenen Port prüft.
Stop-/Unwind-Fälle beobachten `CLOSED + 1` und keinen Finished-Event, solange
die Reservation lebt. Nach normalem Abschluss oder Unwind fällt die Barriere;
weitere Reservierungen bleiben abgewiesen. Die Ownerlimits sind unverändert.

Enges RED/GREEN-Kommando:

```sh
cargo test --locked --offline -p everlast-dictation --lib --features measurement audio::replay::tests::waiting_final_clockfeed_chunk_survives_the_actual_owner_deadline -- --nocapture
```

Das lokale vollständige Audiotestlog liegt in
`artifacts/replay/endpoint-owner-green.log` (31 Feature-Audiofälle).
Standardcheck und npm test grün: 170 Host-, 197 Frontend- und 42 Pythonfälle,
übrige Workspaceprüfungen ebenfalls erfolgreich. Feature-Workspace-Clippy
alle Targets warnungsfrei, Feature-Workspace mit 198 Hosttests grün; Audio-Probe acht Tests
grün. Die sieben gezielten Runtime-Messtests bestehen weiterhin.
Der unabhängige kombinierte Code-Review hat keine offenen Befunde.

## Tatsächlicher 300-s-Lauf

Nach Abschluss sämtlicher Builds/Tests und ohne parallele Prüfungen lief genau
ein Release-Pilot mit der oben festgelegten synthetischen WAV:

```sh
cargo build --release --offline --locked -p everlast-dictation --features measurement --example paced_audio_probe
EVERLAST_BENCHMARK_REVISION=4893873+endpoint-reservation-v1 target/release/examples/paced_audio_probe artifacts/replay/endpoint-300s-pcm16.wav
```

Stdout wurde exklusiv in `artifacts/replay/endpoint-300s-v1.json` erzeugt,
Stderr separat in `artifacts/replay/endpoint-300s-v1.stderr.log`; beide Modus 0600.
Prozessende 0. Berichts-SHA-256:
`bc60875602cf37e295f6637562fa44289630a6fc3197be56849f0b93911c3ce2`.
Release-Binary-SHA-256:
`eb879153627685f941bc155395920e2c1fc375824ddff0d04d1ee9c8cba52e3d`.
Der Bericht enthält zusätzlich alle sechs Audio-/Clock-/Probe-Quellhashes.

| Nachweis | Tatsächlicher Wert |
|---|---:|
| Eingangsframes und identische Ausgabesamples | 4.800.000 |
| Publikationschunks | 15.000 |
| Frühe oder außerhalb Budget liegende Chunks | 0 |
| Größter Chunkjitter | 4.322.250 ns |
| Letzte Publikation nach t0 | 300.000.014.750 ns |
| Geplanter Stopp nach t0 | 300.000.000.000 ns |
| Tatsächlicher Stopaufruf nach t0 | 300.000.015.959 ns |
| Beobachtetes Finished nach t0 | 300.001.537.667 ns |

Genau ein Started- und ein Finished-Event, genau ein expliziter Stopaufruf,
identischer Capture-/Producerstart, kein Audio-/Beobachtungs-/Joinfehler und
bestätigter Join. Der Probe weist `L: null` und `tText: null` aus. Sein Bericht
enthält keinen booleschen Maximum-Grund des Finished-Events; dieser wird hier
nicht aus dem Timing als gesondert gemessen behauptet.
