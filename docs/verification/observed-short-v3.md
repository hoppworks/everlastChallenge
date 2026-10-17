# P05: Dopplung bereits innerhalb eines akzeptierten nativen Segments

Stand: 17.10.2026. Gemeinsamer Engine-Pfad, direkte serielle Entwicklerdiagnose.
Implementierungad83f3f, Freeze3ce5bc6, neue private Zielpfade observed-short-v3.

Launcher89157/PID98183 terminal, Inferenz Exit0,
16:43:28.843976–16:43:32.897681 UTC; Auswerter Exit0. Beide Fälle statusok.
Beide Texte und Ergebniszeilen sind byteidentisch zu den jeweils gebundenen
historischen Engine-Ergebnissen: Original einmal wortgetreu, schwache Maske
vollständiger Absatz zweimal. Auswertung 0/21 beziehungsweise 21/21 Edits.

Je Fall genau ein akzeptiertes natives Segment mit Rohstart−100730 und Ende3000
Zentisekunden. Das numerische Nichtnegativitäts-/Ordnungsflag ist false und
wird nicht als Ausrichtung ausgegeben. No-Speech-Wahrscheinlichkeiten endlich
im Einheitsintervall. Beide tatsächlichen ASR-Eingaben umfassen375360Frames,
Energierange0..375360, kein Padding; keine Kürzung dieses kurzen Kontrollsignals.

Die vollständige Dopplung steht im rohen Text dieses einen nativen Segments.
Seine direkte Verkettung samt abschließendem Trimmen entspricht exakt dem
finalen Text. Damit ist der Dopplungsort für diesen Kurzfall im nativen Output
vor der Rust-Verkettung lokalisiert. Der innere Mechanismus (EOT, Fallback,
Prompt-Historie oder anderes) und sämtliche Ursachen der300s-Auslassungen
sind damit noch nicht bestimmt. Keine Ausgabeentdopplung oder Parameteränderung.

[Inhaltfreier Bericht](observed-short-v3.json) bindet sechs private0600-Dateien
sowie Zeit-/Framewerte, Textbytes und Segmenttexthashes. Texte bleiben privat.
Verworfene Fenster und exakte Seek-/No-Speech-Entscheidungen sind unverändert
unbeobachtet. Keine L-/P05-/P06-/Ressourcen-/Abbruchqualifikation aus diesem Lauf.
