# Engine-RTF und erwarteter leerer Runtimeabschluss

Stand:17.10.2026. Begrenzte Erweiterung des vorhandenen Entwickler-Messpfads,
keine Produktions-Telemetrie oder neue App-IPC. Ohne Feature measurement
entfallen Zeitabfragen, Metadatenfelder und Messspur vollständig.

## Getrennte Zeitgrenzen

Der Inferenzdienst misst direkt um den tatsächlich ausgeführten Decoderaufruf.
Completed transportiert dessen Dauer und die wirklich übergebene Samplezahl
zusammen mit den vorhandenen Session-/Generation-/Job-/Bereichstags. Der
Produktionsdecoder ist Engine::transcribe; deshalb umfasst die Zeit auch
Energieprüfung, VAD, Vorbereitung, frischen ASR-State und Textverkettung.
Es ist keine reine native Whisper-Rechenzeit. Warteschlange, Aufnahme,
Controllerabschluss und native Übergabe zählen nicht in diesen Zähler.

Engine-RTF = Decoderaufrufdauer in Sekunden / (übergebene16kHz-Frames /16000).
Fehlende Messung wird nicht zu null. Bei null Frames bleibt die RTF wegen
fehlendem Nenner leer. Der Ressourcenharness hält einen Decoderwrapper vor
Engine an; er beansprucht ausdrücklich keinen Engine-RTF-Nachweis.

Der Controller übernimmt Timing nur mit einem tatsächlich angenommenen
aktuellen Local-/Empty-Ergebnis. Session, Generation, Job und Samplebereich
bleiben durch dieselbe Ergebnisannahme geschützt. Widerrufene, alte, doppelte
oder falsch zugeordnete Antworten dürfen keine neue Messung setzen.
Messstart setzt alle sitzungsbezogenen Werte zurück.

t_terminal entsteht am akzeptierten Local-/Empty-Übergang. Local setzt am
gleichen Zeitpunkt t_text; Empty lässt t_text leer. Das Terminalenum wird
erst mit dem veröffentlichten Runtime-Snapshot sichtbar. So darf der Beobachter
nach erfolgreichem Abschluss nicht noch einen alten Finalizing-Snapshot lesen.
Die deterministische Acceptance→Publish-Gap-Regression prüft diese Reihenfolge.

## Explizite Erwartung und Auswertung

Der vorhandene Runner bleibt standardmäßig Local und verlangt nichtleeren Text.
Die Einzelprobe bietet --expect-empty als ausdrücklichen zusätzlichen Modus;
dieser darf nicht mit einer nichtleeren Textbaseline kombiniert werden.
Erwartungen stammen aus dem vorab ausgewählten Prüfplan, niemals aus einer
nachträglichen Hypothese. Ein unerwartet leerer Local-Lauf bleibt fehlerhaft.

Erwartetes Empty verlangt einen wirklichen leeren Controllerabschluss. Es hat
eine eigene Stop-bis-Terminalzeit, keinen Textzeitpunkt und keinen Beitrag zu L.
Local benötigt tatsächliche Enginezeit; inkonsistente Framezahlen sind Fehler.
Ein fehlender Decoderaufruf bei einem legitimen leeren Abschluss bleibt als
fehlende Enginezeit samt Grund erkennbar. Einzelwerte, Messabdeckung und
vollständige Zusammenfassungen werden getrennt ausgegeben; fehlende oder
fehlerhafte Zeilen erzeugen keine geschönte Teilstatistik.

Die neuen Felder stehen ausschließlich im privaten Entwicklersidecar. Das
allgemeine L-Protokoll, Qualitätsnormalisierung, Eingabezeitachse, Jitterbudget,
Decoderparameter und historische Parameterdateien werden dadurch nicht geändert.
Aktuelle Templates binden die tatsächlichen Quellen und die neuen Messgrenzen.
Vor jedem nativen Vergleich sind aktuelle Binary-/Parameter-/Lauffreezes nötig.
Kontrollierte Service-/Runtime-Tests und Code-Review ersetzen weder diese
Modelläufe noch Mikrofon-/Hotkey-/Übergabe- oder P05-/P06-Gesamtabnahmen.
