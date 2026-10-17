# P05: vollständige Kalibrierung der nativen Zeitmarkenalternative

Stand: 17.10.2026. Freeze8a291f3, unveränderter42er-Kalibrierungskorpus;
keine neue Blind- oder Abnahmequalifikation. Referenz dann Small seriell, pro
Modell alle42 Fälle, alle bisherigen Rohdaten unverändert erhalten.

Launcher79913 terminal. Referenz PID1436 17:00:44.038413–17:01:21.947038 UTC,
Small PID1492 anschließend bis17:01:33.600941 UTC. Beide Inferenz-/Auswerterpaare
Exit0, alle84 Fälle statusok, keine Lade-/Service-/Cleanupfehler. Sidecar meldet
noTimestamps=false und suppressNonSpeechTokens=false. Je sechs Leerkontrollen
ohne unerwünschten Text; geänderte DE-Trailing-Silence-Ausgabe nur Interpunktion.

| Modell / Gruppe | v5 unterdrückte Zeitmarken | Native Zeitmarken |
|---|---:|---:|
| Referenz DE clean | 8/218 | 7/218 |
| Referenz EN clean | 5/207 | 5/207 |
| Referenz DE challenging | 3/147 | 3/147 |
| Referenz EN challenging | 4/135 | 4/135 |
| Small DE clean | 18/218 | 18/218 |
| Small EN clean | 9/207 | 9/207 |
| Small DE challenging | 9/147 | 10/147 |
| Small EN challenging | 5/135 | 5/135 |

Edits/Referenzwörter; herausfordernde Gruppen einschließlich leiser Sprache und
Kontrollen bleiben getrennt. Die Small-Verschlechterung entspricht0,6803
Prozentpunkten; sie wird nicht versteckt oder durch andere Gruppengewinne verrechnet.
Keine P06-Optimierungsfreigabe wird aus diesem Kalibrierungsvergleich abgeleitet.

Sechs Referenz- und neun Small-Ausgaben ändern sich, sämtliche übrigen Texte und
Status bytegleich zu v5. Alle15 geänderten vollständigen Referenz-/Alt-/Neu-Paare
gelesen: Referenz hauptsächlich Interpunktion sowie wertgleiche Zahlendarstellung;
Small unter anderem zusätzlicher lokaler Wortfehler in leiser deutscher Sprache.
Keine neue verlorene explizite Negation oder falsche Zahl in diesen geänderten
Paaren identifiziert; daraus keine gesamte Anker-/Sprachqualifikation abgeleitet.

[Inhaltfreier Bericht](native-timestamps-calibration-v1.json) bindet elf private
0600-Dateien, alle Gruppenwerte, Unterschiede und Completion. Initialisierung und
zusätzliche Beobachtungsallokationen bleiben diagnostisch, kein Init-/RTF-/
Ressourcenvergleich. Produkt weiterhin unverändert. Vor einer Aktivierung bleiben
der unveränderte90er-Korpus, aktuelle Anker und übrige Pflichtnachweise zu prüfen;
300-s-Restfehler und Zusatztext aus dem engen Vergleich bleiben offen.
