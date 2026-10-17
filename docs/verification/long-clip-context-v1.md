# P05: isolierter Original-/Maskenvergleich DE1662

Stand: 17.10.2026. Einmalige diagnostische Referenzinferenz, kein neuer Blindtest.

Parameter und semantische Fragen vorab in 6f79229 festgehalten, SHA
`7d57e997a5ea5a1aa8d4a44b9b1c1a55cb18b3be59b55a9d6dac6e8af5094794`.
Unabhängiger Preflight bestätigte Originalclip und byteexakten eingebetteten
PCM-Bereich 272640..648000 des unveränderten 300-s-Falls, je 375360 Frames.
Gleiche Referenzkonfiguration und Binary wie im 90er-v2-Lauf; kein weiterer
Gain, keine Rauschänderung oder Referenzkorrektur.

PID94578 terminal Exit0, 16:24:05.639393–16:24:09.303365 UTC, Auswerter Exit0.
Beide Fälle statusok, keine Lade-/Service-/Cleanupfehler. Original: 0/21 Edits,
beide vollständigen Aussagen wortgetreu, Mindestzahl zwei und negierter Hinweis
auf Elemente im Periodensystem erhalten. Maskierter Ausschnitt: 21/21 Edits;
nur eine Musikmarkierung, keine der beiden Aussagen enthalten.

Damit ist die vorhandene Maskierung für diesen lokalen Inhaltskollaps bereits
ohne 300-s-Kontext hinreichend. Das ist kein Beweis, dass sämtliche Fehler der
Langaufnahme dieselbe Ursache haben, und keine Lokalisation eines Journal-,
Rust-Textassembly- oder nativen Fensterfehlers. Eine niedrige WER wird hier
nicht mit semantischer Vollständigkeit gleichgesetzt: beide Rohsatzpaare wurden
gelesen. Der vorhandene 300-s-Fall bleibt unverändert und nicht abgenommen.

[Inhaltfreier Ergebnisbericht](long-clip-context-v1.json) bindet Manifest,
Ausschnitt, Rohtexte, Metadaten, Logs, Auswertung und Completion, jeweils0600.
Keine Rohhypothesen veröffentlicht, kein App-Build oder Mikrofonzugriff,
keine Decoderoptimierung, Referenzreparatur oder P05-/P06-/L-Qualifikation.
