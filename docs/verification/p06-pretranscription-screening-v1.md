# P06 – prospektives natives Vorabtranskriptions-Screening

Stand:18.10.2026. **NOT RUN / NOT QUALIFIED.** Ergänzt den bestehenden
[Vorabtranskriptionsprospekt](p06-pretranscription-experiment-v1.md), ohne
Pausenregeln, Qualitätsgrenzen oder Abschlussbedingungen zu ändern.

## Vollständige Qualität und Pipeline zuerst

Unveränderte [100er-Auswahl](../../tests/fixtures/corpus/p06-context-quality-selection-v1.json)
und [Manifest](../../tests/fixtures/corpus/p06-context-quality-manifest-v1.json).
Vier vollständige Blöcke: Reference→Pretranscription→Pretranscription→Reference.
Jeder Fall verwendet einen frischen Prozess der vorhandenen
`paced_runtime_probe`, genau einen Warmup und einen Messlauf. Somit400
Prozesse und800 Ergebniszeilen; kein Falltausch, Retry, Subset oder Nachjustieren.

Die primären Vergleiche1/2 und4/3 verwenden den einzelnen Messlauf je Fall.
Original90 behalten ihre Nenner; zusätzliche10 bleiben separat. Warmups
werden ebenfalls vollständig und separat gegen denselben festen Text und
den jeweils gepaarten Arm geprüft. Nicht in den Nenner mischen und keine
bessere Hypothese auswählen; Warmupfehler bleiben Fehler des Falls. Ganze
geänderte Texte und betroffene Anker gegen die neue gleichcode-Referenz
prüfen; alte Urteile nur bei exakter Status-/Texthashidentität übernehmen.

Ein Messlauf reicht nicht für eine Latenz- oder Ressourcenentscheidung.
Ein negatives Screening darf verwerfen. Ein positives ersetzt weder6×30
zentrale Messläufe noch je10 Messläufe der übrigen94 Fälle oder Ressourcen-,
Langlauf-, Abbruch-, Neustart- und Installationsabnahmen.

## Tatsächliche Ausführung

Beide Arme wählen ihre vollständige Parametervorlage explizit: gleicher
TC-Publisher, Ring, kontinuierlicher DSP, Worker und vollständiger
Referenzdecoder. `flash_attn=false`, `audio_ctx=0`, Modellkontext1500.
Candidate aktiviert ausschließlich die vorhandenen bestätigten Pausen.
Keine andere Schwelle, erzwungener Schnitt oder Textreparatur.

Originale PCM16/16-kHz-Mono-Eingaben im tatsächlichen20-ms-Takt einspeisen;
keine beschleunigte Dateiübernahme und kein Warten auf Inferenz. Spätere
Samples erst nach ihrer wirklichen Publikation nutzen. Die sechs festen
Empty-Fälle ausdrücklich als Empty starten, die anderen94 als Local.
Keine exakte Textreferenzoption an die Probe übergeben: Wortfehler gehören
in die nachfolgende Qualitätsauswertung.

## Freeze und Belege

Vor Start stabile Source einschließlich aller kompilierten Quellen,
Entwickler-Releasebinary, vollständige Referenz-/Candidate-Parameter,
Modelle/VAD, alle100 Inputs und Referenzen, Launcher, Reihenfolge,
400 genaue Kommandos, frische Ausgabepfade und Liveness-Grenzen einfrieren.
Keine App-/DMG-Erstellung oder Produktionsaktivierung.

Alle800 Rows benötigen eindeutige äußere Fall-/Block- und innere Warmup-/
Messlauf-ID, Session/Generation/Sprache, Framegrenzen, erwarteten Abschluss
und passende Parameter-/Sourcebindung. Jede Publikation, vollständige
Framezahl, tatsächlichen Zeitpunkt,10-ms-Timinggrenze, Stop, TC-Snapshots
und Publisher-Join prüfen.

Höchstens601 aktuelle Jobtrace-Einträge, vollständige disjunkte Timeline-
Abdeckung, passende lokale Speicherbereiche und geordnete Hosteinreichung,
Serviceannahme, Decodergrenzen und Ergebnisübernahme. Frühe Jobs benötigen
bei Einreichung mindestens `timeline.end+4800` bereits vollständig
publizierte Frames, finale Jobs mindestens `timeline.end`. Exportierte
Inputbeweise gegen die gesamte Publikationsliste unabhängig nachrechnen.

Summe akzeptierter Jobframes entspricht `engineInputFrames`. Der gemessene
Decoderzeitwert ist nicht identisch mit der Summe der äußeren Worker-
Zeitspannen: diese enthalten Beobachtungsaufwand. Er darf deren Summe nicht
überschreiten; keine fehlenden Werte als null Sekunden interpretieren.
`jobTraceAfterCleanup`, dessen Fehlerfeld, Owner-Join und tatsächliche
Prozessbeendigung zusätzlich prüfen. Fehlerhafte Belege sperren Statistiken.

Bei Fehler Rohdaten behalten und sämtliche folgenden Slots als nicht
ausgeführt markieren. Kein automatischer Neustart oder Wiederholungsversuch.
Verzeichnisse0700, Rohtexte/Logs/Metadaten/Abschlüsse0600; atomare
Veröffentlichung ohne Überschreiben. Keine Hypothesentexte in öffentlichen
Berichten. Während Modellläufen keine anderen Builds, Tests, Modelle oder
Agentenarbeiten; nur den eigenen Lauf beobachten.

## Aufwand und Entscheidung

Die100 Inputs enthalten1717,2964375 Sekunden Audio. Vier Blöcke mit Warmup
und Messlauf benötigen allein fürs Einspeisen13738,3715 Sekunden, etwa3h49min;
Laden, Decoderabschluss und Aufräumen kommen hinzu. Technische Untergrenze,
keine Fertigstellungszusage.

Unveränderte Gates: je Sprache und Klasse höchstens0,5pp Verschlechterung,
clean absolut höchstens10%, keine neuen relevanten Ankerverluste, falschen
Zahlen, verlorenen Negationen oder Segmentauslassungen/-dopplungen. Bestehende
Fehler transparent erhalten; kein neues pauschales Null-Namensfehler-Gate.
Erst nach erfolgreichem Screening die übrigen Qualifikationsmessungen mit
eigenem Freeze durchführen.
