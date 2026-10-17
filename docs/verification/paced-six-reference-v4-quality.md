# P05: Qualität des zentralen Sechs-Fälle-Wiederholungsblocks v4

Stand:17.10.2026. Messcode `80261d3`, Referenzmodell `large-v3-turbo-q5_0`;
dieselben sechs bereits exponierten clean-Abnahmefälle. Alle181 Hypothesen sind
je Fall bytegleich mit den bisherigen Native-Timestamp-Reference-Ausgaben.
Es gibt sechs eindeutige Ganzpaare; auch der Warmuptext stimmt damit überein.
Keine neue Inferenz, Referenzkorrektur oder Normalisierungsänderung.

## Unveränderte Unicode-13-WER-Auswertung

`scripts/quality.py.evaluate` lief mit `/usr/bin/python3`3.9.6/Unicode13.0.0.
Eindeutige Slot-IDs und ausschließlich `{id,status,text}` gingen als Ergebnisse
an den eingefrorenen Evaluator. Die180 Messzeilen und das einzelne Warmup wurden
getrennt ausgewertet; keine Zeile fehlt oder scheiterte.

| Scope | Zeilen | Edits/Referenzwörter | WER |
|---|---:|---:|---:|
| DE, drei Fälle jeweils30-mal |90|210/1650|12,7273%|
| EN, drei Fälle jeweils30-mal |90|30/1650|1,81818%|
| Warmup DE5, separat |1|1/10|10,0000%|

Die Wiederholungsgewichte erzeugen keine neuen unabhängigen Referenzwörter:
je Sprache sind es55 Referenz-Tokenpositionen über drei eindeutige Eingaben
(keine Behauptung von55 verschiedenen Wortformen).
Das ist ein **SixCaseRepeatSubset**, keine erneute90er-Korpusauswertung. Die
historischen Full90-clean-Werte bleiben46/915=5,0273%DE und34/734=4,6322%EN;
sie wurden weder ersetzt noch mit Wiederholungen vermischt. Der höhere deutsche
Teilblockwert ist bei bytegleichen älteren Ausgaben keine neue Regression.

## Vollständiger Inhalt und Anker

Alle sechs vollständigen Referenz-/Hypothesenpaare und ihre16 Inventaranker wurden
nochmals gelesen. Für alle181 Zeilen wurden die historischen Urteile wegen
exakter Referenz-, Status- und Hypothesenidentität unverändert übernommen und
per SHA gebunden. Es gibt keine neue Satzteil-Auslassung oder -dopplung.

| Fall | Edits/Wörter je Wiederholung | Ganzsatzurteil | Inhaltsfreie Einordnung |
|---|---:|---|---|
|fleurs-de-test-1736|1/10|fail|Bekannte explizite Vorname-Entitätssubstitution; Handlung und Zahlen-/Modellanker erhalten.|
|fleurs-de-test-1663|4/21|pass|Referenten und Relation erhalten; dokumentierte Namensorthografie/Diakritik abweichend.|
|fleurs-de-test-1668|2/24|pass|Namensform-/Wortgrenzenvariante, gleiche Zutrittsrelation.|
|fleurs-en-test-1679|1/9|pass|Regionale Orthografievariante derselben Aussage.|
|fleurs-en-test-1676|0/17|pass|Vollständige Aussage erhalten.|
|fleurs-en-test-1667|0/29|pass|Zahlenrelation und beide Teilaussagen erhalten; Interpunktion abweichend.|

Ein eindeutiges Ganzpaar bleibt semantisch fail, fünf pass. Das entspricht
30 fail/150 pass unter den180 Messwiederholungen; der separate Warmup ist fail.
Es sind Wiederholungen eines bekannten Fehlers, keine31 unabhängigen neuen Fehler.
Von16 eindeutigen Ankern sind15 pass und einer fail; gemessen450 pass/30 fail
unter480 Ankerwiederholungen. Zahlenanker sind erhalten. Explizite Negationsanker
kommen hier nicht vor; daraus entsteht keine allgemeine Negationsfreigabe.
Namensschreibungen und eine tatsächliche Entitätssubstitution bleiben getrennt;
eine neue pauschale Null-Namensfehlergrenze wurde nicht eingeführt.

Der [inhaltfreie JSON-Bericht](paced-six-reference-v4-quality.json) bindet
Rohdateien, Manifest, sechs WAVs, Parameter/Freeze, Evaluator und historische
Urteile. Er enthält alle181 numerischen Bewertungen bzw. Reviewzuordnungen,
keine Referenz- oder Hypothesentexte. Decoderkontext, FullParams, SpeechGate und
numerische Vorbereitung stimmen mit dem historischen Native-Timestamp-Vergleich
überein; Scheduling-/Messgerüst und einzelne Beschreibungsfelder sind getrennt.

Keine neue Hörprüfung oder zeitliche Wort-/Audiokorrespondenz, kein Beweis einer
Segmentierungsursache. Keine L-, Full90-, Mikrofon-, Installations-, P05- oder
P06-Freigabe aus diesem Bericht. Die strukturelle Echtzeitprüfung erfolgt separat.
