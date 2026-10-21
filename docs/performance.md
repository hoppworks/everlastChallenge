# Leistung und Messgrenzen

Der ausgelieferte Pfad nutzt einen MLX-Encoder mit vollständigem whisper.cpp-Decoder.
Die Verarbeitung erfolgt lokal. Eine universelle Latenzgarantie wird nicht gegeben.

## Reproduzierbare Messstrecken

- [Getakteter Sechs-Fälle-Lauf](verification/paced-six-reference-v4.json)
  und [Qualitätsauswertung](verification/paced-six-reference-v4-quality.md).
- [Ressourcenmessung](verification/runtime-resource-v2.json): die genannte
  Prozess-/Speicherkennzahl ist keine gleichbedeutende Footprint-Garantie.
- [Audio-/Inferenzverträge](autonomous-testing.md): feste Samples, Parameter,
  Zeitgrenzen und getrennte Software-/OS-Nachweise.
- [Messwerkzeuge](../tools/measurement/README.md) für erneute kontrollierte Läufe.

Messwerte gelten für ihre dokumentierte Revision, Modellwahl und Maschine.
Start eines neuen Prozesses, Modellbereitschaft, warme Transkription und
Ende-zu-Ende-Zeit bis zum sichtbaren Text sind verschiedene Größen.

Die jüngste UI-Änderung wurde durch [native Bedienungstests](verification/palette-2026-10-21.md)
verifiziert. Dafür wurde keine neue ASR-Leistungskampagne behauptet. Entfernte
interne Vergleichsunterlagen dienen nicht als Beleg öffentlich behaupteter Zahlen.
