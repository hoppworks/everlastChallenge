# P05: echter Runtime-Smoke mit isoliertem TC-Publisher

Code80261d3, prospektiver Freeze28aaf24. Originalhandle16938 terminalExit0.
Genau zwei serielle Prozesse: Local23128 von19:29:58.409090 bis19:30:18.337681,
Empty23190 von19:30:18.339778 bis19:30:30.142498UTC am17.10.2026. Beide PIDs
anschließend beendet. Alle Nebenagenten, Builds und Tests ruhten während der
Messung. Keine neue Aufnahme, kein App-/DMG-Build, Hotkey oder native Übergabe.

Je Prozess genau ein Warmup und eine Messung. Derselbe Referenzdecoder und
unveränderte FullParams wie beim früheren RTF-Smoke; geändert ist der explizite
TC-Publisher samt SID-gebundenem Stop-Einstieg. Der normale Parent hält Runtime
und warmen Modellkontext; nur der eigene Publisherthread erhält die feste
TC-Anforderung. Ready vor Parent-Start, Stop/Portdrop vor Nach-Feed-Beobachtung,
bestätigter Publisherjoin vor gemeinsamer Terminalauswertung.

| Gemessene Zeile | Local8,192s | Empty5s |
|---|---:|---:|
| Publikationen |410|250|
| Größte Publikationsverspätung |17.500ns|26.625ns|
| Stopabweichung zur geplanten Grenze |3.125ns|3.083ns|
| Stop bis vollständiger Text |1.030.190.250ns|nicht anwendbar|
| Stop bis akzeptiertes Terminal |1.030.190.250ns|30.250.625ns|
| Tatsächlicher Engine-Aufruf |1.004.785.459ns|277.583ns|
| Engine-RTF |0,12265447497558594|0,0000555166|

Alle vier Zeilen einschließlich Warmups ok; alle1.320 Publikationen vollständig,
frühestens zum geplanten Zeitpunkt und innerhalb10ms. Größte Abweichung über
alle vier Zeilen29.208ns. Alle Capture-/Controlleruhren und Sitzungs-IDs passen;
Engine-Timing bezieht sich auf die tatsächlichen131.072 bzw.80.000Frames.
TC configured/beforeFeed/afterFeed/post überall exakt requested/defaultfalse/
basic2, keine API-/Policy-/Postfehler oder Ausreißersnapshots. Sämtliche rohen
Policy-/CPU-Abfragen bleiben erhalten; sie sind keine kontinuierliche Demotions-
oder Rechenbudgetbeobachtung.

Local stimmt in beiden Zeilen bytegenau mit der bisherigen privaten56B-ASR-
Baseline überein. Diese Vergleichsausgabe ist keine unabhängige Referenz des
gesprochenen Wortlauts. Private Texte bleiben ausschließlich in den0600-
Rohdateien. Empty hat leeren Text, t_text/Stop-bis-Text/summary/L bleiben null;
sein schneller Engine-Aufruf durchläuft den Energie-Leerheitsguard und ist kein
Whisper-Durchsatznachweis. Cleanupjoin Local3.258µs, Empty12.721µs, beide ohne
Fehler. Kein Teilmengenaggregat über fehlgeschlagene Zeilen.

Der [inhaltfreie Readback](paced-runtime-tc-smoke-v1.json) bindet sieben600-
Rohdateien, Freeze und Parameter und enthält alle vier Text-freien Zeilen,
Policy-/CPU-Snapshots, Summaries und kanonische Publikationshashes. Root prüfte
alle1.320 Zeilen,34aktuelle+immutable Quellpins, Binär-/Modell-/VAD-/Fixture-/
Baselinebindungen, tatsächliche Prozess-IDs und alle angegebenen Kennzahlen.

Dies qualifiziert den engen TC-Runtime-Aufbau für weitere vorab eingefrorene
Messungen. Es sind weder die181zentralen L-Zeilen noch die zehn warmen Läufe
jedes Restfalls, eine allgemeine Scheduling-/Latenzzusage oder P05/P06 als
Gesamtpaket abgeschlossen. Frühere fehlgeschlagene Messläufe bleiben erhalten.
