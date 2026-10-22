# Legitime Bruchnotation durch Decoder-Tokensperre beschädigt

Stand 17.10.2026. Die historische Ankerprüfung
ließ EN1682 bei beiden Modellen numerisch unklar. Dieser Befund wird nicht
rückwirkend geändert. Ein enger Diagnosevergleich mit dem Referenzmodell zeigt
jetzt einen korrigierbaren Einfluss der aktiven Tokensperre.

## Mechanismus und kontrollierter Vergleich

`transcription.rs` setzte `suppress_nst=true`. Der tatsächlich gebündelte
whisper.cpp enthält in `non_speech_tokens` unter anderem `/`, `+`, `=` und
Klammern. Bei aktivem Flag maskiert er die exakten Vokabeleinträge sowie deren
Variante mit führendem Leerzeichen, sofern vorhanden. Das bedeutet kein
allgemeines Verbot aller mehrzeichigen Tokens mit diesen Zeichen; die konkrete
Wirkung wurde deshalb gemessen statt nur aus der Liste angenommen.

Zunächst reproduzierte der unveränderte alte Corpus-Probe-Build mit exakt dem
bisherigen EN1682-WAV den uneindeutigen ersten Bruch. Ein enger Prüfhelfer gab
Exit1: erste Dimension nicht eindeutig, zweite eindeutig. Danach wurde nur
`suppress_nst=false` im Produktionsdecoder und die entsprechende Probe-Metadaten-
angabe geändert. Service, Gate, gebündelter C++-Code und Parameterwrapper sind
gegen c6cb312 unverändert; `git diff` zeigt am ASR-Pfad nur diese Flagänderung.
Es gibt weder eine Zahlenumwandlung noch einen neuen Textpostprozessor.

Vor dem gebundenen Off-v2-Lauf wurden Quellen, Binary, Eingangsmanifest und
vollständige Decoderkonfiguration festgehalten. Derselbe vollständige Audioclip
lieferte nun eindeutige Schreibweisen beider veröffentlichten Bruchwerte; der
unveränderte Prüfhelfer gab Exit0. Die Engine-Metadaten unterscheiden sich nur
im untersuchten Suppress-Flag. Beide echten Inferenzprozesse endeten mit Exit0,
ohne Lade-/Service-/Cleanupfehler. Das ist ein realer RED/GREEN-Beleg an diesem
vorher exponierten Fall, kein neuer blinder Heldout-Erfolg und kein Beleg für
jede mögliche Zahlenäußerung. Datei-Backendzeiten sind kein Echtzeit-L.

Eine Ausführungsabweichung bleibt erhalten: Beim ersten Off-Diagnoseversuch
scheiterte ein Parameterexport an einem falschen Dictionarypfad; die Shell
startete danach trotzdem den Modelllauf. Dessen Rohdateien sind als explorativ
aufbewahrt und zählen nicht als vollständig vorab gebundener Nachweis. Erst
Off-v2 folgte dem erfolgreich geschriebenen Parameterbeleg. Es wird weder ein
fehlgeschlagener Helper verschwiegen noch ein späterer Freeze als früher ausgegeben.

## Unveränderte Kalibrierung als Kontrolle

[Parameter v5](../../tests/fixtures/corpus/reference-parameters-v5.json), SHA256
`633ecf4f2c4b4d95bc306c277862cb47bcdcffd36c7aa4f9acabe12c43556f3a`,
standen vor beiden folgenden Läufen fest. Der bestehende 42er-Kalibrierungskorpus
bleibt bytegleich. Energiegrenzen, Ganzaufnahme-VAD und alle anderen Decoderwerte
bleiben unverändert. Kein Parametervergleich wurde anhand besserer Heldout-WER gewählt.

Beide Modelle wurden nacheinander im neuen Corpus-Probe-Build ausgeführt und
mit dem unveränderten Python-3.9.6-/Unicode-13-Auswerter bewertet. Alle 84 Fälle
haben status=ok; keine Lade-, Service- oder Cleanupfehler. **Sämtliche 42 Texte
je Modell sind byteidentisch zu v4**, ebenso alle Qualitätsgruppen. Je sechs
Leerkontrollen bleiben ohne unerwünschten Text. Kurze/leise Sprache und die
Satz-plus-Stille-Fälle bleiben enthalten; keine neue Stillequalifikation aus
bloßem Prozessende behauptet.

| Modell | DE clean | EN clean | DE challenging | EN challenging | unerwünschte Leerausgaben |
|---|---:|---:|---:|---:|---:|
| Referenz | 8/218 | 5/207 | 3/147 | 4/135 | 0/6 |
| Small | 18/218 | 9/207 | 9/147 | 5/135 | 0/6 |

Die challenging-Nenner enthalten leise Sprache **und** den jeweils gesprochenen
Satz-plus-Stille-Fall; die leer referenzierten Kontrollen erhalten keinen Nenner.
Die neue Kalibrierung ersetzt weder den unveränderten 90er-Abnahmelauf mit dem
korrigierten Decoder noch dessen erneute relevante Ankerbewertung. Small ist
dadurch nicht automatisch qualifiziert, und die Echtzeit-/Ressourcen-/nativen
Produktabnahmen bleiben getrennt. Kein App-/DMG-Build wurde erzeugt.

## Reproduzierbarer enger Feedbackloop

Das Einzelfallmanifest ist eine unveränderte Auswahl des bereits exponierten
EN1682 aus acceptance-v1, keine neue Referenz oder ausgewählte erfolgreiche Aufnahme.
Die zwei CLI-Ausführungen nutzten `corpus_probe MODEL reference MANIFEST NEW_OUTPUT`.
Der enge, unveränderte Prüfhelfer ist lokal mit Inhaltshash gebunden:

```sh
/usr/bin/python3 artifacts/diagnostic-speech/check-fraction-regression-v1.py artifacts/diagnostic-speech/fraction-suppress-nst-on-v1.json
# Exit 1
/usr/bin/python3 artifacts/diagnostic-speech/check-fraction-regression-v1.py artifacts/diagnostic-speech/fraction-suppress-nst-off-v2.json
# Exit 0
```

Er prüft nur die explizite Schreibbarkeit dieser bekannten Dimensionen und ist
kein allgemeiner semantischer Zahlenprüfer. Der vollständige rohe Satz wurde
zusätzlich gelesen. Der [inhaltfreie Beleg](non-speech-token-mask-v5.json) bindet
15 private Eingangs-/Ergebnis-/Parameter-/Prüfdateien mit Hash, Größe und 0600,
die Engineänderung und sämtliche Kalibrierungsgruppen. Vollständige Hypothesen
bleiben lokal. Der Codekandidat und die Messmetadaten werden gemeinsam geprüft;
kein unveränderter dritter zentraler Zeitversuch wird aus diesem Befund gestartet.
