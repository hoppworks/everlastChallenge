# P05: Engine-RTF und Empty am echten Modellpfad geprüft

Stand:17.10.2026. Code c394d9e, Freeze65e9daa, Releasebinary f813174b….
Ursprünglicher Launcher94497 terminal; Local PID10556
18:00:22.217800–18:00:42.137459 UTC, danach Empty PID10617 bis18:00:53.887482.
Beide Prozesse Exit0. Je ein ausdrücklich markierter Warmup und ein Messlauf;
vier Zeilen statusok, beide Runtime-Joins bestätigt. Keine parallelen Builds,
Tests oder Modelle und kein App-/DMG-Neubau.

| Gewerteter Einzel-Smoke | Local8,192s | Empty5s |
|---|---:|---:|
| Übergebene Engineframes | 131072 | 80000 |
| Tatsächlicher Engineaufruf | 1007173917ns | 276041ns |
| Engine-RTF | 0,12294603479 | 0,0000552082 |
| Stop bis angenommenem Abschluss | 1027715375ns | 28425375ns |
| Textzeitpunkt vorhanden | ja | nein |

Local stimmt in beiden vollständigen Rohtexten bytegenau mit der zuvor
bestätigten privaten56Byte-Baseline überein; keine Ausgabe des Wortlauts.
t_text und t_terminal sind jeweils derselbe Annahmezeitpunkt. Beim ausdrücklich
erwarteten Empty ist der Text leer, t_text/Stop-bis-Text/L bleiben null;
der gesonderte Terminalzeitpunkt und die wirkliche Enginezeit bleiben erhalten.
Der Empty-Fall ist ein synthetisches Nullsignal. Seine Enginezeit umfasst die
Energie-/Leereprüfung und ist kein Nachweis für Whisper-Erkennungstempo.

Alle1320 Publikationen mit festen20ms-Abschnitten plus Rest geprüft, kein
Chunk vor seinem Sollende. Maximale Abweichung227833ns bei Local und205541ns
bei Empty, innerhalb10ms. QoS-Eintritt/-Ende für alle vier Zeilen bestätigt,
keine Fehler. Engine-RTF rechnerisch mit Dauer und übergebenen16kHz-Frames
reproduziert; beide Seiten berichten vollständige Messabdeckung1/1.

Die1,027715375s Stop-bis-Text des einzelnen nichtzentralen Local-Smokes sind
keine bestandene1s-Mediangrenze. Keine belastbare p95-Verteilung aus einem Wert,
kein sechsfaches L und keine zehnfache RTF-/P05-/P06-Gesamtqualifikation.
Der [inhaltfreie Bericht](paced-rtf-smoke-v1.json) bindet sieben private600-Dateien,
Freeze/Binary/Parameter, alle vier Zeilen und ihre Feedzusammenfassungen.
Die vollständigen Publikationsdaten und Nutzerdiktate bleiben lokal erhalten.
