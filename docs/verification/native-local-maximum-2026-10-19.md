# Autonomer lokaler Lauf bis zur Maximaldauer

Öffentliche FLEURS-Datei `fleurs-de-dev-1517.wav` einmal über `afplay`
abgespielt, Exit 0. Standardmikrofon, Rechte und Anzeige unverändert.
Kein manueller Stopp oder Abbruch. Beobachtete native Zustände:

| UTC | Anzeige | Status |
| --- | --- | --- |
| 14:57:46.478 | 0:00 | Aufnahme läuft |
| 15:01:36.148 | 3:49 | Aufnahme läuft |
| 15:02:25.066 | 4:38 | Aufnahme endet bei 5 Minuten |
| 15:02:59.705 | beendet | 5-Minuten-Grenze erreicht · Diktat ist fertig |

Damit sind Vorwarnung während der letzten 30 Sekunden und automatisches
Beenden mit lokalem Ergebnis direkt beobachtet. Die Stichproben messen nicht
den exakten Umschaltzeitpunkt bei 270/300 Sekunden und keinen Hardware-Join.
Der Startbutton war danach wieder verfügbar. Eigenen Testtext anschließend
über „Löschen“ entfernt; leeres Textfeld bestätigt. Keine Mikrofon-Audiodatei
oder erkannter Mikrofontext im Nachweis gespeichert.

Der öffentliche Satz war enthalten, zusätzlich entstanden nicht abgespielte
Textteile nach dem langen Nachlauf. Das ist ein offener Erkennungsbefund,
kein bestandener Qualitätsfall. Ohne gespeichertes Eingangsaudio ist die
Ursache nicht isoliert. Keine erneute Qualitätsreihe oder Parameteränderung
aus diesem Versuch abgeleitet.

Dieser lokale Buttonlauf beweist weder physische globale Hotkeyzustellung
noch das Fenster-/Fokusverhalten eines Hotkey-Diktats. A26 bleibt insgesamt
offen. Formale Paketabschlüsse weiterhin 4/14. Die schnellen kontrollierten
Softwaretests für 270-/300-Sekunden-Grenzen bleiben der normale autonome
Regressionspfad; dieser reale Lauf ergänzt ausschließlich die fehlende
native Beobachtung.
