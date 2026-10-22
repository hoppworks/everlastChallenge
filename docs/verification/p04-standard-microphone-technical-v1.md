# P04: technische Abnahme mit Standardmikrofon

Stand 19.10.2026, Quelle `2abf49503fe8b8c43fd7916cc5be3cb9aea3be6f`.
[Quell-/Logbindungen](p04-standard-microphone-technical-v1.json).

Die technischen Kriterien dieses Berichts gelten für **Systemstandard**.
Andere Geräte sowie reales Sleep/Wake wurden hier nicht geprüft.

| Kriterium | Wiederverwendeter beziehungsweise neuer Nachweis |
|---|---|
| A06 | Synthetische Raten-/Kanal-, Frequenz-, Impuls-, Rundungs-/Flush-Prüfungen; vorhandene reale Standardmikrofonaufnahme |
| A09 | Vorhandene DSP-, Pausen-/Rand- und Flushverträge |
| A13 | Neuer kontrollierter CPAL-Fehlercallback durch echten RecordingOwner, CpalAudio-Poll/Join und Runtime-Actor |
| A14 | Überlauf-/Journal-/Besitzverträge, [Pufferbudget](audio-buffer-budget-v1.md), [gemischter Ressourcenlauf](runtime-resource-v2.md) |
| Wiederholungen | [100 native Start/Cancel-Folgen](native-lifecycle-2026-10-19.md) plus separater gemischter 100er-Ressourcenlauf; keine Umdeutung zu 100 nativen Stop-Diktaten |

## Geschlossene A13-Testlücke

Der bisherige Verbundtest injiziert NaN und prüft `InvalidSamples`. Er erreicht
nicht den Fehlercallback, den CPAL bei einem Streamfehler aufruft.
`CapturePort::stream_error_callback` extrahiert exakt die bisherige Closure:
ein Arc-Clone beim Aufbau, ein atomarer Gate-Store mit Release im Callback.
`stream_for` und der neue gerätefreie Verbundtest verwenden dieselbe Factory.
Keine zusätzliche Allokation, Logs, Sperre oder DSP im Callback.

Der neue Test startet reale kontrollierte Besitzer, erzeugt zunächst vorhandenen
Text und bestätigt ein gültiges Ziel. Danach wird derselbe Callback mit
`cpal::StreamError::DeviceNotAvailable` ausgelöst. Erwartet und beobachtet:
sichtbarer Runtimefehler, alter Text erhalten, Zieltoken widerrufen, kein
weiterer Decode oder Übergabebefehl, tatsächliches Reaping/Join, neuer
Aufnahmebesitzer vor Drop alter Producerports und geordnetes Ende.
Kein physisches Abziehen oder OS-seitig verursachter Fehler wird behauptet.

## Ausführbares Feedback und Review

- Initiales E0599 bei noch fehlender Factory: Compile-RED, kein Produktbug.
- Beide Connector-Tests grün. Der kontrollierte No-op-Callback-Mutant ergibt
  einen tatsächlichen Laufzeitfehler: erwarteter Fehlerzustand bleibt aus.
  Das belegt Testsensitivität, keinen historischen Produktdefekt.
- Originalbytes nach Mutation wiederhergestellt. Späteres rustfmt faltet nur
  die Methodensignatur; beide historischen Snapshothashes und der aktuelle
  Quellhash sind getrennt gebunden und unabhängig geprüft.
- `npm run check` und `npm test`: Exit 0; 225 Hosttests bestanden, zwei
  bekannte Modelltests ignoriert. Keine Modell-/Korpusmessung daraus.
- Dieselben zwei Connector-Tests auch mit `measurement` bestanden.
- Gepinnter kanonischer Secret-Scan: Exit 0, keine Treffer bei rund 1,86 GB geprüftem Bestand.
- Ein kombinierter unabhängiger Review bestätigt Korrektheit, Standards und
  Spezifikationsdeckung ohne Befund. Die enge Evidenzpräzisierung ist geschlossen.

## Transfer und Grenzen

Der [selektive funktionale Transfer](reference-source-transfer-2026-10-19.md)
und der enge P04-Audit stützen die Wiederverwendung vorhandener Belege.
Audio/DSP waren von `634d6c9` bis `d05ae10` unverändert; die aktuelle
Änderung extrahiert ausschließlich den identischen Fehlercallback und ergänzt
Tests. Pufferbudget und gemessener Prozessverbrauch bleiben getrennt; es wird
keine universelle exakte Heapobergrenze behauptet.
