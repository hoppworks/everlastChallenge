# Unabhängiger Readback: Cache256-Runtime, frische 12er-Bestätigung

**Bestanden: 230/230 unabhängige Prüfungen, keine offenen Findings.** Die eng begrenzte Bestätigung erfüllt `prospective12Passed`, `allExecutionChecksPassed` und `semanticReviewComplete`. Die zugehörige Acceptance-Datei (historischer oder lokaler Beleg, nicht mitgeliefert) ist neu geschrieben und an den [vollständigen JSON-Readback](cache-runtime-prospective-readback.json) gebunden. `qualified` bleibt **false** für das Gesamtprodukt. Es wurden keine Modelle, Builds oder neuen Messungen gestartet und keine Rohdaten verändert.

Grundlage ist der vollständig abgeschlossene Prospective-Lauf (historischer oder lokaler Beleg, nicht mitgeliefert). Unabhängig geprüft wurden beide Rohdateien, sämtliche 24 Metadatenzeilen, alle zwölf vollständigen `{id,status,text}`-Objekte, die Logs, die Owner-Receipts, die Auswahl und die Freeze-Kette. Alle zwölf Objekte sind zwischen CPPReference und Cache256-FP16 exakt gleich; es gibt keine geänderten Paare und keinen Bedarf an semantischen Sonderfreigaben.

| Sprache/Klasse | Fälle | CPP-Edits | FP16-Edits | Referenzwörter | WER-Delta |
|---|---:|---:|---:|---:|---:|
| DE / challenging | 2 | 1 | 1 | 50 | 0 |
| DE / clean | 4 | 6 | 6 | 104 | 0 |
| EN / challenging | 2 | 3 | 3 | 32 | 0 |
| EN / clean | 4 | 6 | 6 | 68 | 0 |

Die Distanzen wurden aus den tatsächlichen Texten mit einer eigenen Levenshtein-Berechnung nachgerechnet; Normalisierung und Unicode 13.0.0 bleiben an den eingefrorenen bestehenden Oracle gebunden. Zusätzlich reproduziert der eingefrorene Comparator die gespeicherte Vergleichsdatei exakt. Die Gleichheit bedeutet gleiche vorhandene Fehler, nicht fehlerfreie Transkription.

Die Auswahl ist anhand aller **94 gepinnten früheren Inventare** und beider gepinnten Test-TSVs nachgerechnet. Ausschlüsse: 64 DE- und 52 EN-Satz-IDs. Es sind die ersten vier noch unbenutzten kanonischen Aufnahmen zwischen 4 und 30 Sekunden je Sprache: DE 1713/1714/1715/1717 und EN 1702/1703/1704/1705. DE 1716 ist mit 651840 Frames zu lang und wurde regelkonform übersprungen. Die ersten zwei Aufnahmen jeder Sprache haben zusätzlich eine feste Gain-0,05-Variante. Somit gibt es **acht neue Quellaufnahmen plus vier abgeleitete Varianten**, keine zwölf unabhängigen Aufnahmen.

Alle zwölf WAV-Hashes, PCM-Formate, Framezahlen und Bindungen an die ausgewählten TSV-Zeilen stimmen. Die vier Quiet-PCM-Dateien wurden bytegenau gegen `scale_pcm(original, 0.05)` geprüft. Der unveränderte Vorbereitungscode schreibt die Auswahl exklusiv vor Archive-Extraktion und ASR; die beobachteten Dateizeiten bestätigen Auswahl → WAVs → Manifest → Referenzstart. Große Archive wurden hier nicht erneut gelesen: ihre Prüfung und die F32→PCM16-Konvertierung sind an den eingefrorenen Vorbereitungspfad gebunden. „Frisch“ bezeichnet den Ausschluss aus den bekannten gepinnten Inventaren, keine beweisbare Unkenntnis der Inhalte durch jeden Menschen.

Der Lauf ist auf die bereits akzeptierte Quality100-Prüfung (historischer oder lokaler Beleg, nicht mitgeliefert) gebunden; deren Acceptance lag vor dem ersten Start vor. Alle 3562 registrierten Dateizeugen sind unverändert, alle 3523 kompilierten Quelldateien haben dieselben Zeugen wie im akzeptierten Quality100-Lauf. Modell-, Binary- und Kernelbindungen sind identisch zu diesem vorangegangenen Lauf; große Blobs wurden hier nicht erneut gehasht. Kleine Quellen, Kontrollerkopien, Auswahl-, Manifest-, Qualitäts-, Build- und Run-Bindungen wurden direkt geprüft. Der Build-Receipt dokumentiert erfolgreiches Release-Build mit `mlx-encoder-f16` sowie unveränderte Quellen vor/nach dem Build.

Der Referenzarm lädt das gebundene Q5-CPP-Modell. Der Kandidat verwendet tatsächlich den FP16-Referenzencoder mit F32-Checkpoint und F32-Featurebrücke zum gleichen CPP-Decoder. Alle gemeinsamen CPP-Optionen sind identisch. Der Kandidatenlog enthält genau einen passenden Load-Receipt, **eine Installation von 268435456 Byte Cachebudget auf dem modellführenden Worker** und zwölf Encoder-Nutzungen mit je einem externen Fenster und null nativen Fenstern. Der Referenzlog enthält keine MLX-Ereignisse. Das Budget begrenzt wiederverwendbaren Cache; dieser Qualitätslauf enthält keine Speicherzähler und beweist keine 256-MiB-Grenze für Gesamt-RAM.

Beide Owner-Receipts stimmen exakt mit den Einträgen in `completion.json` überein: Exit 0, reaped, Terminierung bestätigt, keine Cancellation, kein Watchdog, kein erzwungenes Gruppen-Cleanup und keine Beobachtungsfehler. Referenz PGID 97554 endete vor Start des Kandidaten PGID 97577. Beide Gruppen wurden im unabhängigen Readback erneut mit `killpg(pgid, 0)` geprüft und waren mit **ESRCH abwesend**. Load/Cleanup: CPP 1278/3 ms, FP16 6964/14 ms; diese Einzelwerte sind kein Geschwindigkeitsnachweis.

Diese Evidenz erlaubt die nächste geplante Messstufe zu binden. Sie ersetzt keine gepacete Stop-L-/p95-Prüfung, keine RAM-Messung, keinen Energievergleich und keine Gesamtprodukt-Abnahme. Die bestehende strikte Qualitätsgrenze bleibt unverändert.
