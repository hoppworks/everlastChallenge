# P05: erneuter Qualitätsvergleich nach Korrektur der Tokensperre

Stand: 17.10.2026. Dateibasierter Qualitätsvergleich, keine Echtzeit-Latenzabnahme.

## Vorabbindung

Quellstand db9a640; Release-Corpus-Probe SHA
`abf9ce66f600090778cca1e682087606be79c513bfb881f8bc60d95224aca90f`.
[Parameter v2](../../tests/fixtures/corpus/acceptance-reference-parameters-v2.json),
SHA `9b045db30e32bc7dfaa8775fba598ca93c7553b6f4b93c26af3dd6180b39f0f7`,
vor Modellarbeit eingefroren und in cae6815 committet. Unabhängig geprüft:
28 Quellen gegen Arbeitsstand und immutable Git, Binary, beide Modelle/VAD,
alle 90 WAV-Hashes und Header/Frames, unveränderter Manifesthash und Unicode13.
Die einzige operative Decoderänderung gegenüber v1 ist suppress_nst true→false;
Parameter entsprechen der zuvor geprüften v5-Kalibrierung. Keine Referenzreparatur.

Bereits exponierte Fälle, kein neuer Blindtest. Das gefrorene beschreibende Feld
openCoverageFromManifest bezeichnet abgeleitete aktuelle Restanforderungen,
keinen wörtlichen Export des historischen Manifestfelds. Dieser Namenshinweis
kam nach Laufbeginn; Freeze blieb unverändert, Bedingungen sind nicht betroffen.

## Ergebnisse

Beide Modelle liefen seriell, Referenz 16:14:49–16:16:30 UTC, Small danach bis
16:17:01 UTC. Beide Inferenzprozesse und Auswerter Exit0, je 90 Fälle statusok,
keine Lade-/Service-/Cleanupfehler. Vier synthetische Leerkontrollen je Modell
ohne unerwünschten Text; daraus folgt keine natürliche Raumrauschabnahme.

| Modell | DE clean | EN clean | DE challenging | EN challenging |
|---|---:|---:|---:|---:|
| Referenz | 47/915 = 5,1366 % | 31/734 = 4,2234 % | 279/559 = 49,9106 % | 5/99 = 5,0505 % |
| Small | 96/915 = 10,4918 % | 44/734 = 5,9946 % | 346/559 = 61,8962 % | 9/99 = 9,0909 % |

Edits/Referenzwörter, Micro-WER. Referenz besteht das numerische Clean-Kriterium
beider Sprachen; Small verfehlt Deutsch weiterhin. Kein finales Modell gewählt.
Gegen v1 ändern sich vier Referenz- und fünf Small-Fallausgaben. Alle anderen
Rohtexte und Status sind bytegleich; keine nachträgliche Änderung der WER-Regeln.
Die neue englische Bruchschreibweise ist eindeutig, während eine neue deutsche
Small-Bruchschreibweise mehrdeutig bleibt. Semantische Ankerprüfung ist separat.

Im unveränderten konstruierten maskierten 300-s-Fall bleiben 274/463 Edits bei
der Referenz (343 Hypothesentokens); Small hat 335/463 (266 Tokens). Diese
starken Abweichungen werden nicht als bestandene Vollständigkeit gewertet.
Tokenzahlen allein bestimmen weder Auslassungsort noch technische Ursache.

[Inhaltfreier Zahlen-/Hashbericht](acceptance-reference-v2.json) bindet sämtliche
Rohdateien und die Completionbelege. Hypothesen und Logs verbleiben privat mit
Modus0600. Weder dieser Lauf noch gute Clean-WER schließen P05: Langaufnahme,
aktuelle Anker, belegter natürlicher Raumton, reproduzierbare Zeitmessung sowie
relevante Ressourcen-/Abbruch- und native Abnahmen bleiben gesondert zu prüfen.
Kein App-/DMG-Build oder Mikrofonzugriff.
