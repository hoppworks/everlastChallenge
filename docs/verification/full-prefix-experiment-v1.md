# Vollständige Prefixrevisionen: prospektiver Messvertrag

Stand: 18.10.2026. Getrennter experimenteller Messmodus, Produktion aus.
Implementationsgrundlage: `c78b4da` (Runtime) und `1026947` (Cache/Receipts).
Noch kein bestandener nativer Qualitäts-, Leistungs- oder Ressourcennachweis.
Die verworfene Suffixvariante und ihre unveränderten Fehlerbelege bleiben erhalten.

## Algorithmus und Identität

Die Kandidatenvariante verarbeitet verfügbare kumulative Bereiche `0..cut`.
Zwischenresultate ersetzen nur die interne Revision. Ausschließlich der finale
vollständige Auftrag `0..N` nach AudioReleased, Drain und Flush finalisiert.
Eine leere neue Suffixmenge ersetzt diesen Auftrag nicht. Die Referenz verarbeitet
nur die vollständige Aufnahme. Beide Arme verwenden dieselbe Engine-, Modell-,
Sprach- und Decoderkonfiguration und dieselbe Initialisierungsstrecke.

Ein Cachelookup folgt immer der aktuellen normalen Validation, Energyprüfung,
VAD und Vorbereitung. Nur exakt gleiche vorbereitete f32-LE-Samples, Framezahl
und Sprache erlauben einen Hit innerhalb derselben SID/Generation/Tokenepoche.
Jeder Warmup- und Messstart besitzt einen neuen Scope. Textgleichheit, Stille am
Ende und gleiche VAD-Entscheidungen allein erlauben keine Wiederverwendung.
Fehler oder Widerruf dürfen keinen alten Text als Ersatz liefern.

Vor dem ersten gewerteten Lauf sind neue Mode-/Parameteridentität, vollständige
relevante kompilierte Quellen, Binärdatei, Modell und Eingaben einzufrieren.
Der alte Validator für disjunkte Suffixbereiche ist hierfür ungeeignet.
Initialisierung auf dem Developer-Caller und Decode auf dem seriellen Worker
müssen gegebenenfalls ausdrücklich so ausgewiesen werden.

## Erforderliche Belege je tatsächlichem Lauf

- Alle zeitgesteuerten Veröffentlichungen, Stop, Drain/Flush, AudioReleased,
  tatsächlichen Jobstarts/-abschlüsse und finalen Controllerzeitpunkte.
- Jeder vorgezogene Auftrag beginnt bei null, endet am belegten Cut und benutzt
  ausschließlich bereits veröffentlichte Frames. Ein Cut `v + 3200` verlangt
  die tatsächliche Verfügbarkeit mindestens bis `v + 8000`, also `cut + 4800`.
  Die Cuts steigen; wiederholte stille Blöcke erzeugen keine doppelte Revision.
- Der letzte Auftrag deckt exakt `0..N` ab und folgt der finalen Audiofreigabe.
  Zwischenrevisionen haben keinen finalen Textzeitpunkt und keine native Ausgabe.
- Jeder Cachebeleg trägt aktuelle SID/Generation/Job-ID sowie, falls die
  Vorbereitung erreicht wurde, vorbereitete Framezahl und PCM-Hash. Andernfalls
  bleiben diese Felder ehrlich `null`. Ein Hit nennt einen früheren nativen Ursprungsjob
  derselben Epoche mit gleicher Vorbereitung und behauptet keinen Decode- oder
  NativeFull-Versuch. Ein Miss führt die normale frische Dekodierung aus.
- Die gespeicherten Audiodaten bleiben global auf aktives Journal plus einen
  tatsächlichen Jobsnapshot begrenzt. Cancel/Neustart muss auch bei gehaltenem
  altem Decode die Freigabe vor dem nächsten Snapshot belegen. Abschluss und
  Join aller tatsächlichen Besitzer sind erforderlich.

Die Summe kumulativer Jobframes darf größer als N sein. Session-RTF ist die
Summe tatsächlich gemessener Decodeaufrufzeiten geteilt durch `N / 16000`.
Bearbeitete Jobframes werden zusätzlich separat ausgewiesen. Ein Cachehit hat
weiterhin eine gemessene Aufrufzeit; fehlende native Zeit ist kein Nullmesswert.
Stop-bis-Text enthält weiterhin Warteschlangen, Drain, Flush und Restarbeit.

## Reihenfolge und unveränderte Gates

Zuerst ein kurzer serieller Modellversuch mit tatsächlichem Worker: je zwei
gebundene Prefix-/Whole-Eingaben, anschließend Whole unter neuer Session.
Die vorbereiteten PCM-Hashes entscheiden über Hit/Miss; beide neuen Sessions
müssen tatsächlich nativ dekodieren. Dies ist eine Diagnose ohne L-Nachweis.

Danach vollständiger zeitgesteuerter Qualitätsvergleich aller 100 Fälle in
beiden Paarreihenfolgen, mit gekennzeichnetem Warmup und Messlauf. Für jede
Sprache und Testklasse gilt weiterhin höchstens 0,5 Prozentpunkte WER-Anstieg;
keine neuen Anker-/Segmentfehler. Keine Mittelung über Klassen oder Reihenfolgen.
Absolute clean-WER ≤ 10 % je Sprache und alle Stille-/Leertextpflichten des Plans
gelten zusätzlich; sie werden im ausführbaren Freeze ausdrücklich abgebildet.
Nur nach bestandenem Qualitätsgate folgt die Leistungsqualifikation.

Für L folgen zwei vollständige unabhängige Paare Referenz→Kandidat und
Kandidat→Referenz. Jeder Konfigurationsblock enthält im selben Prozess einen
Warmup und sechs Fälle mit je 30 Messwiederholungen. Beide Paare verlangen
mindestens 10 % L-Gewinn; der Kandidat muss bei jedem der sechs Fälle Median
≤ 1 s und p95 ≤ 1,5 s erfüllen. Zusätzlich bleiben die Wiederholungen der
übrigen 94 Kandidatenfälle mit mindestens zehn warmen Messungen je Fall sowie
Ressourcen-, Cancel- und Maximaldauergates
erforderlich. Alte Referenzzahlen ersetzen den neuen gebundenen Vergleich nicht.

Zeitfenster dürfen zwischen vollständigen Paaren liegen. Kalenderpause allein
ist keine Setupänderung; geänderte Hardware-, Strom-, Last-, Quellen- oder
Parameterbedingungen verlangen einen frischen betroffenen Vergleich. Keine
Fortsetzung unvollständiger Prozessblöcke, selektive Wiederholung oder Mischung
alter und neuer Kandidaten. Fehler und nicht ausgeführte Zeilen bleiben erhalten.
P06 und die finale Modellwahl bleiben bis zur vollständigen Abnahme offen.

## Übertragung zwischen Qualitäts- und Leistungswerkzeugen

Der gebundene Single-Runner-Qualitätsnachweis bleibt seiner tatsächlichen
Binärdatei und Revision zugeordnet. Ein zusätzlicher Batchwrapper für die
zentralen Leistungsblöcke erhält einen eigenen prospektiven Quellen-, Parameter-
und Binaryfreeze. Beide Leistungsarme und beide Paarrollen verwenden dieses
identische Batchbinary. Alte Referenzzahlen ersetzen keinen dieser Blöcke.

Die Übernahme des Qualitätsnachweises verlangt einen expliziten Hashvergleich
aller gemeinsamen RecordingOwner-, Controller-, Cache-, Engine-, Service- und
Runnerquellen gegen den Qualitätsfreeze. Der neue Wrapper muss unabhängig
dieselbe Konfigurationsauswahl, Workerinitialisierung, Runtime und
`one_full_prefix`-Strecke sowie eine frische Session je Slot belegen.
Die zusätzlichen Batchslots ändern weder Eingaben noch Decoderalgorithmus.
Ein zusätzlicher Wrapper allein verlangt damit keinen neuen Korpuslauf;
Änderungen der gemeinsamen Qualitätsalgorithmus-, Eingabe- oder Decoderstrecke
invalidieren dagegen die betroffenen Nachweise. Wrapperprüfung und tatsächliche
Leistungsabnahme bleiben erforderlich; ein neuer Build allein belegt beides nicht.
