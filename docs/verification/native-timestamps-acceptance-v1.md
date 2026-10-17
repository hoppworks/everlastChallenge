# P05: unveränderter 90er-Korpus mit nativen Zeitmarken

Stand: 17.10.2026. Freeze945a1c1, geprüftes Entwicklerbinary eb3378f.
Alle90 Fälle, Audiodaten, Referenzen, Klassen und Reihenfolge unverändert.
Bereits exponierte Fälle, kein neuer Blindtest oder nachträgliches Korpustuning.

Ursprünglicher Launcher41746 terminal. Referenz PID2244
17:06:01.600464–17:07:46.838348 UTC, Small PID2341 anschließend bis
17:08:20.419061 UTC. Beide Inferenz-/Auswerterpaare Exit0, alle180 Fälle statusok,
keine Lade-/Service-/Cleanupfehler. Je vier synthetische Leerkontrollen ohne Text;
daraus folgt keine natürliche Raumrauschabnahme. Sidecar bestätigt
noTimestamps=false und suppressNonSpeechTokens=false.

| Modell | DE clean | EN clean | DE challenging | EN challenging |
|---|---:|---:|---:|---:|
| Referenz | 46/915 = 5,0273 % | 34/734 = 4,6322 % | 230/559 = 41,1449 % | 5/99 = 5,0505 % |
| Small | 100/915 = 10,9290 % | 42/734 = 5,7221 % | 352/559 = 62,9696 % | 10/99 = 10,1010 % |

Edits/Referenzwörter, unveränderte Micro-WER. Referenz besteht numerisch beide
Clean-Grenzen; Small verfehlt Deutsch weiterhin. Gegen Baselinev2 verbessern
sich manche Gruppen, andere verschlechtern sich: Referenz ENclean31→34Edits,
Small DEclean96→100 und DEchallenging346→352 sowie ENchallenging9→10.
Kein Verrechnen oder Verwerfen verfehlter Gruppen; keine P06-Freigabe daraus.

Zehn Referenz- und24 Small-Ausgaben ändern sich, alle übrigen Texte/Status bleiben
bytegleich. Der stark verrauschte alte300s-Fall bleibt enthalten: Referenz225/463
Edits mit470 Hypothesentokens, Small340/463 mit421 Tokens. Die Zahlen allein
belegen keine Vollständigkeit oder Ursache; der getrennte kontrollierte300s-Fall
ersetzt diesen Geräuschbefund nicht. Aktuelle Clean-Anker werden separat geprüft.

[Inhaltfreier Bericht](native-timestamps-acceptance-v1.json) bindet sämtliche
privaten0600-Ergebnisse, Metadaten, Logs, Auswertungen und Completion sowie neue
und alte Gruppen. Keine Referenzreparatur, Ausgabeentdopplung oder Klassenänderung.
Produktkonfiguration unverändert. Keine P05-/P06-/L-/RTF-/Ressourcen-/Abbruch- oder
installierte native Abnahme aus dieser Qualitätsdiagnose.
