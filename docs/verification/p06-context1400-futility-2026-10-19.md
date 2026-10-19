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

## Inhaltlicher Offline-Readback

Die vorhandenen Texte wurden danach ohne weitere ASR gegen das tatsächliche,
SHA-gepinnte Fleurs-Gold und die eingefrorene quality.py-Normalisierung
(`/usr/bin/python3`, Unicode 13) ausgewertet. DE15 enthält vier zusätzliche
normalisierte Tokens gegenüber dem historischen Output, keine bloße
Interpunktionsabweichung. Gegen 24 Goldwörter steigt die Einzelfall-WER von
I0/D1/S1 = 2/24 (8,3333 %) auf I4/D1/S1 = 6/24 (25 %). Alle acht aktuellen
DE15-Zeilen stimmen miteinander überein, ebenso die jeweils 30 historischen
Reference-/FullPrefix-Zeilen. Die aktuelle Single-/Combo-Variante liefert
identische Ergebnisse. DE10 bleibt unverändert: I1/D0/S3 = 4/21 (19,0476 %).

Dies ist ein Urteil über genau zwei vorhandene Fälle, keine Korpus-WER und
keine nachträgliche Qualifikation. Zusammen mit der verfehlten Diagnose-Latenz
rechtfertigt es keine Aktivierung oder weiteren langen Korpuslauf dieser
Variante. Die stabile Produktionsreferenz bleibt unverändert.

Neuer privater Readback: `execution-v1/de15-quality-readback-v1.json`, 16.168 Bytes,
SHA256 `cd0981d96d601e8c55ef5891d83c4970531747bba435559d6f36702eb8ba4b8c`.
Zwölf Eingabebindungen erneut überprüft, Golddatei bytegleich mit der originalen
Fleurs-TSV. Alte Belege unverändert; keine erneute Transkription.
