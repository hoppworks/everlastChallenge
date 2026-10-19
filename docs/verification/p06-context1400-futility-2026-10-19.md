# Kontext 1400: kurze prospektive Diagnose am 19.10.2026

Keine Leistungs- oder Qualitätsqualifikation; L bleibt null. Produktionspfad
weiterhin unveränderte Referenz. Kein anschließender Korpuslauf gestartet.

Privater eingefrorener Prüfstand: c91dc472a1d2e9dd9fdad94f2d32e03113462300,
Binary SHA256 d8aae78c628ee3fc2d2e67106c6fa7fe6270301f76c078fc2b6b7e4ed711bdc6.
Vier Arme, jeweils ein Warmup und drei gemessene Sitzungen, insgesamt exakt 16.
Geprüft wurden DE10 und DE15 jeweils isoliert und zusammen mit FullPrefix.
Alle Prozesse/Joins, PCM-/Context1400-/Clock-/Trace-Prüfungen und anschließende
Beendigung der Powerassertion erfolgreich. Keine Mikrofonqualifikation.

| Diagnosemedian Stop → Text | Isoliert | Mit FullPrefix |
| --- | ---: | ---: |
| DE10¹ | 1,028620542 s | 1,016391708 s |
| DE15 | 1,063637584 s | 1,164868625 s |

¹ Beide DE10-Arme überlappten das konservativ erfasste Interferenzfenster
06:28:50–06:29:45 UTC: Nutzer hatte die installierte App zum Erneuern ihrer
Accessibility-Zuordnung neu gestartet und zwischenzeitlich ein Diktat abgebrochen.
Diese Beobachtungen sind deshalb keine belastbare Schätzung ungestörter Leistung.
Rohdaten wurden erhalten; keine stillschweigende Entfernung betroffener Zeilen.

Alle zwölf gemessenen Einzelwerte lagen über einer Sekunde. Die drei Messungen
je Arm ersetzen weder 30 Wiederholungen noch p95 oder die vollständige zentrale
Abnahme. Aus dieser Diagnose folgt keine absolute Freigabe und kein nachgewiesener
reproduzierbarer 10-%-Vorteil. DE15 lag nach dem erfassten Interferenzfenster.

DE10: sämtliche acht Texte bytegleich zum jeweiligen historischen Referenzhash.
DE15: alle acht Texte untereinander gleich, aber abweichend vom historischen
Referenzhash. Das verlangt eine Inhalts-/WER-Prüfung anhand bereits vorhandener
Texte; die Hashabweichung allein ist kein Beleg einer Qualitätsverschlechterung.

Belege lokal unter
`artifacts/p06-context1400-futility-v1/revision-v2/execution-v1/`:
completion.json, readback.json, interference.json und armweise Rohbelege.
Unabhängiger Readback: 5.490 Bytes, SHA256
8289284ef2a779be710324d5c3afed626278ae756e86cdd2af249878c715a42c.
Die Vorprobe rechtfertigt derzeit keinen weiteren mehrstündigen Qualitätslauf.
P06 bleibt offen; kein Gate wurde abgesenkt.
