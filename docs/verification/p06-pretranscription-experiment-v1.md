# P06 – isolierte Vorabtranskription an bestätigten Pausen

Stand: 18.10.2026. Prospekt vor nativen Kandidatenausgaben. Noch keine
Qualitäts-, Ressourcen- oder Leistungsqualifikation und keine Aktivierung.

## Feste Variante

Die Variante verwendet dieselbe vollständige Reference-Engine mit
Modellkontext1500, Parameter `audio_ctx=0`, denselben Decoderparametern,
Energie-/VAD-/Paddingregeln und demselben Modell. Reduzierter Kontext und
Flash Attention sind kein Bestandteil dieses isolierten Versuchs. Der
Produktionsstandard bleibt bis zur vollständigen Entscheidung unverändert.

Die bereits im Plan festgelegte Energieanalyse gilt unverändert:
20-ms-Fenster auf der kontinuierlichen16-kHz-Zeitachse, Beginn-RMS0,00005,
End-RMS0,000025,150ms Vorlauf,200ms Nachlauf,500ms bestätigte Pause.
Nach letztem aktiven Fensterende `v` ist ein Schnitt bei `v+3200` erst
zulässig, nachdem mindestens `v+8000` stabile Frames vorliegen. Keine
Mindestsprachdauer, erzwungenen Zeitschnitte, Überlappungen oder nachträgliche
Wortreparaturen. Eine Pause beendet die Sitzung nicht. Ohne sichere Pause
bleibt der Bereich bis Stop offen, einschließlich der vollen fünf Minuten.

Die Bereiche bilden eine geordnete, disjunkte Partition der vollständigen
DSP-Ausgabe. Globale Timelinebereiche sind getrennt von lokalen Storage-
Slices. Resamplerzustand und Filter laufen über Pausen unverändert weiter;
Drain/Flush und finale Längenkorrektur finden erst beim Aufnahmeende statt.
Unwiderruflich veröffentlichte Präfixe dürfen nicht durch spätere finale
Längenkorrektur verändert werden. Eingangsformat, Teilfenster und Rundung
gehören deshalb zu den kontrollierten Verträgen.

## Besitz und Abschluss

Sealing bewegt den bestehenden Journal-Vec in den unveränderlichen Jobstorage
und kopiert ausschließlich den noch offenen Suffix in genau einen neuen
offenen Journal-Vec. Höchstens zwei große Journalstorages, auch über Cancel
und neuen Start hinweg. Die Suffixkopie liegt im zweiten Journal; sie ist kein
dritter Vollpuffer. Bei Rückstau bleibt nur eine begrenzte Schnittbeschreibung,
keine PCM-Warteschlange. Der Einspeisetakt wartet nicht auf den Decoder.

Ein weiteres Seal benötigt den tatsächlichen Besitznachweis: kein älterer
Jobstorage mehr im aktiven Worker und das zugehörige Ergebnis konsumiert.
`busy=false` allein genügt nicht. Der Worker muss seine Audio-Referenz vor
Ergebnisveröffentlichung/Freigabe fallen lassen. Lebenszyklusereignisse müssen
auch unter Rückstau zustellbar bleiben. Bei Cancel wird die ganze Sitzung
invalidiert; ältere Ergebnisse dürfen keinen neuen Text erzeugen.

Session, Generation, monotone Job-ID und globaler Bereich werden gemeinsam
validiert. Ein begrenzter Ledger sammelt geordnete Ergebnisse. Nur nach
AudioReleased, Drain/Flush und vollständiger Bereichsabdeckung entsteht genau
ein endgültiges Transkript. Die bestehende Core-Phasenfolge bleibt erhalten.
Nichtleere getrimmte Teile werden mit genau einem Leerzeichen verbunden;
keine heuristische Deduplizierung. Gesamter Text höchstens256KiB.

Die bereits benannten numerischen Pufferposten bleiben unter einem strikten
Zwei-Journal-Vertrag höchstens59.322.889B beziehungsweise57.740.288B bei
16-kHz-Mono. Journalpayload: `2 × 4.800.000 × 4 = 38.400.000B`.
Ein verzögert übergebener Suffix kann fast ein ganzes Journal umfassen und
wird nicht pauschal als300ms budgetiert. Zusätzliche begrenzte Ledger-, Text-
und Messmetadaten werden separat nachgewiesen. Das ist kein Heap-/RSS-Beleg;
Allocator, Stacks, OS und Backend bleiben separat zu messen.

## Nachweise und unveränderte Auswahl

Der erste öffentliche kontrollierte Runtime-/Capture-/DSP-Test lief tatsächlich
RED9889→GREEN36169. Er prüft frühen Decoderstart während Recording,
19.200-Frame-Präfix, gehaltenen Auftrag, Endmarker im finalen Rest, exakte
verkettete Decoder-Slices und genau einen geordneten endgültigen Text. Das
ist ein Implementierungsnachweis mit kontrolliertem Decoder, keine ASR-Abnahme.

Weitere kontrollierte Verträge:499/500ms, lange Stille ohne Leerjobflut,
fünf Minuten ohne sichere Pause, mehrfacher Rückstau, tatsächlicher
Storage-Drop vor Freigabe, Cancel/neuer Start,44,1kHz sowie letztes Teilfenster
und finale Silbe. Native Prüfungen benötigen danach einen zusätzlichen
Source-/Binary-/Parameter-/Kommandofreeze am stabilen Kandidaten.

Die unveränderte [100er-Auswahl](../../tests/fixtures/corpus/p06-context-quality-selection-v1.json)
bleibt Grundlage: ursprüngliche90 mit ihren eigenen WER-Nennern, vorhandene
zehn Kontrollen separat. Neue gleichcode-/gleichaufbau-Referenz verwenden;
ältere Texte sind Drift-/Semantikanker, ältere L-Werte ersetzen den neuen
Variantenvergleich nicht. Kein Falltausch und keine WER-getriebene Änderung
der Pausenschwellen oder des Joinverfahrens.

Eine reine Inputanalyse ohne Inferenz zeigt begrenztes Schnittpotenzial:
11/100 Fälle haben unter diesen festen Regeln mögliche Pausenschnitte, bevor
Worker-Rückstau berücksichtigt wird. Die zentralen DE10-/DE15-Fälle haben
keinen sicheren Schnitt und bleiben reguläre vollständige Endaufträge.
Das ist weder ein Laufzeitbeleg noch ein Anlass, diese Fälle auszutauschen
oder erzwungene Schnitte einzuführen. Die private Inputnotiz
`artifacts/p06-context-quality/input-pause-analysis-v2.json` (SHA
`10555eed44b756185320a5d0cc2045086b552d28111d42a5c1de8a733337c6f9`)
präzisiert die Hysterese gegenüber der aufbewahrten ersten Diagnose; keine
Änderung der Planparameter und kein ASR-/Laufzeitnachweis.

## Entscheidungsgates

Vorabtranskription wird mit zeitgesteuerter Einspeisung durch denselben echten
Runtime-/Ring-/DSP-/Workerpfad geprüft. Späteres Audio darf nicht vorzeitig
verfügbar sein; Feed darf für Inferenz nicht verlangsamt werden. Reine
Dateitranskription kann dieses Verhalten und dessen Stop-bis-Text-L nicht
qualifizieren. Frühe Teilergebnisse setzen keinen finalen Textzeitpunkt.

Alle Fehler, unerledigten Aufträge, Bereiche und ausgeführten Kontextwerte
bleiben erhalten. Ein begrenzter Jobtrace dokumentiert die tatsächliche Arbeit
vor und nach Stop; bloß weniger Arbeit nach Stop ist kein Nachweis, dass
dieselben Samples vollständig verarbeitet wurden.

Aktivierung nur gemäß Plan: mindestens10% reproduzierbare L-Verbesserung,
höchstens0,5 Prozentpunkte WER-Verschlechterung je Sprache und Klasse,
Clean-WER höchstens10%, keine neuen Anker-/Segmentfehler, sämtliche absoluten
Median-/p95-, Ressourcen-, Abbruch- und Maximaldauergates. Die sechs zentralen
Fälle benötigen je mindestens30 gemessene Wiederholungen, die übrigen je
mindestens10; Warmup und kalte Initialisierung bleiben getrennt. Konkrete
Reihenfolge, Vollparameter, Jobtracegrenzen und Ausgabepfade werden vor dem
ersten nativen Variantenlauf zusätzlich eingefroren. Ein früher Fehler darf
einen Versuch begründet verwerfen, aber keine unbestandenen Abschlussgates
als bestanden ausweisen.

Das [prospektive native Qualitäts-Screening](p06-pretranscription-screening-v1.md)
konkretisiert die erste vollständig gepaarte Prüfung dieser unveränderten
Variante. Es ersetzt keine der vollständigen Abschlussbedingungen.
