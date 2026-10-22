# Messwerkzeuge und historische Adapter

## Aktueller Einstieg

`node --import tsx tools/measurement/summarize-helper-comparison.ts` prüft die
veröffentlichten Messzeilen und berechnet den historischen Helfervergleich neu.
Der Befehl startet keine Modelle und keinen neuen Benchmark. Methodik,
Ergebnisse und Aussagegrenzen stehen in [Vergleich](../../docs/comparison.md).
Weitere aktuelle Mess- und Auswertungsbefehle stehen unter
[Entwicklerwerkzeuge](../../docs/tooling.md).

## Historische Patches

Die Patch-/JSON-Paare in diesem Ordner sind technische Artefakte früherer
Versuche. Sie gehören nicht zum normalen App-Build und sind nicht gegen den
aktuellen Hauptzweig anzuwenden. Jeder Patch ist ausschließlich an den in
seinem Begleit-JSON genannten Quellstand gebunden. Diese Dateien bewahren
Herkunft und Implementierung; sie belegen keine aktuelle Produktfunktion.

- `full-prefix-resource-v2`: isolierter FullPrefix-Ressourcenadapter. Apply und
  Reverse-Apply wurden gegen die gebundene Basis geprüft; daraus folgt kein
  erfolgreicher nativer Build oder Modelllauf dieses Adapters.
- `exact-state-counter-v2`: zustandsbezogene Messzähler; Umfang und Nachweise
  stehen im [zugehörigen Bericht](exact-state-counter-v2.md).
- `context-1400-calibration-v2` und `metal-command-buffers-v1`: experimentelle
  Varianten, keine Auswahlmöglichkeiten der veröffentlichten App.

Die früheren Python-Kampagnenstarter sind stillgelegt. Für die damalige
Reproduktion ist der jeweilige historische Quellstand einschließlich seiner
Werkzeuge erforderlich. Ein neuer Versuch benötigt eigene Quell-, Modell-,
Parameter- und Ausgabebindungen; alte Messwerte werden dadurch nicht erneuert.
