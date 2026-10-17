# Rohinhaltsprüfung der eingefrorenen Clean-Anker

Stand 17.10.2026. Bewertet sind die unveränderten Ausgaben der bestehenden
90er-Abnahmeläufe am Quellstand `c6cb312`, mit aktivem `suppress_nst`.
Die [vor Rohtextansicht festgeschriebene Liste](reference-anchor-inventory-v1.md)
entstand transparent erst nach diesen ursprünglichen ASR-Läufen. Sie umfasst
alle 75 clean-Fälle und 158 N-/Z-/E- und Grenzfallspans; kein Fall wurde nach
Ergebnis entfernt. Die übrigen 15 Fälle sind damit nicht qualifiziert.

Zwei getrennte lokale Agenten lasen jeweils alle 75 vollständigen Rohtextpaare
und prüften die Bedeutung der Anker, Zahlenwert/Einheit/Vergleich, Negationsaussage,
Entität und Orthografie. Ein unabhängiger gemeinsamer Readback prüfte ebenfalls
alle Paare, Fall-/Spanidentitäten und Bindungen. Die Publisherreferenzen sowie
Hypothesen wurden nicht repariert oder normalisiert. Dies ist kein eigenständiger
Hörnachweis und keine neue Decoderkalibrierung.

| Modell | pass | fail | unclear | Negationsanker | Zahlenanker |
|---|---:|---:|---:|---|---|
| large-v3-turbo-q5_0 | 154 | 2 | 2 | 18 erhalten | 34 eindeutig, 1 unklar |
| small-q5_1 | 144 | 7 | 7 | 18 erhalten | 34 eindeutig, 1 unklar |

Das sind **Ankerurteile**, keine Fall-WER oder Vollständigkeitsgarantie. Beim
Referenzmodell betreffen Entitätsfehler DE1684 und DE1736; EN1669 bleibt als
Namensidentität unklar. Zusätzlich wurden elf Sätze mit Wort-/Sinnabweichungen
berichtet, die nicht automatisch eine neue Pipeline-Segmentauslassung belegen.
Beim kleinen Modell bleiben Namens-/Begriffsfehler in DE1671/1678/1684/1689/1692/1736
und EN1670 sowie weitere orthografische/Entitätsunsicherheiten dokumentiert.
Seine bereits verfehlte deutsche Clean-WER wird durch diese Prüfung nicht geheilt.

**EN1682 ist in beiden Modellen numerisch unklar:** Der erste gemischte Bruch
hat keine eindeutige Bruchnotation; die zweite Dimension bleibt erkennbar.
Es wird weder ein sicher anderer Zahlenwert behauptet noch der Anker als
bestanden gezählt. Damit ist eine gesicherte Gesamtankerabnahme weiterhin offen.
Keine verlorene explizite Negation und keine neue vollständige Segmentauslassung
oder -dopplung wurde in diesem Umfang belegt. Einzelne Wort-/Aliasverluste
werden transparent berichtet, ohne eine unbelegte Segmentierungsursache zu erfinden.

## Reviewkorrektur und Provenienz

Der unabhängige Review fand eine inkonsistente Aliasbewertung in EN1670:
Der ausgeschriebene gleiche Organisationsname blieb in beiden Modellen erhalten,
das redundante Akronym fehlte. Nach der vorgegebenen Entitätsregel gilt dies
beiderseits semantisch als pass; der Aliasverlust bleibt als Oberflächenabweichung
sichtbar. Nur der Small-Anker und zwei abgeleitete Zähler wurden korrigiert.
Die ursprüngliche private Bewertung bleibt bytegleich erhalten; `small-review-r2.json`
ist eine neue Datei mit Revisionsbegründung. Der weitere Personenfehler in
EN1670 bleibt fail, ebenso alle anderen Urteile und numerischen Unklarheiten.

Bei der Small-Erstsichtung waren Ergebnis-/Metadatenhash und dokumentierte
Parameterbindung vor der Paaransicht geprüft. Der tatsächliche Hash der
Parameterdatei wurde erst unmittelbar danach zusätzlich bestätigt; er stimmt
mit dem dokumentierten Wert überein. Beim Referenzmodell wurden auch die
Parameterdatei und übrigen Bindungen vor der Paaransicht geprüft. Es wird kein
abweichender zeitlicher Ablauf behauptet.

Der [inhaltfreie Zahlenbericht](reference-anchor-review-v1.json) bindet alle
75 Fallidentitäten je Modell, 158 Span-Hashes/Urteile sowie die sieben privaten
Roh-/Reviewdateien mit SHA256, Größe und 0600. Vollständige Referenz-/Hypothesentexte
und Einzelbegründungen bleiben in den privaten Reviewdateien. Dieses Ergebnis
ändert keine alte WER, keine Quelle, keine Abnahmeschwelle und kein Modellmanifest.
Es ist keine P05-/P06-Qualifikation. Die neu begonnene Untersuchung der konkreten
Tokensperre ist ein separater Diagnosevergleich, kein rückwirkender Pass dieser
historischen Ausgaben.
