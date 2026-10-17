# P05: Kalibrierung mit niedrigerer Energiegrenze

Stand: 17.10.2026. Die Korrektur betrifft ausschließlich die Start-/Endschwellen
der transparenten Energieanalyse: RMS **0,00005 / 0,000025** statt
0,01 / 0,005. Decoder, Modellgewichte, Randreserven und Sprachparameter bleiben
unverändert. Grund ist die in [v1](calibration-v1.md) belegte Verwerfung von
14 Sprachfällen vor dem Decoder.

Die neue vor dem Lauf festgeschriebene Parameterdatei ist
[`reference-parameters-v2.json`](../../tests/fixtures/corpus/reference-parameters-v2.json),
SHA256 `d9eace7686ac2ccc8fbde9ee12410ff178d2b69832dce109122f51126a64f0eb`.
Die Messung entstand im Arbeitsstand nach `20aaf03`, vor dem Korrekturcommit;
die Metadaten kennzeichnen dies als `20aaf03+energy-v2-1d932135`.
Der tatsächliche Energiequelltext hat SHA256
`1d93213539481a9a19aeb191a34737796b94544eee4e1340444261bc3db32c46`,
die Entwicklerprobe
`7df81684165cac914ae844de12f139d5dc38b65ddc2066757ed5db08e2e8b085`.
Der Enginequelltext ist gegenüber v1 unverändert und in der Parameterdatei gehasht.

## Unveränderte 34 Sprachfälle

Dieselbe getrennte Kalibrierungsmenge wie v1 wurde verwendet, Manifest-SHA256
`39d1029d375a6f639fc9b7c44c3c897252f7baee6b081046f789f77f3c5365b6`.
Beide Modelle liefen nacheinander mit je einem warmen Dienst und frischem
Zustand je vollständiger Inferenz. Alle 34 Jobs je Modell waren technisch
erfolgreich; es gab keine leeren Sprachausgaben mehr.

| Modell | DE sauber, 218 Wörter | EN sauber, 207 Wörter | DE leise, 121 Wörter | EN leise, 114 Wörter |
|---|---:|---:|---:|---:|
| large-v3-turbo-q5_0 | 8 Fehler / 3,67 % | 5 / 2,42 % | 3 / 2,48 % | 2 / 1,75 % |
| small-q5_1 | 18 Fehler / 8,26 % | 9 / 4,35 % | 9 / 7,44 % | 3 / 2,63 % |

Diese einmaligen Kalibrierungswerte dienen der Parameterwahl. Sie sind keine
Abnahmemessung, Leistungsfreigabe oder finale Modellwahl. Die separate
Abnahmemenge aus dem ursprünglichen 124-Fälle-Manifest wurde nicht transkribiert.

## Automatisierte Regression

`quiet_pcm16_signal_is_not_discarded_before_inference` war mit der alten
Startschwelle rot (`None` statt eines Energiebereichs) und ist mit der neuen
Schwelle grün. Der Test verwendet ein synthetisches PCM16-Signal nahe dem
schwächsten Kalibrierungspegel; er behauptet keine semantische Spracherkennung.
Ein noch leiserer Beginn wird durch 150 ms Vorlauf erhalten; ein 300-ms-Signalende
unterhalb der Startschwelle bleibt durch die niedrigere Endschwelle vollständig
erhalten, gefolgt von 200 ms Nachlauf. Nullsignal und ein wechselndes Ein-LSB-Signal
bleiben ohne Energiebereich. Diese Signalfälle ersetzen keine Raumgeräuschprüfung.

## Decodergrenzen

Im gepinnten whisper.cpp-Quelltext ist `no_speech_thold` kein eigenständiger
Sprachdetektor: Die No-Speech-Entscheidung verlangt zugleich eine hohe
No-Speech-Wahrscheinlichkeit und mittlere Logwahrscheinlichkeit unter
`logprob_thold`. Dieselben Größen beeinflussen den Fallback, während die
Entropieschwelle die Decodersequenz bewertet. Ein gesetzter Parameter beweist
deshalb keine halluzinationsfreie Ausgabe. Die Werte bleiben in diesem Versuch
unverändert; tatsächliche Negativfälle werden gesondert geprüft.

## Kontrollfälle und verbleibende Abnahme

Die ergänzende Kalibrierungsmenge mit Stille, synthetischem Rauschen,
[natürlichem Büroumgebungston](room-noise-source.md) und Satz plus 3 s Stille
wurde separat vor Inferenz als `tests/fixtures/corpus/calibration-v2.json` mit
42 Fällen festgeschrieben (34 unverändert übernommene Sprachfälle und acht neue
Kontrollen). Manifest-SHA256:
`514befba28e7ea14882cd13a71f971e5fb6c6131f914491b697741a027611a49`.
Die acht Kontrollen wurden je Modell in einem separaten warmen Dienst ausgeführt;
die bereits gemessenen 34 Sprachfälle wurden dafür nicht erneut gezählt.
Der abgeleitete Acht-Fälle-Manifest hat SHA256
`aae5f5dd8ef1d739920bbf89fe2e2cddcf2d9c7de406ffe782bab105579a0094`.

Beide Modelle lieferten auf sechs leer referenzierten Kontrollen jeweils vier
unerwünschte Ausgaben: DE/EN-Rauschen und DE/EN-Büroton. Nur die beiden
Nullsignal-Kontrollen blieben leer. Auch reine Satzzeichen zählen als
unerwünschte Ausgabe. Der Büroton erreicht maximale 20-ms-RMS 0,0142181 und
ist damit deutlich energiehaltig; ein reines Energie-Gate ist kein ausreichender
Schutz. Die Satz-plus-Stille-Kontrollen hatten bei beiden Modellen DE 0/26 und
EN 2/21 Wortfehler. **v2 ist wegen der Negativfälle nicht qualifiziert.**

Ein zusätzlicher Referenzmodell-Lauf mit dem ursprünglichen Energiequelltext
aus `20aaf03` und den v1-Schwellen zeigte auf denselben acht Kontrollen
byteidentische Ergebnisse, einschließlich derselben vier unerwünschten Ausgaben.
Der Kontrollfehler bestand auf diesen Fällen somit bereits vor der
Schwellenkorrektur. Dieser Nachlauf trägt die Metadatenkennung
`20aaf03+baseline-controls-diagnostic`; vorübergehend wurden ausschließlich
numerische No-Speech-Werte protokolliert. Die Instrumentierung wurde entfernt;
Engine- und Probequellhash entsprechen wieder den oben dokumentierten Werten.
Die Quelle ist nur vom Herausgeber als leeres Büro beschrieben; eine unabhängige
Hörprüfung fehlt. P05 einschließlich der übrigen Abdeckungslücken, Inhaltsanker,
Ressourcen-, Abbruch-, Echtzeit- und nativen Abnahmen bleibt offen.

## Lokale Messdateien

Die Dateien liegen ausschließlich unter `artifacts/corpus`. Hashes pinnen die
tatsächlichen Ergebnisse und Metadaten, ohne Rohtexte einzuchecken.

| Datei | SHA256 |
|---|---|
| calibration-reference-energy-v2-results.json | `431e5816197c6adde2a52da53204fa8384d50ae7d1d30dffaba780367abd851e` |
| calibration-reference-energy-v2-results.json.metadata.json | `b66d4454cce5d72f1e223bb68b138d9cdc2376b557a35e50b4b939b436bcd001` |
| calibration-reference-energy-v2-report.json | `803add41dbf31a89d3a2b1f820bd156cb6de1e74d3211b38b897c12dec5c321a` |
| calibration-reference-v2-controls-results.json | `8e5907ff3247d189cad022815b47808cb0b1cf60d0720aa9cee619c04ae50c93` |
| calibration-reference-v2-controls-results.json.metadata.json | `7dfd240be6fc781708cfd2e6ca169d2664009214540fe033ea28550d217e383f` |
| calibration-reference-v2-controls-report.json | `bffa0913f802125ab25547c5f9f251c8aa521db424f30be08fe079d353df163f` |
| calibration-small-energy-v2-results.json | `11175d3f43b879505d280f8fcea81a3dd8d60c99e5fddbf1401295f8d8331b6e` |
| calibration-small-energy-v2-results.json.metadata.json | `909540e4f7188031ffe4b765e238756658921842ec6b58e085b78e6d615c01f5` |
| calibration-small-energy-v2-report.json | `b67558e8f66750d2e7f5c60fd78e36b5c761ea6515b4b99b00c8ff25722d8e2d` |
| calibration-small-v2-controls-results.json | `e81dc7d95da9b9bea1bfb2d85c3c28828d38c00b209c08aa860435146eeb15be` |
| calibration-small-v2-controls-results.json.metadata.json | `5612605c7675fefd9c17b43bc1ab398a52daa61251b8391195ec0cf788e88c56` |
| calibration-small-v2-controls-report.json | `c14725d6ddeedf81465a7aa1d7d83673bc9ce4e7257e6432c36cc752e1d59ef7` |
| calibration-reference-v1-controls-results.json | `8e5907ff3247d189cad022815b47808cb0b1cf60d0720aa9cee619c04ae50c93` |
| calibration-reference-v1-controls-results.json.metadata.json | `76e03103bb6a77bd512761a96980cfd7e00cfd920883d2b552a0d862277623c9` |
| calibration-reference-v1-controls-report.json | `bffa0913f802125ab25547c5f9f251c8aa521db424f30be08fe079d353df163f` |
