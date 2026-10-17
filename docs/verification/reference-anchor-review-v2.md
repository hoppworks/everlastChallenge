# P05: Ankerdelta nach Decoderkorrektur

Stand: 17.10.2026. Gleiche 75 Clean-Fälle und 158 eingefrorene Spans je Modell.
Alle 90 Status-/Textpaare je Modell mit v1 verglichen. Je 71 unveränderte
Clean-Fälle übernehmen exakt die bereits unabhängig geprüften historischen
Urteile; je vier geänderte vollständige Satzpaare samt allen Ankern neu gelesen.
Keine historische Reviewdatei geändert; neue Rohdaten vorher hashgebunden.

| Modell | pass | fail | unclear |
|---|---:|---:|---:|
| Referenz | 155 | 2 | 1 |
| Small | 144 | 7 | 7 |

Die Referenz erhält nun alle 35 Zahlenanker eindeutig und alle 18 Negationsanker.
Der englische Bruchfall EN1682 ist geklärt. Zwei bestehende Entitätsfehler und
eine bestehende Entitätsunklarheit bleiben. DE1675 beschädigt jetzt zusätzlich
das explizite englische Wortbeispiel: lokaler Wortfehler, kein belegter
Segmentierungsfehler. Satzurteile daher 62 pass / 12 fail / 1 unclear; diese
Urteile sind keine neu erfundene allgemeine Nullfehlergrenze.

Small erhält weiter 34 Zahlenanker eindeutig, einen nicht: die Unklarheit
wechselt vom englischen zum deutschen Bruchfall DE1682. Die neue Notation
beweist weder die richtige gemischte Zahl noch einen bestimmten falschen Wert.
Alle 18 Negationsanker bleiben erhalten. Bestehende Namens-/Wortfehler und
Unklarheiten bleiben dokumentiert, keine Optimierung oder Referenzreparatur.

Im Clean-Delta keine neue ganze Segmentauslassung/-dopplung nachgewiesen.
Das ist kein Nachweis der Vollständigkeit des gesonderten 300-s-Falls und
keine gesamte P05-Qualifikation. Ankerverständnis wurde anhand unveränderter
Publisherreferenzen und roher Hypothesen beurteilt, keine Hörprüfung behauptet.
[Inhaltfreier Bericht](reference-anchor-review-v2.json) bindet die neue private
0600-Reviewdatei und alle historischen/neuen Eingaben. Unveränderte Urteile
werden nur bei exakter Rohtext- und Statusidentität weiterverwendet.
