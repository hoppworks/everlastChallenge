# P06 – numerischer ABBA-Qualitätsvergleich

Das Werkzeug `scripts/compare-screening-quality.py` vergleicht acht vollständige
Zahlenberichte aus dem unveränderten kanonischen `scripts/quality.py`:
je einen Warmup- und Messbericht für jeden der vier eingefrorenen Blöcke.
Jeder Eingabebericht enthält alle 100 Fälle aus dem festen Manifest.
Hypothesen und Referenztexte werden nicht in den Vergleichsbericht übernommen.

```sh
/usr/bin/python3 scripts/compare-screening-quality.py \
  tests/fixtures/corpus/p06-context-quality-selection-v1.json \
  B1_WARMUP.json B1_MEASURED.json B2_WARMUP.json B2_MEASURED.json \
  B3_WARMUP.json B3_MEASURED.json B4_WARMUP.json B4_MEASURED.json \
  --output NEUER_VERGLEICHSBERICHT.json
```

Die acht Dateinamen im Beispiel sind Platzhalter für tatsächlich erzeugte
kanonische Berichte. Die Reihenfolge ist Reference → Candidate → Candidate →
Reference. Vor dem Aufruf müssen Rohdaten, Fallabschlüsse und Metadaten des
eingefrorenen Laufs geprüft und jedem Bericht die passende Phase und der
passende Block zugeordnet sein. Der Vergleicher prüft diese Laufzeitprovenienz
nicht selbst; Reporthashes erhalten die verwendeten Eingaben für den Readback.

## Feste Nenner und getrennte Vergleiche

Die feste Auswahl und die neue
[Referenzwortzahl-Fixture](../../tests/fixtures/corpus/pretranscription-quality-denominators-v1.json)
sind gehasht gebunden. Die Fixture enthält je Fall Identität, Referenztexthash,
ursprüngliche Korpuszugehörigkeit und kanonische Wortzahl. Ihre Herkunft bindet
den vorhandenen [Qualitätsbericht](p06-flash-attention-quality-v1.json), das
unveränderte 100er-Manifest und den Unicode-13-Auswerter. Auf dem Mac wurden
alle 100 Wortzahlen zusätzlich unmittelbar aus den Referenzen nachgerechnet.
Der Vergleicher selbst normalisiert keinen Text neu.

Original90 werden über das ursprüngliche Quellmanifest ausgewählt; `acceptance`
allein trennt sie nicht von den Zusatzfällen. Warmup und Messlauf bleiben
getrennt, ebenso die Paare 1/2 und 4/3. Die zehn Zusatzfälle erscheinen separat
und verändern die ursprünglichen WER-Nenner nicht.

Je Sprache und Klasse gilt exakt `200 * (CandidateEdits - ReferenceEdits) <=
ReferenceWords`; für clean zusätzlich `10 * CandidateEdits <= ReferenceWords`.
Die Entscheidung verwendet ganze Zahlen. Unvollständige oder fehlerhafte
Fälle und unerwünschte Ausgabe bei Stille verhindern ein positives Zahlen-Gate.
Gruppensummen werden aus den Fallzahlen neu berechnet; falsche Nenner,
Identitäten, Summen, Vollständigkeitsflags und widersprüchliche Stille-Berichte
werden abgewiesen.

## Ausgabe und Beleggrenzen

Eingaben sind begrenzt; doppelte JSON-Schlüssel und nicht endliche Zahlen sind
ungültig. Die neue Ausgabe wird privat und ohne Überschreiben veröffentlicht.
Exit 0 bedeutet bestandenes Zahlen-Gate, Exit 1 einen erhaltenen negativen
Zahlenbericht und Exit 2 ungültige Eingaben oder eine fehlgeschlagene Ausgabe.

Auch bei Exit 0 bleiben `qualified=false` und `L=null`. Ganze Textpaare,
Anker, Zahlen, Negation und Segmente benötigen ihre eigene semantische Prüfung.
Der Zahlenvergleich ersetzt weder Pipeline- und Quellenprüfung noch Latenz-,
Ressourcen-, Geräte- oder finale P06-Abnahme. Ein unvollständiger Messlauf wird
dadurch nicht zu einem vollständigen Vergleich.
