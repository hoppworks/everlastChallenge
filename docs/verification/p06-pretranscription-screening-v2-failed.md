# P06 – zweites Vorabtranskriptions-Screening ungültig

Stand: 18.10.2026. **Keine Qualitäts-, Leistungs- oder P06-Entscheidung.**
Der Lauf endete nach einer Akku-Beobachtung an der Fallgrenze. V2 bleibt
unverändert erhalten und wird weder fortgesetzt noch automatisch wiederholt.

[Prospekt](p06-pretranscription-screening-v1.md),
[unveränderter Freeze](../../tests/fixtures/corpus/p06-pretranscription-screening-run-v2.json),
[vollständiges inhaltfreies Archiv](p06-pretranscription-screening-v2-failed.json).
Source `4fa4d936cd35ffcc6055c1c1da9b25bf62ec9a72`, ursprüngliche
Ausführungssession 28893, bestätigter terminaler Exit 1. Der Freeze umfasst
568.608 Bytes mit SHA-256
`96e11df8d0f33c0467f184712b4ad21ed0379ac684a0d5242f4f7fe52c580a81`.

## Erhaltener Umfang

Von 400 geplanten Fallprozessen wurden 186 gestartet: alle 100 Fälle des
ersten Referenzblocks und 86 des ersten Vorabtranskriptionsblocks. Davon
bestanden 185 die gesamte Ausführungsvalidierung; ein Fall scheiterte am
Netzstrom-Gate. Die weiteren 214 Prozesse sind ausdrücklich nicht ausgeführt.
Der zweite Candidate- und der zweite Referenzblock wurden nicht begonnen.

Alle 800 geplanten Zeilen sind erhalten. 370 Zeilen gehören zu den 185
akzeptierten Fällen. Zwei weitere Zeilen tragen ebenfalls Rohstatus `ok`
und bestanden die Pipeline-Prüfung, gehören aber zum wegen der Strombeobachtung
ungültigen Fall. Sie werden nicht zu den akzeptierten Zeilen gezählt.
428 Zeilen sind `unavailable`. Es gibt somit 372 pipelinegültige Rohzeilen,
aber nur 370 Zeilen aus vollständig akzeptierten Fällen.

Alle 400 Fallabschlüsse stimmen mit ihren Einträgen im globalen Abschluss
überein. Alle 558 Rohdateien wurden unabhängig nach SHA-256, Größe und Modus
0600 geprüft. Zusammen mit Freeze-Snapshot, Launch- und Gesamtabschlussdatei
sind damit sämtliche 961 Dateien im Laufverzeichnis erfasst. Das JSON bindet
jeden Fallabschluss und jede Rohdatei; Hypothesentexte, Referenztexte und große
Publikationslisten bleiben in den unveränderten privaten Originalbelegen.

Die Ausführungsmetadaten enthalten 303.640 Publikationen über die 372
pipelinegültigen Zeilen. Framegrenzen, tatsächliche Eingabezeitachse, Stop,
Jobidentitäten und lückenlose globale Abdeckung, Nachweise bereits vollständig
publizierter Samples, Decoder-/Übernahmegrenzen, TC-Snapshots und Joins wurden
nachgerechnet. 358 Zeilen enthalten einen Job, zehn enthalten zwei und vier
enthalten vier Jobs. Dies ist ein Pipeline-Nachweis, keine Qualitätsbewertung.
Die 145 gebundenen Quelldateien stimmen mit dem aktuellen Stand und der
unveränderlichen Source-Revision überein; Binary, Parameter, Auswahl, Manifest
und alle 100 Eingaben passen zu ihren Freeze-Bindungen.

## Konkreter Abbruch

Der betroffene Fall ist
`02-pretrans-086-constructed-de-masked-300s`.
Wrapper PID 52763 und Probe PID 52764 liefen von
12:18:28,756513 bis 12:29:06,864158 UTC. Der Wrapper endete mit Exit 0;
die Prozessbeendigung wurde bestätigt. Warmup und Messlauf haben jeweils
4.800.000 Frames, 15.000 gültige Publikationen, genau einen Job und einen
ordnungsgemäß veröffentlichten Abschluss. Beide Rohzeilen sind `ok` und
`validated=true`; Runtime und Publisher wurden gejoint.

Die Strombeobachtung unmittelbar vor diesem Prozess meldete um
12:18:28,754959 UTC `AC Power`. Die Beobachtung nach dem Prozess meldete um
12:29:06,882439 UTC `Battery Power`. Ausschließlich diese Abweichung erzeugte
den globalen Fehler:

`post-probe AC Power evidence missing or battery observed`

Anfangsbeobachtung und erneute Startup-Prüfung meldeten Netzstrom. Alle 186
Vorher-Beobachtungen und alle 186 Hostbeobachtungen der Probe vor
Modellinitialisierung meldeten ebenfalls Netzstrom. Bei den Nachher-Prüfungen
stehen 185 Netzstrom-Beobachtungen genau einer Akku-Beobachtung gegenüber.

Diese diskreten Beobachtungen bestimmen den Zeitpunkt des Wechsels innerhalb
des Prozesses nicht. Sie beweisen keinen durchgehenden Netzbetrieb und keine
Schlaf-, CPU-, GPU- oder Hardwareursache. Das frühere Deckelschlaf-Ereignis
aus V1 wird nicht auf diesen Lauf übertragen.

## Aufräumen und Konsequenz

Der globale Abschluss bestätigt das Ende der eigenen Idle-Sleep-Anforderung.
Ein zusätzliches ausschließlich lesendes `ps`-Audit um
12:33:19,067667 UTC fand keine der 374 gebundenen PIDs mehr vor:
186 Wrapper, 186 Probes, Launcher 25640 und caffeinate 25937. Es gab bei diesem
Audit keinen sichtbaren PID-Konflikt; es wurde kein Prozess signalisiert.
Das JSON hält den Beobachtungszeitpunkt und jede geprüfte PID fest.

Die unvollständige ABBA-Ausführung erlaubt weder eine Corpus-Teilmenge als
Ersatz für den geplanten Vergleich noch eine Latenz-, Ressourcen- oder
Aktivierungsentscheidung. Die zwei pipelinegültigen Zeilen des gescheiterten
Falls heilen das Netzstrom-Gate nicht. Ein weiterer Versuch benötigt einen
eigenen prospektiven Freeze und eine erneute Bereitschaftsprüfung. V1 und V2
bleiben vollständig erhalten; Pausenregeln, Eingaben und Abnahmegrenzen werden
nicht geändert.
