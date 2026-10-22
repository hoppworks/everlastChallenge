# Isolierter Kandidat für langen Rauschnachlauf

**Entscheidung:** Der anschließend integrierte Softwarekandidat wurde wegen
belegten Verlusts später leiser Audioframes verworfen. Der Produktquellstand
ist wiederhergestellt. [Gegenprobe und Rücksetzung](long-tail-candidate-rejection-2026-10-19.md).

Die autonome Dateidiagnose wurde am 19.10.2026 mit der echten Silero-VAD
und dem unveränderten Reference-Decoder erweitert. Kein Mikrofonzugriff,
keine Produktquellen-, Installations- oder Parameteränderung.
[Messdaten, Quellen und Hashbindungen](long-tail-candidate-2026-10-19.json).
Das Diagnoseprogramm liegt ausdrücklich getrennt unter
`artifacts/long-tail-candidate-v1`, ohne Verbindung zur App-Steuerung.

## Beobachteter Ansatz

Der 60-s-Fehlerfall enthält tatsächlich den vollständigen öffentlichen Satz
plus das nicht abgespielte „Vielen Dank.“. Damit ist jetzt die konkrete
Zusatztext-Symptomatik belegt, über die bisherige reine Größendifferenz hinaus.
Dies beweist weiterhin nicht die genaue Ursache des akustischen Mikrofonlaufs.

Die VAD erhält dieselbe normalisierte Scratch-Eingabe wie im Produkt.
Isoliert untersucht: letztes 32-ms-Fenster mit Wahrscheinlichkeit mindestens
0,01 bestimmen und eine Sekunde Randreserve hinzufügen. Nur die äußere
Endgrenze wird verkürzt; PCM davor bleibt bytegleich, keine inneren Schnitte,
keine Textfilter oder Decoderänderungen. Der vorhandene `corpus_probe`
verarbeitet ursprüngliche und so abgeleitete WAVs durch denselben Reference-Pfad.

| Dateipaar | Wortfehler vorher → Kandidat |
| --- | --- |
| 60 s, niedriger Rauschnachlauf | 2 → 0 |
| 300 s, höherer Rauschnachlauf | 36 → 0 |
| Zweiter öffentlicher Satz bei 40 s, Faktor 0,1 | 0 → 0 |
| Zweiter Satz bei 40 s, Faktor 0,01 | 1 → 1 |
| Zweiter Satz bei 40 s, Faktor 0,001 | 17 → 17 |

Wortvergleich mit unverändertem Unicode-13-Evaluator. Der Faktor betrifft nur
den zweiten Satz; erster Satz und Rauschanteil bleiben gleich. Beim leisesten
Fall erkennt die VAD den zweiten Satz nicht mehr. Auch der bisherige Decoder
erkennt ihn nicht korrekt: Das ist keine erfolgreiche Leisesprache-Abnahme.
Eine zusätzliche rauschfreie Kontrolle desselben sehr leisen zweiten Satzes
liefert ebenfalls vor und nach Zuschnitt kein korrektes vollständiges Diktat.

## Noch kein Produktfix

Die beiden reinen Rauschnachläufe liefern nach dem isolierten Zuschnitt exakt
den öffentlichen Satz. In diesen fünf Paaren steigt der Wortfehler nicht.
Das ist ein brauchbarer Ansatz, aber kein ausreichender Nachweis gegen
abgeschnittene leise Wörter. VAD-Schwelle und Randreserve sind Diagnosewerte,
keine neue freigegebene Produktkonfiguration.

Vor Integration fehlen Verhaltenstests an der wirklichen Transkriptionsgrenze,
eine begründete konservative Anwendungsgrenze sowie die Kontrolle später und
leiser Sprache. Für die vorhandenen qualifizierten Fälle müssen die endgültigen
Decoderinputs verglichen werden: Nur tatsächlich identische Eingaben bei
unverändertem Decoder erlauben die Übernahme alter Resultate. Veränderte Fälle
müssen erneut ausgewertet werden. Keine pauschale Freigabe aus fünf Paaren,
kein neuer mehrstündiger Lauf ohne diese Eingrenzung.

Die installierte normale App und die P06-Referenz bleiben durch diesen isolierten Kandidaten unverändert.
