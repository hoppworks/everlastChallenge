# P06 – isolierte Flash-Attention-Dateiqualität

Stand: 18.10.2026. **Der isolierte Kandidat wird vor Echtzeitmessungen
verworfen und nicht aktiviert.** Der vollständige ABBA-Versuch ist gültig;
beide Vergleichspaare verfehlen die festgelegte Qualitätsgrenze. P06 und die
finale Modellwahl bleiben offen.

## Versuch und Bindung

[Prospekt](p06-flash-attention-experiment-v1.md),
[Runfreeze](../../tests/fixtures/corpus/p06-flash-attention-quality-run-v1.json)
und [vollständiger Zahlen-/Belegbericht](p06-flash-attention-quality-v1.json).
Source `0e59c14`, Freezecommit `7125be9`. Derselbe vollständige
large-v3-turbo-q5_0-Kontext1500 mit `audio_ctx=0`, dieselbe Vorbereitung,
Sprache, Decoderparameter und Zustandsinitialisierung; einzige semantische
Parameteränderung `flash_attn=false→true`. Keine Vorabtranskription oder
Kontextreduktion, keine Produktionsaktivierung.

18.10.2026,03:18:11–03:26:29UTC; Apple M5,32GiB,macOS27.0.1/26A434,
Netzstrom. Vier frische Prozesse in fester Reihenfolge Reference→Flash→Flash→
Reference, jeweils unveränderte100 Fälle. Alle400 Fälle erfolgreich beendet;
kein Ersatz, Retry oder Ausschluss. Beide Referenzdateien sind bytegleich,
ebenso beide Kandidatendateien. Die100 Referenztexte entsprechen vollständig
den zuvor geprüften historischen Baseline-Ausgaben.

Jeder Block belegt94 tatsächliche Decodeversuche bei Kontext1500/Parameter0,
zwei EnergyEmpty- und vier VadRejected-Fälle. Auswahlflag und tatsächlicher
Full-Aufrufversuch sind getrennt nachgewiesen. Vorbereitete Framezahlen
und Identitäten bleiben passend; das Flag beweist keine vollständige
GPU-/Kernelabdeckung. Alle Probe-/Wrapper-/Supervisor-Prozesse sind beendet,
ebenso die eigene Wachhalteanforderung. Private Rohtexte bleiben0600.

## Ursprüngliche90 – jedes Paar separat

Die Tabelle gilt einzeln für1/2 und4/3. Wiederholungen verändern den Nenner
nicht. Fester Python3.9.6-/Unicode13-Auswerter, keine neuen Referenztexte.

| Sprache/Klasse | Wörter | Referenzfehler | Flashfehler | WER Referenz → Flash | Differenz |
|---|---:|---:|---:|---:|---:|
| DE clean |915|46|49|5,0273% →5,3552%|+0,3279pp|
| EN clean |734|34|33|4,6322% →4,4959%|−0,1362pp|
| DE challenging |559|230|257|41,1449% →45,9750%|**+4,8301pp**|
| EN challenging |99|5|5|5,0505% →5,0505%|0pp|

Die Clean-Absolutgrenze10% bleibt erfüllt. Die Grenze höchstens0,5pp
Verschlechterung gilt jedoch für jede Sprache und Klasse: DE challenging
verfehlt sie in beiden Paaren. Exakter Ganzzahltest:
`200*(257-230)=5400 > 559`. Eine Verbesserung in EN gleicht das nicht aus.

## Vollständige Inhaltsprüfung und zusätzliche10

Root und ein unabhängiger Reviewer haben alle15 geänderten vollständigen
Referenz-/Baseline-/Kandidatenpaare geprüft, einschließlich beider langen
Kontrollen. Die unveränderten85 übernehmen frühere Urteile ausschließlich
bei vollständiger Text-/Statusidentität. Keine neue Audioanhörung behauptet.

Besonders relevant ist `constructed-de-masked-300s`: zusätzlich zu den bereits
erheblichen Baselinefehlern entstehen erfundene Wiederholungsschleifen;
zuvor erhaltene Aussagen fehlen, darunter die ganze Überhitzungswarnung mit
Negation sowie die FBI-/Agentenaussage. Zuvor korrektes2005 wird2000,
zwei Verhaltensweisen werden drei. Das ist kein bloßer Schreibweisenwechsel.

Die zusätzliche kontrollierte300s-Aufnahme behält alle21 Quellblöcke einmal
erkennbar. Ihre Fehlerzahl steigt28→29/463; vorhandene lokale Wort-/Namensfehler
bleiben, der bisherige unbelegte Schluss-Musikhinweis entfällt. Im separaten
DE30s-Supplement steigt die Fehlerzahl4→5: ein neuer unbelegter Schlusszusatz
ersetzt die frühere überzählige Konjunktion. Die anderen zusätzlichen Fälle
bleiben numerisch gleich; Kurzäußerungen, beide Raumfälle und Nachlauf sind
vollständig enthalten. Diese zehn Fälle werden nicht in die ursprünglichen
WER-Nenner eingemischt.

Weitere Änderungen betreffen Schreibweise, Grammatik, Interpunktion oder
eine wiederhergestellte Konjunktion. DE1669 enthält einen neuen
Namensschreibfehler bei weiterhin erkennbarer Person; EN1684 verändert eine
bereits falsche Namensschreibung. Diese Fehler werden offengelegt, ohne ein
nachträgliches Null-Namensfehler-Gate einzuführen.

Die binären Gesamturteile bleiben80pass/19fail/1unclear, weil die großen
Verschlechterungen bereits vorher fehlgeschlagene Fälle betreffen. Diese
unveränderte Summe bedeutet ausdrücklich keine unveränderte Qualität.
Alle23 betroffenen Anker-Vorkommen, darunter21 ursprüngliche Clean-Anker,
bleiben semantisch pass. Insgesamt bleiben die158 ursprünglichen eindeutigen
Clean-Anker bei154pass/3vorherigenfail/1vorherigenunclear.

## Entscheidung und Grenzen

Keine Latenz-/Ressourcenqualifikation aus Dateizeiten oder `/usr/bin/time`.
Für diesen isolierten, schon an Qualität gescheiterten Kandidaten sind keine
weiteren Echtzeitläufe zur Aktivierungsentscheidung erforderlich. Die
pausengestützte Vorabtranskription wird gesondert qualifiziert. Ein späterer
Kombinationsversuch benötigt einen eigenen Prospekt und vollständige Belege;
dieses Ergebnis beweist weder dessen Erfolg noch dessen Scheitern.
