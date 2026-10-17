# P05: kontrolliertes 300-Sekunden-Supplement weiterhin unvollständig

Stand: 17.10.2026. Einmaliger Diagnose-Dateilauf, kein L oder Maximaldauerabschluss.

[Rezeptur](long-control-v1.md) und Parameter in 1c0a08a vor Inferenz eingefroren.
Unabhängig geprüft: alle 15.000 Fenster über Energiegrenzen, unveränderte
21 Quellclips, Referenzreihenfolge, Decoder-/Binarybindung und freie Ausgabeziele.

PID95259 / ursprünglicher Launcher11666 terminal, Inferenz Exit0,
16:28:39.359148–16:28:53.162294 UTC. Auswertung Exit0; keine Lade-, Service-
oder Cleanupfehler. 147 Wortedits bei 463 Referenzwörtern (31,7495 % WER),
335 Hypothesentokens. Besser als die starke alte Maske, jedoch kein bestandener
Vollständigkeitsnachweis. Insbesondere fehlen weiterhin beide Aussagen DE1662
zu Legierungen und Elementen im Periodensystem; die Ausgabe wurde vollständig
gelesen. Weitere Block-/Fensterzuordnung wird getrennt untersucht.

Der fehlgeschlagene Kontrollfall bleibt unverändert erhalten. Die erfolgreiche
Erkennung des isolierten Originals und das Versagen des alten stark maskierten
Ausschnitts erklären diesen neuen Kontrollfehler nicht allein. Die schwächer
maskierte isolierte Version wurde noch nicht geprüft. Ein Langkontext-/Fenster-
fehler ist eine Hypothese, keine bereits beobachtete technische Ursache.
Tatsächliche native Seek-/No-Speechentscheidungen wurden bisher nicht erfasst.

[Inhaltfreier Ergebnisbericht](long-control-result-v1.json) bindet sämtliche
privaten 0600-Artefakte samt Rezeptur, Ausgabe und Completion. Keine Anpassung
der Grundlast anhand dieses Ergebnisses, kein Austausch des alten 90er-Korpus,
keine Referenzreparatur, keine P05-/P06-/L-/Ressourcen-/Abbruchqualifikation.
