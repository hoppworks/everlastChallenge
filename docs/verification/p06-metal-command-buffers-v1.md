# P06: isolierter Metal-Command-Buffer-Vergleich

Die Hypothese war ein geringerer CPU-Aufwand bei der Übergabe an Metal durch
zwei zusätzliche Encoder-Worker statt einem. Der gepinnte Backendcode bietet
diese Einstellung getrennt von den vier Inferenzthreads. Kein Modell-,
Audiokontext-, Flash-Attention-, Decoder- oder Produktparameter wurde geändert.

Ein getrenntes Derivat der bereits geprüften Exact-State-Diagnose ändert
ausschließlich zwei Initialisierungen von `n_cb=1` auf `n_cb=2`. 1.582
Quelldateien wurden abgeglichen, genau eine Datei unterscheidet sich. Der
[Patch](../../tools/measurement/metal-command-buffers-v1.patch) wird nicht auf
den Produktbaum angewandt. Geklonter Buildcache mit anschließend gereinigtem
whisper-rs-sys; die neue CMake-Ausgabe enthält die geänderte Datei. Ein
anfänglich beim Kopieren ausgelassenes natives Quellverzeichnis wurde vor
dem erfolgreichen Build unverändert ergänzt.

## Prospektiver kurzer Vergleich

Feste öffentliche Kalibrieraufnahme `fleurs-de-dev-1510.wav`, unveränderte
Modellgewichte und Diagnoseparameter. Reihenfolge vorab festgeschrieben:
Baseline, Kandidat, Kandidat, Baseline. Je Prozess ein Warmup und drei
Messungen. Alle vier Prozesse beendet, insgesamt 16 erfolgreiche Aufrufe,
darunter zwölf Messungen. Vor jedem Prozess wurde Netzbetrieb festgestellt.
Quellen-, Binär-, Parameter-, Eingabe- und Ausgabebindung stehen im
[strukturierten Beleg](p06-metal-command-buffers-v1.json).

| Paar | Baseline NativeFull-Median | Kandidat NativeFull-Median | Verbesserung |
|---|---:|---:|---:|
| Baseline → Kandidat | 1,028360 s | 1,031120 s | −0,2684 % |
| Kandidat → Baseline | 1,084816 s | 1,083200 s | +0,1489 % |

Alle Texthashes waren gleich. Der zusammengelegte Median ergibt deskriptiv
2,0091 %, ist bei den unterschiedlichen Zeitniveaus der Blöcke jedoch kein
Nachweis einer reproduzierbaren Verbesserung. Beide benachbarten Paare liegen
nahe null und liefern unterschiedliche Vorzeichen.

**Entscheidung:** Keine Ausweitung auf mehrstündige Qualitäts- oder
Leistungsqualifikation; keine Aktivierung. Dieser kleine Kalibriervergleich
qualifiziert weder Korpusqualität noch zentrale Latenz, p95 oder Ressourcen.
Er schließt nur diese Hypothese als derzeit begründeten nächsten Kandidaten
aus. P06 und die verbindliche Ein-Sekunden-Grenze bleiben offen.

Der unabhängige Readback bestätigt sämtliche Quellbindungen, die beiden
kompilierten Initialisierungen im ARM64-Objekt, Eingabe-/Ausgabehashes und
die oben genannten Zahlen. Er bestätigt keine vollständige P06-Abnahme.
