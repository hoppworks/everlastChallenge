# P05: Ankerprüfung der Zeitmarkenalternative

Stand: 17.10.2026. Parameterfreeze945a1c1, Entwicklerstand eb3378f.
Unverändertes Inventar f55322e: je75 Clean-Fälle mit158 Ankern.
Alle90 alten und neuen Ergebnisse je Modell nach ID, Status und vollständigem
Rohtext verglichen. Nur bei exakter Identität wurde das vorherige Urteil
unverändert übernommen. Alle acht geänderten Clean-Ganzpaare der Referenz und
19 von Small samt ihren Ankern erneut im Satzkontext gelesen.

| Modell | Anker bestanden | Fehler | Unklar | Zahlen | Negationen |
|---|---:|---:|---:|---|---|
| Referenz | 154 | 3 | 1 | 35 bestanden | 18 bestanden |
| Small | 145 | 7 | 6 | 34 bestanden, 1 unklar | 18 bestanden |

Die Referenz hat gegenüber v2 einen zusätzlichen Namens-/Entitätsfehler in
DE1679; bestehende Fehler DE1684 und DE1736 sowie die Unsicherheit EN1669
bleiben erhalten. Ganze Satzurteile:61 bestanden,13 fehlerhaft,1 unklar.
Small löst die bisherige Namensunsicherheit DE1660, behält aber die numerische
Mehrdeutigkeit DE1682. Eine lokale syntaktische Zuordnungsunsicherheit EN1681
ist gesondert vermerkt. Keine bestätigte neue falsche Zahl, verlorene explizite
Negation oder neue vollständige Klauselauslassung/-verdopplung in diesem Umfang
festgestellt. Das ist keine Behauptung vollständiger Texttreue.

Der unveränderte stark verrauschte300s-Fall gehört weiterhin zur schwierigen
Klasse und liegt außerhalb dieser75 Clean-Fälle. Seine WER bleibt im
[numerischen Bericht](native-timestamps-acceptance-v1.md) erhalten; keine
semantische Vollständigkeitsabnahme daraus. Die Quelle der Solltexte ist der
veröffentlichte Korpus; kein unabhängiger Audio-/Labelnachweis und kein neuer
Blindtest. Namensfehler werden berichtet, ohne eine zusätzliche allgemeine
Nullfehlergrenze zu erfinden.

Der [inhaltfreie Bericht](native-timestamps-anchor-review-v1.json) bindet den
privaten Review,26 Eingabedateien, sämtliche180 Identitätsvergleiche und die
Urteilszähler. Historische Rohdaten und Reviews unverändert. Die Alternative
nutzt no_timestamps=false und bleibt ausschließlich Entwicklerdiagnose.
Keine P05-/P06-, Leistungs-, Ressourcen-, Abbruch- oder native Produktabnahme.
