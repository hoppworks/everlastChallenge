# Leistung und Messgrenzen

Der Produktpfad verwendet einen MLX-Encoder und whisper.cpp-Decoder. Lange
Aufnahmen werden vollständig vom nativen whisper.cpp-Pfad verarbeitet. Die
optionale Textbereinigung läuft ebenfalls lokal; eindeutige Korrekturen können
ohne Modellaufruf in Rust abgeschlossen werden.

Der [aktuelle Vergleich](comparison.md) enthält vier einzelne Fallpaare bis
zum tatsächlich beobachteten Textempfang. Drei kurze Fälle sind schneller,
der lange Fall ist langsamer. Die vollständige Inhaltsprüfung besteht nicht
durchgehend. Daraus folgt weder ein allgemeiner Geschwindigkeitsvorsprung noch
eine isolierte Aussage über die Geschwindigkeit der Textbereinigung.

Die [Messwerte](benchmarks/native-comparison-0.3.0.json) benennen Fall,
Quellstand, Zeitintervall und Grenzen. Akustische Wiedergabe, bekannte deutsche
Testtexte und nur eine Beobachtung pro Seite begrenzen die Aussagekraft.
Beobachtungsintervalle sind keine statistischen Konfidenzintervalle.

Erststart, Modellladen, Verarbeitung einer fertigen Audiodatei und die Zeit vom
Stopp bis zum sichtbaren Zieltext sind unterschiedliche Messgrößen. Die
[Softwareprüfungen](autonomous-testing.md) belegen ihre jeweiligen Verträge,
keine allgemeine Latenz- oder Genauigkeitsgarantie.
