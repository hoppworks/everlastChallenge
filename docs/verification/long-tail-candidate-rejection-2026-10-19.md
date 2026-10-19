# Rauschnachlauf-Kandidat wegen Verlust leiser Sprache verworfen

Der isolierte VAD-Zuschnitt wurde als begrenzter Softwarekandidat umgesetzt:
Aufnahme mindestens 30 s, mindestens 15 s entfernbarer Nachlauf nach einer
Sekunde Reserve, VAD-Zulassung unverändert ab 0,5, Erhaltung später Fenster
ab 0,01. Decoderparameter und Text unverändert.

## Warum die grünen Tests keine Freigabe ergeben

Der reale gemeinsame Vorbereitungspfad lieferte zunächst RED: 960.000 statt
erwarteter 144.000 Samples. Nach der Änderung GREEN mit ursprünglichem
geliehenem PCM-Präfix und genau einer VAD-Auswertung. Weitere Grenz-/Fehlertests,
49 Transkriptionstests und fokussiertes Clippy bestanden. Ein Modelltest blieb
ignoriert. Diese Prüfungen belegen die gewählte Regel, nicht deren Eignung
für sämtliche tatsächlich gesprochenen Wörter.

Die bekannte Gegenprobe `late-quiet-0.001.wav` enthält einen echten zweiten
öffentlichen Satz ab Sample **640.000**, Länge **168.960 Samples**. Die gewählte
Regel endet schon bei **158.848**. Beide Dauerbedingungen sind erfüllt;
damit würde der gesamte zweite Satz aus dem Decoderinput verschwinden.
Gleichbleibende 17 Wortfehler vorher/nachher ersetzen keinen Erhaltungsnachweis.
Dass der bisherige Decoder diesen Satz ebenfalls nicht korrekt erkennt,
erlaubt kein stilles Entfernen seiner Audioframes.

Der Kandidat verletzt damit die geforderte Erhaltung leiser Sprache und wird
**nicht integriert oder ausgeliefert**. Kein Anheben der Energieschwelle,
kein Filter gegen bestimmte Textphrasen und keine Backendumstellung.

## Rücksetzung und verbleibender Stand

Die zwei geänderten Produktdateien sind wieder bytegleich zu HEAD und zu den
Release-Quellpins. Alle 13 Quellpins des finalen Transkriptionsmanifests
stimmen. Die normale installierte App hat weiterhin SHA256
`c79d838d2c999a907a850217d131b6c1b5b6d0bc6881505cb906b195210eae2b`.
Kein Appwechsel oder neue Rechtevergabe. Patch, Kandidatenquellen und
RED/GREEN-Logs bleiben ausdrücklich als verworfene Diagnose unter
`artifacts/long-tail-integration-v1` erhalten.
[Hashbindungen und Rücksetzprüfung](long-tail-candidate-rejection-2026-10-19.json).

Die vier langen bisherigen Qualitätsfälle hätten nach VAD-Vorprüfung keinen
ausreichend langen entfernbaren Nachlauf. Das ersetzt keine Freigabe eines
Kandidaten mit anderweitig bekanntem Sprachverlust; deshalb wurde keine
weitere Qualifikation gestartet. Der ursprüngliche Rauschnachlauf-Befund
bleibt offen. Andere unabhängige Abnahmen bleiben ausführbar. Stand 4/14.
