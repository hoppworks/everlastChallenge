# Leistung und Messgrenzen

Der ausgelieferte Pfad nutzt einen MLX-Encoder mit vollständigem whisper.cpp-Decoder.
Die Verarbeitung erfolgt lokal. Eine universelle Latenzgarantie wird nicht gegeben.

## Vergleich mit Voicely

Der [separate Vergleichsbericht](comparison.md) erläutert den historischen
Datei-/CLI-Vergleich: 36,4–39,1 % kürzere Prozessmediane auf drei deutschen
Aufnahmen, einschließlich Laden und Aufräumen. Er enthält die 30 Messzeilen,
Nachrechnung und Grenzen. Das ist kein End-to-End-Vergleich der aktuellen Apps.

Der [native Kandidatenvergleich vom 22.10.2026](verification/native-comparison-2026-10-22.md)
ergänzt 24 beobachtete Ausgaben beider Apps. Zwei saubere Lesefälle sind beim
Kandidaten schneller; Korrekturfälle dauern länger und zeigen unterschiedliche
Inhaltsverluste. Daraus folgt kein allgemeiner Geschwindigkeitsvorsprung.

## Weitere Messstrecken

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
verifiziert. Dafür wurde keine neue ASR-Leistungskampagne behauptet. Der öffentliche Vergleich enthält einen bewusst begrenzten, nachvollziehbaren
Datenauszug. Interne Analyseunterlagen bleiben ausgeschlossen.
