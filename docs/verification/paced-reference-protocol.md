# Sechs zentrale Fälle in einer warmen Runtime

Stand: 17.10.2026. `paced_reference_probe` ergänzt die bisherige Einzelfallprobe
um den vollständigen zentralen Vergleich aus §13. Hier wird das Messwerkzeug
beschrieben, noch kein abgeschlossener Latenzvergleich oder P05-Abschluss.

## Feste Eingaben und Reihenfolge

[Die sechs zentralen Fälle](../../tests/fixtures/corpus/central-reference-v1.json)
haben SHA-256 `244b2cd766874855174699f8568dbb09ae664380a98b272bf6588293f44a36b5`.
Ihre IDs und Audiohashes waren bereits vor der ersten Abnahmeinferenz in
`acceptance-reference-parameters-v1.json` festgelegt. Die unveränderten
Fallobjekte stammen aus dem 90er-Abnahmekorpus; dies sind keine sechs
zusätzlichen unabhängigen Sprachaufnahmen.

Der Adapter akzeptiert genau DE/EN × 5/10/15 s, saubere Abnahmefälle mit
eindeutigen IDs, Audioassets und nichtleeren begrenzten Referenzen. Tatsächliche
WAV-Hashes, PCM16-Mono-16-kHz-Format und Framezahlen werden vor Modellarbeit
geprüft. Alle sechs dekodierten Eingaben umfassen zusammen 960.000 f32-Samples
(3.840.000 Byte Nutzdaten); dies ist keine Gesamtspeicherobergrenze.

Ein einziger `InferenceService` und eine `MeasuredRuntime` bleiben für die
Konfiguration warm. Zuerst genau ein markierter Warm-up auf DE5, danach
30 gewertete Läufe je Fall in der Reihenfolge DE5/10/15, EN5/10/15: 181 Slots.
Keine Wiederholung oder neue Aufwärmphase zwischen Fällen. Der vorhandene
zeitgetreue Feeder, Audioowner, DSP, Controller, Inferenzpfad und die vorhandenen
Stop-/Text-Zeitstempel bleiben beteiligt.

## Fehler, Auswertung und private Ergebnisse

Ein fehlerhafter oder unerwartet leerer Lauf beendet die weitere Ausführung;
die verbleibenden Slots werden als nicht ausgeführt mit Ursache aufbewahrt.
Jeder Slot bindet Fixture-ID/-Hash, Sprache, Dauer und Laufnummer. Es entsteht
keine Statistik über einen erfolgreichen Teilbestand. Auch Warm-up-Fehler,
fremde/fehlende Slots und unbestätigter abschließender Join unterdrücken die
Gesamtstatistik.

Erst nach bestätigtem Cleanup berechnet das Werkzeug die sechs Mediane und
Nearest-Rank-p95-Werte sowie deren ungewichtetes Medianmittel `L` in ns.
Die absoluten Grenzen (je Fall Median ≤1 s und p95 ≤1,5 s) werden zusätzlich
als eigene Kriterien ausgewiesen. Ein vollständig ausgeführter Messlauf ist
kein automatisches Bestehen dieser Grenzen.

Die veröffentlichten Referenzen dienen später der unveränderten WER- und
Originalinhaltprüfung. Sie sind keine bytegenaue Modellbaseline; der Timinglauf
verlangt deshalb keine exakte Zeichenübereinstimmung. Sämtliche Hypothesen
bleiben für die gesonderte Qualitätsprüfung erhalten. Das Werkzeug behauptet
weder geprüfte Sprachqualität noch native Mikrofon-/Hotkey-/Zielübergabe,
Ressourcenabnahme oder Gesamtqualifikation.

Hypothesen und numerische Metadaten werden jeweils atomar als neue Dateien
mit Modus 0600 publiziert; vorhandene Belege werden nicht ersetzt. Das Paar
ist keine dateiübergreifende Transaktion. Ein Fehler der zweiten Publikation
wird gemeldet; die bereits publizierte erste Datei bleibt als Beleg erhalten.

## Aufrufvertrag und Parameterfreeze

Release-Build ausschließlich des Entwicklerwerkzeugs:

```sh
cargo build --release --offline --locked -p everlast-dictation --features measurement --example paced_reference_probe
```

`--print-parameters` gibt das einkompilierte Template aus. Vor dem ersten
Modelllauf muss es in eine neue Parameterdatei geschrieben und zusammen mit
Binary-, Manifest- und Modellhashes geprüft werden. Das Template bindet
Messprotokoll und beteiligte Quellen; eine abweichende Datei wird abgewiesen.
Danach lautet die explizite CLI:

```text
paced_reference_probe MODEL_PATH reference|small SIX_CASE_MANIFEST PARAMETERS_JSON NEW_OUTPUT_JSON
```

## Tatsächlicher Release-/Freeze- und Startnachweis

Werkzeugcommit `b62f86f`; Release-Build offline/locked erfolgreich in 3,18 s.
Binary SHA-256 `a7468fbd2005b5a78ec4cae8674d9630082e299d3ee88d62deaa4ebe2d0f5749`.
[Templatefreeze](../../tests/fixtures/corpus/paced-six-reference-parameters-v1.json)
SHA-256 `882314f2b0e13a3dff8b648ec3c549f86f8537364ab7f48b556f4a92944a66ba`;
[Aufruf-/Hostfreeze](../../tests/fixtures/corpus/paced-six-reference-run-v1.json)
SHA-256 `1d0b9d0d5339b32b69a5f877d73432bb8559163fc93f3e91f60d9d60a952878f`.
Beide vor Modellarbeit lokal als `1f1aa44` committed. Der unabhängige Readback
bestätigte das tatsächliche Release-Template, 39 Sourceeinträge, sechs WAVs,
ASR-/VAD-Modelle, Host-/AC-/Energiebeobachtungen und den exakten Aufruf.

Der einmalige eingefrorene Referenzlauf startete am 17.10.2026 um
14:39:57.579991 UTC mit `EVERLAST_BENCHMARK_REVISION=b62f86f`. Beim Start
gehörte ihm PID 79998; Prozessfortschritt und Betriebssystemzustand wurden
anschließend bestätigt. Private Fortschrittslogs liegen unter
`artifacts/paced-six/v1/reference.command.log`. Der Launcher wartet auf genau
diesen Prozess und schreibt erst nach dessen Ende einen Completionbeleg mit
Exitstatus und Rohhashes. Dieser Startbeleg ist kein abgeschlossener Messbericht;
L, vollständige 181 Zeilen, absolute Grenzen und Cleanup sind noch zu prüfen.
Ein Timeout der Beobachtung ist kein Grund zum Neustart.

Nachtrag: Der Prozess ist am selben Tag um 14:57:53.854355 UTC mit Exit 1
terminal geworden. Eine Chunkpublikation überschritt das eingefrorene
Jitterbudget; alle 181 Slots einschließlich 78 nicht ausgeführter bleiben
erhalten. [Fehlversuch v1](paced-six-reference-v1.md) dokumentiert Rohdaten,
beendeten Prozess und bestätigten Cleanup. Es gibt aus diesem Versuch kein L
und keinen bestandenen zentralen Vergleich. Der Prozess wird nicht neu gestartet.

## Neuer getrennter Referenzversuch v2 mit Publisher-QoS

Vor Modellarbeit am 17.10.2026 eingefroren: Werkzeugstand `fbbab45`,
[Parameter v2](../../tests/fixtures/corpus/paced-six-reference-parameters-v2.json)
und [Aufruf-/Hostbeobachtung v2](../../tests/fixtures/corpus/paced-six-reference-run-v2.json).
Binary-SHA256 `49c5991e5cc7adf39bfd4a4352177f6959a0e5fcfb4ff94e71b34c3d1490ffb3`;
Parameter-SHA256 `ec18cb5d8291c3481cde54ac77a45b8c7bb584807de427301d4875e5739f0667`.
Der Developer-Releasebuild endete erfolgreich nach 6,42 s; kein App-/DMG-Build.

### Abschluss v2: erneut ungültiger Messaufbau

Der nach Freezecommit 1f92e63 gestartete Originalprozess PID 88303 / Launcher
30830 endete am 17.10.2026 um 15:48:00.061931 UTC mit Exit 1. Alle 181 Slots
bleiben erhalten: 81 ok einschließlich Warm-up, ein invalid_setup, 99 unavailable.
DE15-Lauf21 überschritt zweimal die feste 10-ms-Grenze, trotz bestätigtem
QoS-Eintritt und -Ende. Alle Besitzer gejoint; keine Gesamt- oder Teilstatistik,
L bleibt null. [Vollständiger Fehlversuch v2](paced-six-reference-v2.md) mit
inhaltfreiem Zahlenbericht und Rohdateibindungen. Kein Neustart und keine
Grenzwertänderung; weitere Arbeit benötigt gezielte Ursacheninstrumentierung.
