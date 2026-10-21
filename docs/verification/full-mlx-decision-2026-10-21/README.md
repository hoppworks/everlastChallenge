Der reine Decoder mit den exakt aus Q5 abgeleiteten F32-Koeffizienten scheitert
im vorhandenen 90-Fälle-Vergleich: Deutsch schwierig 230 → 258 Fehler bei 559
Referenzwörtern; Deutsch sauber 46 → 51 bei 915. Der ergänzende Vergleich hebt
diesen fehlgeschlagenen Teil nicht auf. Auch F32, Best-of-five und die exponierten
Einzelfall-Kontrollen für unfused Attention, tanh GELU und Beam-five lieferten
keine belastbare Korrektur. Bei en1690 entstand wiederholt „the certification“
statt „decertification“; dies ist eine inhaltliche Bedeutungsänderung.

Ein teilweise bestandenes automatisches Gruppengate ist keine semantische
Freigabe. Es wird keine Aussage getroffen, dass jeder mögliche MLX-Decoder oder
ein anderer Rust-Port ungeeignet wäre. Für unsere aktuelle Implementierung fehlt
der nötige Nachweis. Ohne eine neue begründete Korrekturhypothese ist eine erneute
lange Kampagne derselben verworfenen Varianten nicht sinnvoll.

Die Kopien der Entscheidungs-/Vergleichsdateien sind durch provenance.json an
Quelle und Hash gebunden. Frühere Geschwindigkeitsnachweise gelten für den
Hybrid, nicht für reines MLX. Ein vollständig nativer Rust-/Tauri-Aufbau benötigt
keine vollständige MLX-Implementierung: Die Challenge erlaubt whisper.cpp oder
einen gleichwertigen lokalen Weg.
