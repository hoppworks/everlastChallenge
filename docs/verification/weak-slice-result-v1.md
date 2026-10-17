# P05: schwache Maske isoliert – vollständige Absatzdopplung

Stand: 17.10.2026. [Vorabfreeze](../../tests/fixtures/corpus/weak-slice-parameters-v1.json)
in 0c4a011, SHA9b298b31a5b17ae8bd02d6acb74e29861dd0bbf5b6c160343df6614b14c90dba.
375360 unveränderte Frames aus dem 300-s-Control, keine Grundlast-/Decoderänderung.

Ursprünglicher Launcher7345/PID95796 terminal Exit0,
16:32:01.645927–16:32:04.310555 UTC; Auswerter Exit0. Keine Lade-/Service-/
Cleanupfehler. Beide vollständigen Quellsätze einschließlich Mindestzahl zwei
und negiertem Periodensystemhinweis erscheinen wortgetreu, jedoch der gesamte
Absatz zweimal statt einmal. 42 Hypothesentokens gegen 21 Referenztokens,
21 Einfügungen / 100 % WER. Der vollständige rohe Text wurde semantisch gelesen;
kein gültiger Vollständigkeitsnachweis trotz erhaltener Einzelwörter.

Die schwache Maske lässt den Inhalt also erkennen, zeigt aber bereits im
23,46-s-Einzelfall einen vollständigen Dopplungsfehler. Damit ist eine reine
300-s-Längenursache weiterhin nicht belegt. Das Original war unter derselben
gebundenen Engine-/Binarykonfiguration einmal wortgetreu; die alte starke Maske
lieferte nur eine Musikmarkierung. Diese Befunde bleiben getrennt erhalten.
Native Segment-/Fensterentscheidungen sind noch nicht beobachtet.

[Inhaltfreier Hashbericht](weak-slice-result-v1.json) bindet alle privaten
0600-Rohdaten. Eine anschließende Messinstrumentierung wurde erst nach
bestätigtem Prozessende begonnen und ist keine rückwirkende Quelle dieses Laufs.
Keine Referenzreparatur, Maskenjustierung, Ausgabeentdopplung oder P05-/L-Abnahme.
