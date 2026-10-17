# Ergänzender ASR-Nachweis mit ausdrücklich synthetischer Sprache

Stand: 17.10.2026. Drei lokal mit macOS `say` erzeugte Sprachdateien ergänzen
die vorhandenen natürlichen Korpusfälle. Es sind keine menschlichen Aufnahmen.
Die Sprachsynthese lief ohne Audiowiedergabe mit Stimme
`Anna (German (Germany))`, 180 Wörter/min, PCM16 mono 16 kHz.
Audio und Rohhypothesen bleiben lokal; die Dateien werden nicht ausgeliefert.

| Vorab vorgegebener Text | Frames | Dauer |
|---|---:|---:|
| Ja. | 7382 | 0,461375 s |
| Nein. | 9109 | 0,5693125 s |
| Vielen Dank fürs Zuschauen. | 29350 | 1,834375 s |

Das [Manifest](../../tests/fixtures/corpus/synthetic-speech-diagnostic-v1.json)
mit unveränderten Synthesevorgaben wurde vor Inferenz eingefroren, SHA-256
`9ece2985b5e2bf46199f0187341ec0ee75d737cbca6f847c1b56f1808db9141b`.
Die [Befehle und Parameter](../../tests/fixtures/corpus/synthetic-speech-parameters-v1.json),
SHA-256 `6645533c2f5dc7d234c05035ffaa27929ee9d4732966fb84237dd6126bea2818`,
binden das unveränderte, bereits kompilierte Corpus-Werkzeug der Revision
`c6cb312` und dessen geprüfte ASR-/VAD-Konfiguration. Währenddessen entwickelte
neue Batch-Messquellen sind nicht Teil dieses Binärstands; die aktuelle
Cargo-Manifestabweichung ist ausdrücklich angegeben. Die übrigen 27 Quellen
stimmten beim Freeze mit der ursprünglichen Konfiguration überein.

Beide Modelle verarbeiteten dieselben drei Dateien seriell ohne parallele
Builds/Tests. Beide Inferenz- und beide Auswerterprozesse endeten mit Exit 0;
keine Load-/Service-/Cleanupfehler. Unter dem eingefrorenen Auswerter
Python 3.9.6/Unicode 13.0.0 ergeben sich **je Modell 0 Wortedits bei 6
Referenzwörtern**, alle drei Sprachfälle erfolgreich und nicht leer.
Der [Zahlenbericht](synthetic-speech-v1.json) bindet sämtliche lokalen
Audio-/Generierungs-/Ergebnis-/Metadaten-/Logdateien mit Hash und Dateimodus.

Damit ist am echten ASR-Pfad belegt, dass diese beiden kurzen Ja-/Nein-Dateien
verarbeitet werden und der legitime Satz in dieser synthetischen Sprachdatei
erhalten bleibt. Dieser ergänzende A07b-/A22-Audio-/Inferenznachweis ersetzt
weder die natürlichen Kurzsprachfälle noch eine native Aufnahme und Übergabe
in ein tatsächliches Ziel. Er wird nicht rückwirkend dem unveränderten
90-Fälle-Abnahmevergleich zugeschlagen und liefert keine allgemeine
Sprachqualitäts-, Leistungs- oder P05-Gesamtqualifikation.
