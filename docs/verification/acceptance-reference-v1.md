# P05: erster Vergleich mit den veröffentlichten Abnahmereferenzen

Stand: 17.10.2026. Die vorab eingefrorene Konfiguration prüft beide vollständigen
Referenzmodelle mit demselben unveränderten Abnahmekorpus. Dies ist eine
Datei-/Backendprüfung der Textqualität. Sie liefert kein zeitgetreu eingespeistes
`L` und schließt P05 nicht allein ab.

## Vor der ersten Modellantwort festgelegt

[Abnahmekorpus](../../tests/fixtures/corpus/acceptance-v1.json), SHA-256
`7d7a25695ab77d9d4affe04b0bbc1ce5f8bc4b38a59b40763fd26884d090c67d`:
alle 90 `acceptance`-Fälle des vollständigen Manifests in ursprünglicher
Reihenfolge, ohne Änderung von Audio, Referenz, Klasse oder Fallinhalt.
Die sauberen Referenzen enthalten 915 deutsche und 734 englische Wörter.

[Parameter und Befehle](../../tests/fixtures/corpus/acceptance-reference-parameters-v1.json),
SHA-256 `aacbe6cc61d9077966b91dbd3fafcafd2357dc53373ca7b3a9829e85677fbde3`,
binden die 28 Quellen, Modelle, VAD-/Decoderparameter, Hostdaten und sechs
modellunabhängig gewählten zentralen Zeitmessfälle. Die Laufzeitparameter wurden
gegenüber der kalibrierten Referenz nicht geändert. Auswertung ausschließlich
mit `/usr/bin/python3` 3.9.6 / Unicode 13.0.0 und dem festgelegten Auswerter.

Der unabhängige Readback vor Inferenz bestätigte alle Bindungen, sämtliche
90 Audiohashes samt Frames/Format, die unveränderte Auswahl und die Wortzahlen.
Der ausführende Prozess prüfte Quellen, Binary, Parameter und Manifest erneut;
vor dem ersten Modell zusätzlich alle 90 Audiodateien. Beide Modellläufe sind
seriell vorgesehen, ohne parallele Builds/Tests. Fehler und unerwartet leere
Ausgaben werden aufbewahrt; keine Referenzreparatur oder Kalibrierung anhand
dieser Abnahmeantworten.

## Tatsächliche Ergebnisse

Beide Inferenzprozesse und beide Auswertungen endeten mit Exit 0. Je Modell
liegen alle 90 Fälle vor, ohne fehlende/fehlgeschlagene Fälle oder Load-,
Service- und Cleanupfehler. Die vier leeren Referenzfälle je Modell
(synthetische Stille/Rauschen, je Sprache) erzeugten keinen unerwünschten Text.
Dies qualifiziert keinen natürlichen Raumrauschkorpus.

| Modell | DE clean | EN clean | DE challenging | EN challenging |
|---|---:|---:|---:|---:|
| large-v3-turbo-q5_0 | 46/915 = 5,0273 % | 32/734 = 4,3597 % | 279/559 = 49,9106 % | 5/99 = 5,0505 % |
| small-q5_1 | 96/915 = 10,4918 % | 45/734 = 6,1308 % | 347/559 = 62,0751 % | 9/99 = 9,0909 % |

Die Brüche sind Wortedits/Referenzwörter, jeweils Micro-WER. Die sauberen
Sprachgruppen enthalten 43 deutsche und 32 englische Fälle. `challenging`
enthält acht deutsche bzw. sieben englische Fälle; die jeweils zwei Fälle
mit leerer Referenz zählen nicht zum WER-Nenner.

Die Referenz erfüllt das numerische Clean-WER-Kriterium in beiden Sprachen.
Small verfehlt es für Deutsch; diese Variante ist unter den eingefrorenen
Parametern nicht qualifiziert. Der rein diagnostische Mittelwert der beiden
Clean-WERs (`Q`) beträgt 0,04693497714447373 für die Referenz und
0,0831129673471211 für Small. Daraus folgt noch keine finale Modellwahl.

Der konstruierte maskierte deutsche 300-s-Fall trägt wesentlich zur hohen
schwierigen DE-WER bei: 274/463 = 59,1793 % für die Referenz und
336/463 = 72,5702 % für Small. Die Hypothesen enthalten 343 bzw. 288 Tokens
gegenüber 463 Referenztokens. Diese Zahlen belegen eine starke Abweichung von
der veröffentlichten zusammengesetzten Referenz, bestimmen aber ohne
Originalinhalt-/Alignmentprüfung noch keine konkrete Auslassungsursache.
Der Fall wird weder entfernt noch in eine andere Klasse verschoben.
Seine vollständige Produktabnahme bleibt offen.

Service-Ready wurde nach 1240 ms (Referenz) bzw. 565 ms (Small) beobachtet;
Cleanup nach 4 bzw. 1 ms. Dies sind Einzelbeobachtungen dieses Qualitätstests,
keine zusätzlichen reproduzierten Kalt-/Warmstart- oder `L`-Vergleiche.

Der [inhaltfreie Zahlenbericht](acceptance-reference-v1.json) enthält die
vollständigen Gruppenwerte und SHA-256-/Größen-/Modusbindungen aller zehn
Rohdateien. Hypothesen, vollständige Fallauswertung und Logs verbleiben lokal
unter `artifacts/acceptance/v1/`, jeweils Modus 0600. Keine Ausgaben werden
nachträglich ersetzt oder für eine Anpassung dieser Referenzen verwendet.
Das erfolgreiche Schreiben einer Auswertung ist kein Bestehen ihrer Kriterien.

## Abgrenzung der noch offenen Anforderungen

Die Originalinhalt-/Ankerprüfung von Namen, Zahlen und Negationen bleibt
erforderlich. Sie ist keine im Plan verlangte Vorbedingung für diesen ersten
diagnostischen WER-Lauf gegen unveränderte veröffentlichte Referenzen. Eine
numerisch niedrige WER ersetzt sie ebenso wenig wie ein erfolgreiches
Prozessende.

Der Plan verlangt kurze Sprache im Bereich 0,2–1 s; er verlangt keinen exakt
0,2 s langen Clip. Vorhandene kurze Ziffern liegen in diesem Bereich. Die
gesonderten gesprochenen Ja-/Nein-Fälle und A22 bleiben offen.

Der deutsche Fünf-Minuten-Fall muss ohne sichere Energiepause auskommen.
Ein konstruierter und entsprechend gekennzeichneter Fall kann diese
Replay-Belastung prüfen; eine unveränderte natürliche Monologaufnahme ist keine
zusätzliche harte Planbedingung. Sein Ausgangsmaterial, Rauschmaskierung und
Padding müssen erkennbar bleiben. Weder Dateitranskription noch eine bestandene
Energieprüfung qualifiziert allein die zeitgetreue Maximaldauer-, Vollständigkeits-,
Ressourcen- und Abbruchabnahme. Reale Mikrofon-/Hotkeyprüfungen bleiben separat.

Natürliches Raumrauschen, unabhängige Originalinhalt-/Ankerprüfung, weitere
Zeitmessungen und die vollständige P05-Qualifikation bleiben offen. Frühere
Protokolle nennen teils strengere zusätzliche Korpuswünsche; sie werden nicht
nachträglich als bereits erfüllte Abnahmen oder neue verbindliche Gates behandelt.
