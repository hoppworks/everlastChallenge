# P06: prospektiver Einzelversuch mit reduziertem Audiokontext

Stand:18.10.2026. Vor der ersten Kandidatenausgabe festgelegte Hypothese,
keine gemessene Verbesserung und keine Aktivierung im Produkt.
Voraussetzung ist die [qualifizierte vollständige Referenz](p05-reference-qualification-v1.md).

## Eine festgelegte Variante

Nach unveränderter Energie-/VAD-/Paddingvorbereitung verwendet der Kandidat
`audio_ctx=1024` bei tatsächlicher ASR-Eingabe bis einschließlich320.000
16-kHz-Frames (20s); darüber ausdrücklich `audio_ctx=0` (Modellstandard).
Padding zählt zur Eingabelänge. Abgelehnte/leere Eingaben rufen weiterhin
keinen Decoder auf und erhalten keinen erfundenen effektiven Kontext.
Alle Samples, Sprache, Timestamp-Policy, Textübernahme und übrigen
Decoderparameter bleiben gleich der vollständigen Referenz.

Der Wert wird aus der Encoderzeitachse gewählt:1024 Kontextpositionen decken
2048 Mel-Frames mit10ms Schrittweite, also20,48s ab. Die feste20s-Grenze lässt
480ms Reserve. Die sechs zentralen5/10/15s-Fälle passen vollständig hinein.
Das ist kein auf Acceptance-Ausgaben hin ausgewählter Wert. Es folgt keine
Suche zwischen weiteren Kontextgrößen anhand günstiger WER-Ergebnisse.

Die sichere gepinnte Wrapper-API ist `FullParams::set_audio_ctx(i32)`,
`0` bezeichnet den Modellstandard. Im gepinnten C++ kopiert der Encoder nur
bis `mel_offset + 2*n_ctx`; mehrere Seek-/Fallbackpfade verwenden weiterhin
30s. Deshalb wird ein kürzerer Kontext nicht blind auf lange Aufnahmen
angewandt: der explizite Referenzzweig für mehr als20s gehört zur Variante.
Die erwartete Modellkontextgrenze von1500 Positionen wird geprüft; eine davon
abweichende Konfiguration liefert einen Fehler statt einer stillen Anpassung.
Der vorhandene gehashte Reference-Modellheader enthält tatsächlich1500; das
wurde anhand der gepinnten Loader-Feldreihenfolge ohne Modellinitialisierung
nachgelesen. Die Laufzeitprüfung wird dadurch nicht ersetzt.

Es gibt keine neuen Schnitte, keine Vorabtranskription, keine verschobenen
Zeitachsen, keinen Textfilter und keine Änderung an VAD oder Energiegrenzen.
Der bestehende produktive Referenzeinstieg bleibt bis zur vollständigen
Qualifikation unverändert. Der Kandidat ist zunächst ein ausdrücklich
ausgewählter Entwicklerpfad unter dem Messfeature.

## Implementierungs- und Messvertrag

Die erste vertikale Stufe verbindet dieselbe Vorbereitung und denselben
Decode-/Textjoinpfad mit der benannten Kontextpolicy. Das Ergebnis berichtet
die tatsächlich vorbereitete Framezahl, den gewählten `audio_ctx`-Parameter
(1024 oder0) und dessen Modellkontext (1024 oder1500). Ein gesondertes Flag
kennzeichnet den tatsächlichen Eintritt in `state.full`; eine vorherige
Stateallokation oder bloße Kontextwahl ist keine ausgeführte Inferenz.
Energy-/VAD-Reject bleibt mit leeren Kontextfeldern unterscheidbar. Gezielte RED/GREEN-Tests
prüfen die Sample-/Sprachidentität,320.000/320.001-Frame-Grenze einschließlich
Padding, unveränderte lange Eingabe, Reject-/Fehlerpfade ohne Decode sowie
die nicht unterstützte Modellgrenze. Reine Policytests beweisen keine Qualität.

Danach werden die vorhandenen Corpus-, Single- und Central-Proben so verbunden,
dass alle dieselbe Policy ausdrücklich auswählen. Die tatsächlichen
Kontextentscheidungen je Auftrag werden mit den Messdaten gebunden.
Ein zukünftiges Kandidatenparameterdokument darf nicht pauschal
`fullParams.audio_ctx=0` behaupten: es enthält beide Zweige und die Grenze.
Historische Parameter und Rohdaten bleiben unverändert erhalten.

Vor nativen Kandidatenläufen sind Code, vollständige Parameter, Binary,
Modell/VAD, Quellen, unveränderte Fixtures/Referenzen, Wiederholungen und
wechselnde Variantenreihenfolge prospektiv zu binden und unabhängig zu prüfen.
Für L gilt derselbe tatsächliche MeasuredRuntime-Pfad mit Ring/DSP/Worker,
unverändertem Einspeisetakt und Stop-/Textgrenzen. Keine Dateilatenz als L.
Alle fehlgeschlagenen oder unvollständigen Läufe bleiben erhalten; kein
selektives Wiederholen und keine nachträgliche Lockerung des Jitterbudgets.

### Qualitätsprüfung vor den Echtzeitvergleichen

Die [prospektive Auswahl](../../tests/fixtures/corpus/p06-context-quality-selection-v1.json)
und das daraus unverändert zusammengeführte
[Eingabemanifest](../../tests/fixtures/corpus/p06-context-quality-manifest-v1.json)
enthalten100 eindeutige Fälle: alle ursprünglichen90 Abnahmefälle, das
bestehende Fünfer-Supplement, genau die vier schon verwendeten Kalibrierrollen
für Raumrauschen/Nachlauf und den kontrollierten300s-Fall. Originalreihenfolge,
Referenzen, Klassen und Audiohashes bleiben erhalten. Die zusätzlichen Rollen
werden nicht in die ursprünglichen WER-Nenner eingerechnet.

Der erste native Vergleich verwendet neue Dateiausgaben beider Varianten aus
demselben Binary und gemeinsamen Decoderwrapper. Die Reihenfolge wird vorab
auf Reference→Reduced→Reduced→Reference festgelegt, jeder Block in einem neuen
Prozess und mit identischer Fallreihenfolge. Das sind zwei vollständige
Qualitätsvergleiche in wechselnder Variantenreihenfolge, keine Zeitmessungen
für L. Keine Wiederholungsgewichtung erzeugt zusätzliche Referenzwörter.

Primäre WER-Gruppen bleiben DE/EN jeweils clean/challenging des ursprünglichen
90er-Korpus. Die erlaubte Verschlechterung wird gegen die neue gleichcode-
Baseline exakt als `200*(Edits_candidate-Edits_reference)<=Referenztokens`
geprüft; zusätzlich clean absolut höchstens10% je Sprache. Supplement,
Kalibrierrollen und controlled300 erhalten getrennte Verhaltens-/Inhaltsurteile.
Die neue Reference-Ausgabe wird vollständig mit den historischen Ausgaben
verglichen. Bei Drift wird deren Ursache/Verwendbarkeit geklärt, statt auf einen
günstigeren historischen Nenner auszuweichen. Kontextrecords müssen zur
tatsächlichen Variante und vorbereiteten Eingabe passen; fehlende oder
widersprüchliche Nachweise sind kein bestandener Lauf.

Alle geänderten vollständigen Kandidatenpaare und ihre festen Anker werden auf
neue Negations-/Zahlen-/Segmentfehler gelesen. Exakt identische Paare dürfen
ihre gebundenen historischen Urteile behalten. Neue Ausgabe bei festen Empty-
Fällen, verlorene legitime Kurz-/A22-Sprache und neue unzulässige Segmentfehler
verhindern die Aktivierung unabhängig vom numerischen WER-Ergebnis. Bestehende
Geräuschgrenzen und lokale Wort-/Namensfehler bleiben sichtbar; kein neues
universelles Nullfehlergate. Jeder Block bleibt einzeln auswertbar; kein
Ausmitteln eines verfehlten Vergleichs durch den anderen Block.

Erst bei bestandener Qualitäts-/Kontextprüfung folgen neue gepaarte
Echtzeitläufe beider Varianten, einschließlich neuer Baselinezeiten. Diese
Dateiprüfung ersetzt weder die zentralen30er-Läufe noch die übrigen zehn
warmen Läufe, Ressourcen- oder Abbruchnachweise. Alle konkreten Kommandos,
Parametersätze und freien Ausgabeziele werden vor dem ersten Modellstart
zusätzlich in einem unveränderlichen Runfreeze gebunden.

## Entscheidung nach dem bestehenden Plan

Der Einzelversuch wird erst nach vollständigem Vergleich bewertet:
mindestens10% L-Gewinn gegenüber der vollständigen Referenz,
höchstens0,5 Prozentpunkte WER-Verschlechterung in jeder Sprache und Klasse,
keine neuen unzulässigen Anker-/Segmentfehler und sämtliche absoluten
Pflichtgrenzen. Ressourcen, Abbruch und Ownership müssen weiterhin passen.
Ein negatives Ergebnis wird dokumentiert, nicht durch neue Parameter verdeckt.

Vorabtranskription ist eine separate spätere Variante mit eigenem
verlustfreiem Samplebereichs-/Auftragsvertrag. Eine Kombination wird nur
nach dem dafür erforderlichen eigenen Nachweis bewertet. Dieser Prospekt
schließt P06 nicht ab und ändert weder finales Modellmanifest noch native
Produktabnahmen.
