# P05: Diagnose des unveränderten maskierten Fünf-Minuten-Falls

Stand: 17.10.2026. Untersuchung vorhandener Daten, keine neue Inferenz.

Der native Untersuchungsagent prüfte die 21 tatsächlichen Original-Archiv-WAVs
gegen ihre SHA-Bindungen und verglich sämtliche 4.800.000 vorhandenen Samples
im RAM mit der eingefrorenen Gain-/Xorshift-Rezeptur. Reihenfolge, Offsets und
Samples stimmen exakt. Die Referenz entspricht der Verkettung der jeweils
zugehörigen tatsächlichen TSV-Zeilen. Kein Konstruktions-/Reihenfolgefehler
gefunden; das bestätigt keine unabhängig gehörte akustische Referenzwahrheit.

Die 21 Sprachclips enden nach 287,46 s; darauf folgen 12,54 s Rezepturrauschen.
Die Maskierung ist erheblich: acht ganze Clips haben nach Speech-Gain ein
negatives Signal-Rausch-Verhältnis. DE1662 liegt ungefähr bei −7,52 dB,
DE1666 bei −8,19 dB und DE1675 bei −10,15 dB. Diese gerundeten Ganzclipwerte
enthalten Originalpausen. Die damaligen Energiesummen wurden nicht gespeichert;
die Werte beweisen weder Sprachverständlichkeit noch die alleinige Fehlerursache.

Die tatsächlichen Texte lassen ganze Inhaltsblöcke vermissen, darunter DE1662
und die Aussagen DE1673–1675; zusätzlich lokale Sinnkorruption und stellenweise
Dopplungen. Diese Befunde beruhen auf Inhaltslektüre, nicht allein auf Tokenzahl
oder WER. Die bisherigen Rohtexte lokalisieren keine technische Ursache.

Im Rust-Pfad erfolgt Ganzaufnahme-VAD ohne innere Kürzung; die Energie des
maskierten Materials liegt durchgehend weit über den Grenzwerten. Der gewählte
vollständige Bereich wird an die Engine gegeben, alle nativen Textsegmente
werden gesammelt. Die Textgrößengrenze liefert einen Fehler statt stiller
Kürzung. Native Fenster-/No-Speech-/Seekentscheidungen bleiben als mögliche
Ursachen offen. Die NST-Korrektur beseitigt diesen Fehler nicht: Referenztext
im 300-s-Fall ist v1/v2 bytegleich.

Der nächste enge Vergleich prüft DE1662 isoliert: unveränderter Originalclip
gegen exakt seinen bereits eingebetteten maskierten Bereich 272640..648000
(23,46 s), gleiche Referenzkonfiguration. Maskierter lokaler Fehler bei gutem
Original würde Maskierung als hinreichend für diesen Einzelfehler zeigen;
ein guter maskierter Clip lenkt die nächste Untersuchung auf Langkontext und
Fensterentscheidungen. Keine Änderung oder Entfernung des bestehenden Falls,
keine Optimierungsaktivierung oder P05-/P06-Abnahme aus dieser Diagnose.

[Inhaltfreier Bindungsbericht](long-recording-diagnosis-v1.json) erhält Offsets,
Signalwerte, Quellen-/Ausgabehashes und die private 0600-Diagnosebindung. Keine
Rohhypothesen veröffentlicht. Vollständiger Archivhash wurde bei dieser
Untersuchung nicht erneut berechnet; tatsächliche ausgewählte Member wurden
gegen ihre vorhandenen Hashes geprüft.
