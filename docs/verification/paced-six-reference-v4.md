# Central reference v4: vollständige Messung, absolute Medianziele verfehlt

Der einmalige prospektiv eingefrorene Lauf endete mit Exit 0: **181/181 Zeilen erfolgreich** (ein Warmup, sechs Fälle mit je 30 Messungen), **90.250/90.250 Publikationen innerhalb des festen 10-ms-Budgets**. Die sechs p95-Werte erfüllen 1,5 s; alle sechs Mediane überschreiten 1 s. **L = 1,059564354 s**. Das ist ein vollständiger L-Messdatensatz, keine P05-/P06-Qualifikation.

Quelle `80261d31aedcecd58e3cc3e1a95735c98e56259e`, Freeze-Commit `0c059d90e1af2264a0268e1e3df75cd4d694a210`. Parameter, sechs unveränderte Acceptance-Assets und Referenzhashes, Modell/VAD, Releasebinary und alle 44 Sourcepin-Einträge (42 verschiedene Dateien) wurden gegen die eingefrorenen Bytes geprüft; Sourcepins zusätzlich gegen die unveränderliche Git-Revision. Die gebundenen Originaldateien bleiben unverändert und privat (0600).

Originalprozess PID 24669; Start `2026-10-17T19:40:34.940297+00:00`, Ende `2026-10-17T20:13:53.499063+00:00`. Ein warmer Engine-/Runtimekontext, ein expliziter Warmup, anschließend kanonisch DE5/10/15 und EN5/10/15 mit je 30 Läufen. Keine Erfolgs-Untermenge, Wiederholung oder nachträgliche Ersetzung.

| Fall | n | Median (s) | p95 (s) | Median ≤1 s | p95 ≤1,5 s |
|---|---:|---:|---:|---|---|
| DE5 | 30 | 1.026634624 | 1.033979250 | nein | ja |
| DE10 | 30 | 1.089689708 | 1.097672083 | nein | ja |
| DE15 | 30 | 1.102571000 | 1.109820625 | nein | ja |
| EN5 | 30 | 1.017109417 | 1.026232750 | nein | ja |
| EN10 | 30 | 1.042116792 | 1.047285792 | nein | ja |
| EN15 | 30 | 1.079264583 | 1.096148792 | nein | ja |

Gerade Stichprobenmediane sind der Mittelwert der beiden mittleren Werte; p95 ist nearest-rank (29. sortierter Wert bei n=30). L ist das ungewichtete Mittel der sechs Mediane. Warmup ist ausgeschlossen.

Alle Framebereiche, 20-ms-Endframe-Deadlines, Before-/After-Publikationszeitstempel und Abweichungen wurden einzeln nachgerechnet. Kein früher/verspäteter Chunk; größte Publikationsabweichung **30459 ns**, größter Publikationskörper **30250 ns**, größte Stop-Abweichung vom geplanten Ende **20084 ns**. Jeder Stop liegt nach der letzten vollständigen Publikation. Alle 181 akzeptierten Local-Ergebnisse besitzen konsistente Stop-/Text-/Terminalzeiten und Engine-Dauer/Framezahl/RTF. Engine-RTF bezeichnet den gesamten Decoderaufruf inklusive Engine-Vorverarbeitung/VAD, nicht nur den nativen ASR-Kernel.

Alle **181 Publisher sind gejoint**. Die 724 Post-Setup-Snapshots (configured, beforeFeed, afterFeed, after) enthalten defaultPolicy=false, Basic.policy=2 und exakt period/computation/constraint 480000/12000/24000 Ticks bei preemptible=true. Vor Setup ist Basic.policy=1/defaultPolicy=true. Kein Policy-/QoS-Fehler, kein Ausreißersnapshot. Die fehlenden UI-QoS-Flags sind bei diesem expliziten TC-Pfad erwartbar. Das belegt die abgefragten Zeitpunkte, keine durchgehend wirksame RT-Klasse und kein periodisches CPU-Budget.

Publisher-CPU, über alle 181 Zeilen: Feed **41587650000 ns**, gesamter beobachteter Childthread **41595237000 ns** (jeweils User+System). Pro Zeile und ohne Warmup sind die Differenzen im JSON ausgewiesen; sie sind keine Prozess-CPU-/RSS-Messung. Modellladen: 1269 ms. Runtime-Cleanup: joined=true, error=null, **2895 µs**. Die nächste Startbestätigung prüft vorherige Owner-/Decoder-Quieszenz; separate 181 Audio-Joinquittungen sind nicht exportiert.

Die per-run WaitSummary-Maxima sind im JSON erhalten und aggregiert. Da bei gültigen Publikationen keine internen Detail-Wartezeitstempel gespeichert werden, sind deren Phasenmaxima aus Summaries übernommen; alle 90.250 äußeren Publikationszeiten wurden dagegen vollständig rekonstruiert. Keine unbelegte Kernelursache oder kausale Überlegenheit gegenüber den früheren ungültigen Versuchen wird behauptet.

Semantische Qualität einschließlich Originalinhalt/Ankern wird im getrennten v4-Qualitybericht behandelt. Hier werden keine Hypothesentexte veröffentlicht. Mikrofon, Hotkey, Empfänger, Small-Modell, Ressourcen- und Abbruchqualifikation bleiben außerhalb dieses Readbacks. Ein Medianfehlschlag ist gemäß Plan kein zusätzlich erfundenes Verbot einer P06-Untersuchung; Freigabe und finale Auswahl erfordern die tatsächlich geforderten Nachweise und absoluten Ziele.

Der [strukturierte Bericht](paced-six-reference-v4.json) enthält alle 181 inhaltfreien Zeilen, Fixture-/Runidentitäten, Zeit-/Engine-/Policy-/CPU-Belege, Rohdateihashes, Grenzen und rekonstruierte Statistik. Frühere v1/v2/v3-Belege bleiben erhalten.
