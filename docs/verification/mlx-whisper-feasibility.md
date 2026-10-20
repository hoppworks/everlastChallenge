# MLX Whisper: Aufwand und Dependency-Risiken

Stand: 17.10.2026. Motivation: Performance-Boost für Everlast Dictation,
insbesondere kürzere Zeit von Stop bis zum vollständigen Text.
Quellenrecherche und statische Prüfung des vorhandenen Projekts; keine
Installation, Inferenzmessung oder Sicherheitsfreigabe der Kandidaten.

## Ergänzung: MLX direkt aus Rust

Die erste Fassung betrachtete die beiden genannten Python-Pakete zu eng.
**MLX benötigt keine Python-Laufzeit.** Für dieses Rust/Tauri-Projekt sollte
zuerst ein nativer Rust-Whisper-Kandidat geprüft werden. Die unten genannten
Python-Risiken und 8–15 Entwicklertage beziehen sich auf den beschriebenen
Python-Umbau und dürfen nicht pauschal auf jede MLX-Integration übertragen werden.

- [mlx-rs](https://github.com/oxiglade/mlx-rs) bietet inoffizielle Rust-Bindings
  mit Metal/Accelerate unter MIT OR Apache-2.0. Die Bindings verwenden
  `mlx-sys` und die C/C++-Bibliotheken von MLX; sie benötigen keine Python-
  Inferenzlaufzeit. MLX selbst stellt die [C-API mlx-c](https://github.com/ml-explore/mlx-c)
  bereit. Die Bindings allein sind noch keine vollständige Whisper-Pipeline.
- Es gibt konkrete Rust-Whisper-Implementierungen: beispielsweise verwendet
  [wgo](https://github.com/salernoelia/wgo) laut README Whisper Turbo Q4 über
  MLX auf Apple Silicon. Das [Cargo-Manifest](https://github.com/salernoelia/wgo/blob/main/Cargo.toml)
  bestätigt `mlx-rs 0.25.3`, einen Tokenizer und einen lokalen `mlx-sys`-Patch
  zur Metal-Kernel-Verpackung. Dies ist ein Quellenbeleg für den nativen Weg,
  keine unabhängige Qualitäts- oder Performanceabnahme dieser Anwendung.
- Das separate Projekt [mlxrs 0.2.0](https://crates.io/crates/mlxrs/0.2.0) bietet
  ein optionales `whisper`-Feature. Im veröffentlichten Paket liegen tatsächlich
  [Modell, Encoder, Decoder, Tokenizer und Audio-Frontend](https://docs.rs/crate/mlxrs/0.2.0/source/src/audio/stt/models/whisper/),
  nicht nur Bindings. Es verlangt Rust ab 1.95 und damit mehr als die aktuell
  im Projekt festgelegte Version 1.93; eine Toolchain-Änderung wäre gesondert
  zu prüfen. Beam Search ist im Release noch nicht implementiert. Sein
  [README](https://github.com/findit-studio/mlxrs) dokumentiert wichtige Grenzen:
  Arrays sind `!Send`/`!Sync`, und asynchrone Metal-Fehler können den Prozess
  abbrechen. Unser serieller Inferenzworker passt grundsätzlich zum Besitzmodell;
  für diesen Kandidaten müsste aber die bestehende `Decoder: Send`-Anforderung
  überprüft werden. Das Modell wird bereits im Worker initialisiert, daher
  muss sein Zustand nicht zwangsläufig zwischen Threads bewegt werden.
- [vona-mlx-whisper 0.2.0](https://crates.io/crates/vona-mlx-whisper/0.2.0)
  ist eine weitere konkrete native Implementierung über `mlx-rs`, mit Rust
  ab 1.88 und dem ausdrücklich zu aktivierenden Feature `native-mlx`. Vona
  bezeichnet diesen Adapter in seiner
  [Reifegradtabelle](https://github.com/deliberium/vona/blob/master/docs/adapter-maturity.md)
  als experimentell; vollständige Whisper-Kompatibilität ist damit nicht belegt.

Bei einem vollständig nativen Kandidaten entfallen Python, pip, Torch, Numba
und SciPy sowie ein Python-IPC-Protokoll. Zu prüfen bleiben Reife und Wartung
des konkreten Whisper-Ports, Tokenizer/Mel-/Decoder-Parität, Modellformat,
C/C++-/Metal-Build, FFI und Fehlerverhalten. Eine native Integration kann
ebenfalls in einem separaten Rust-Prozess isoliert werden; das wäre eine
bewusste Architekturentscheidung, keine MLX-Voraussetzung.

**Korrigierte Empfehlung:** Native Rust-MLX-Whisper-Kandidaten zuerst auf Build,
vollständige Transkription und vergleichbare Qualität prüfen und dann gegen
Whisper.cpp/Metal messen. Erst danach lässt sich der Aufwand dieses Wegs
belastbar abschätzen. Rust-Bindings allein beweisen keinen Performancegewinn.

## Einschätzung

Ein Wechsel zu MLX Whisper ist technisch machbar, ohne Aufnahme, Hotkeys,
Oberfläche und native Textübergabe neu zu bauen. Für einen Performance-Vergleich
ist `mlx-whisper` der plausiblere Kandidat. Ein vollständiger Wechsel vor einem
solchen Vergleich wäre derzeit nicht durch Messwerte begründet.

Lightning Whisper MLX würde ich aktuell nicht als Produktbasis wählen:
zusätzliche Wartungsrisiken, alter Tokenizer-Pin, ungeklärte Lizenzfreigabe
und eine MLX-Kompatibilitätskorrektur, die im letzten veröffentlichten Paket fehlt.
Diese Bewertung bezeichnet Integrations- und Wartungsrisiken, keine nachgewiesene
ausnutzbare Schwachstelle.

## Ausgangspunkt im Projekt

- `src-tauri/Cargo.toml`: `whisper-rs = 0.16.0` mit Metal. GPU-Beschleunigung
  ist bereits vorhanden; ein MLX-Wechsel wäre kein Vergleich GPU gegen CPU.
- `src-tauri/src/transcription.rs`: warmer Modellkontext, eigener Zustand je
  vollständiger Inferenz, feste Sprache, keine Übersetzung und kein vorheriger
  Textkontext. Eingaben werden geprüft, über Energie begrenzt und kurze Eingaben
  gepaddet. Textausgaben haben eine Größenbegrenzung.
- `src-tauri/src/inference_service.rs`: vorhandene `Decoder`-Schnittstelle und
  serieller Worker sind eine geeignete Integrationsgrenze. Die konkrete
  Initialisierung nutzt derzeit noch direkt `Engine`; ein produktiver
  Backendwechsel braucht dort eine Anpassung. Cancel verwirft Ergebnisse sofort,
  unterbricht aber den schon laufenden Backendaufruf nicht.
- `src-tauri/src/model.rs`: eigenes Modellmanifest mit fester Revision, Bytezahl,
  SHA-256, begrenzten HTTPS-Redirects, atomarem Download und Cache-Sperre.
  Referenz ist `large-v3-turbo-q5_0`, 574.041.195 Bytes im GGML-Format.
- `vendor/whisper-rs-sys/PATCHES.md`: vorhandener lokaler Patch zur SOT-Zeile
  der No-Speech-Berechnung. Auch der aktuelle Backendpfad hat also eigene
  Wartungskosten. Der Arbeitsbaum war schon vor dieser Recherche verändert;
  diese Recherche verändert oder bewertet diese Änderungen nicht abschließend.
- `package.json`: vorhandene Dependency-Gates sind Cargo Deny und npm Audit.
  Ein gebündelter Python-Stack würde davon nicht erfasst.

## Kandidaten und veröffentlichte Dependencies

| Punkt | MLX Whisper | Lightning Whisper MLX |
|---|---|---|
| Paket | `mlx-whisper` | `lightning-whisper-mlx` |
| Letzter PyPI-Release bei Prüfung | 0.4.3, 29.08.2025 | 0.0.10, 02.04.2024 |
| Herkunft | MLX-Contributors / ml-explore | separates, auf MLX Whisper aufbauendes Projekt |
| Lizenzangabe | MIT | keine Lizenzangabe in PyPI-Metadaten; keine eigene LICENSE im geprüften Repository gefunden |
| Integration | Python-API mit Waveform und lokalem Modellpfad | Python-Wrapper mit eigener Modelltabelle und Downloadlogik |

Quellen: [MLX PyPI](https://pypi.org/project/mlx-whisper/0.4.3/),
[Lightning PyPI](https://pypi.org/project/lightning-whisper-mlx/0.0.10/),
[MLX Repository-Lizenz](https://github.com/ml-explore/mlx-examples/blob/796f5b53cab69a3d48a44233ce21aae889e94a08/LICENSE),
[Lightning Repository](https://github.com/mustafaaljadery/lightning-whisper-mlx/tree/5a4d978bcac88af22e612a3e58dfe07dbb3294ba),
[Lightning Setup](https://github.com/mustafaaljadery/lightning-whisper-mlx/blob/5a4d978bcac88af22e612a3e58dfe07dbb3294ba/setup.py).

Die veröffentlichten Pakete deklarieren jeweils neun direkte Dependencies:

```text
mlx-whisper 0.4.3:
mlx>=0.11, numba, numpy, torch, tqdm,
more-itertools, tiktoken, huggingface_hub, scipy

lightning-whisper-mlx 0.0.10:
huggingface-hub, mlx, numba, numpy, torch, tqdm,
more-itertools, tiktoken==0.3.3, scipy
```

Quelle: versionierte [MLX-Paketmetadaten](https://pypi.org/pypi/mlx-whisper/0.4.3/json)
und [Lightning-Paketmetadaten](https://pypi.org/pypi/lightning-whisper-mlx/0.0.10/json).
Die meisten Constraints sind offen; eine Installation zu einem späteren Zeitpunkt
kann andere transitive Versionen ergeben. Die vollständige transitive Liste steht
erst mit gewählter Python-Version, Zielplattform und aufgelöstem Lock fest.

Die Release-Wheels wurden ohne Installation im Speicher als ZIP geprüft:
Torch-Imports innerhalb der Hauptpakete liegen in der Konvertierungshilfe
`torch_whisper.py`; gewöhnliches pip installiert Torch trotzdem wegen der
Metadaten. `transcribe.py` importiert `timing.py`, das Numba und SciPy importiert.
Nur Worttimestamps auszuschalten entfernt diese Imports daher nicht.
Numba bringt unter anderem LLVM-Bindings über `llvmlite` mit;
[Numba-Metadaten](https://pypi.org/pypi/numba/json). Ein abgespeckter eigener
Paketstand wäre zusätzliche, dauerhaft zu pflegende Arbeit.

## Konkrete Risiken und mögliche Begrenzung

| Risiko | Einschätzung für dieses Projekt | Begrenzung bei späterer Umsetzung |
|---|---|---|
| Zweite Laufzeit und native Python-Dependencies | Hoch beim Ausliefern: Python, MLX/Metal, NumPy, SciPy, Numba/LLVM und regulär Torch zusätzlich zum Rust/Tauri-Stack | Eigenes arm64-Runtime-Bundle, exakte Versionen und Artefakthashes, Python-Advisory-Prüfung, Komponenten-/Lizenzverzeichnis |
| Offen aufgelöste Paketversionen | Bei beiden vorhanden; API-/ABI-Verträglichkeit nicht aus erfolgreichem Download ableitbar | Plattformgebundener Lock, reproduzierbarer Build, keine Paketinstallation beim Endnutzer |
| Lightning-Kompatibilität | Letzter Release älter als eine konkrete MLX-API-Korrektur im Repository | Geprüfter eigener Pin/Fork oder Kandidat verwerfen; nicht blind aktuelles MLX installieren |
| Lightning-Tokenizer | `tiktoken==0.3.3` stammt von 2023; veröffentlichte Wheels nur für CPython 3.8–3.11 | Moderne Runtime-Kompatibilität prüfen; ein Update des Pins wäre eigene Integrationsarbeit |
| Lightning-Lizenzfreigabe | Ungeklärt, deshalb vor Weitergabe klären; öffentlich lesbarer Code allein ist keine belastbare Freigabe | Lizenzherkunft und Weitergaberechte nachvollziehbar dokumentieren |
| Modellbezug und Cache | Standard-Hub-Download übernimmt unsere festgelegte Revision-/Hash-/Cachepolitik nicht | Rust lädt vollständiges Modellverzeichnis anhand eines geprüften Manifests, Python bekommt nur existierende lokale Pfade |
| Zusätzlicher Prozess | IPC-Kopien, Startzeit, Fehlerbehandlung, Shutdown und verwaiste Prozesse kommen hinzu | Dauerhafter Worker, begrenztes Protokoll, Sitzungs-/Job-ID, Antwortlimits, Zeitlimits und kontrollierter Neustart |
| Veränderte Decodersemantik | Neue Stillefehler, Auslassungen oder Wiederholungen trotz gleicher Modellfamilie möglich | Eigenständige Kalibrierung und eingefrorener Vergleichskorpus |

Beleg zum Lightning-Kompatibilitätsrisiko:
Im Release wird `nn.QuantizedLinear.quantize_module` verwendet. Der
[Fix vom 08.05.2024](https://github.com/mustafaaljadery/lightning-whisper-mlx/commit/08292be10d9a96307a1ab4b4f6e5a783384b8af5)
stellt auf `nn.quantize` um und nennt Ladeprobleme mit neuerem MLX. Er liegt
nach dem letzten PyPI-Release. Daraus folgt ein konkreter Prüfbedarf, keine
pauschale Aussage, dass jede heutige Versionskombination scheitert.
Die [Tiktoken-Releaseartefakte](https://pypi.org/pypi/tiktoken/0.3.3/json)
belegen das Wheel-Problem; bei neuerem Python droht Source-/Rust-Build oder
Installationsfehler. Das Alter beweist keine Sicherheitslücke.

Beide letzten Uploads wurden laut PyPI ohne Trusted Publishing veröffentlicht.
Das ist eine schwächere Herkunftsabsicherung, kein Hinweis auf eine tatsächliche
Kompromittierung. Ein vollständiger CVE-/Supply-Chain-Audit wurde hier nicht
durchgeführt; insbesondere ist kein vollständiger Python-Lock aufgelöst worden.

## Technischer Umbau

Die naheliegende Variante wäre ein dauerhaft laufender, mit der App gebündelter
Python-Hilfsprozess hinter `Decoder`. Pro Diktat Python und Modell neu zu laden
wäre dem Performanceziel abträglich. Tauri unterstützt das Bündeln solcher
[Sidecars](https://v2.tauri.app/develop/sidecar/); das belegt den Integrationsweg,
aber nicht automatisch die problemlose Verpackung von MLX und seinen Libraries.

Der Rust-Teil könnte normalisierte 16-kHz-Mono-f32-Samples über eine begrenzte
Pipe mit Job-ID übergeben. Textantworten würden weiterhin validiert und an die
bestehende Sitzungs-/Übergabelogik gehen. Die aktuelle 5-min-Grenze entspricht
rund 19,2 MB Roh-PCM; zusätzliche Kopien und die Gesamt-RAM-Nutzung beider
Prozesse müssten mitgemessen werden. Audio sollte dabei im RAM bleiben.
Ein separater Prozess ist keine Sandbox und erhält ohne weitere Einschränkung
die Rechte des App-Nutzers.

MLX Whisper akzeptiert Waveforms direkt; dadurch muss der Dateiladepfad über
ffmpeg für unsere bereits normalisierten Samples nicht verwendet werden.
Die [Audioquelle](https://github.com/ml-explore/mlx-examples/blob/796f5b53cab69a3d48a44233ce21aae889e94a08/whisper/mlx_whisper/audio.py)
und die [Transcribe-API](https://github.com/ml-explore/mlx-examples/blob/796f5b53cab69a3d48a44233ce21aae889e94a08/whisper/mlx_whisper/transcribe.py)
zeigen diese Trennung. Lightning hat zusätzlich einen Wrapper mit
[eigenem Downloadpfad](https://github.com/mustafaaljadery/lightning-whisper-mlx/blob/5a4d978bcac88af22e612a3e58dfe07dbb3294ba/lightning_whisper_mlx/lightning.py),
der nicht direkt unserer Cachepolitik entspricht.

Unser GGML-Modell ist nicht direkt als MLX-Whisper-Modell verwendbar.
MLX braucht passende Gewichte und Konfiguration; Referenzmodell und
Quantisierung müssen neu gewählt und verifiziert werden. Release und main sind
zu unterscheiden: MLX Whisper 0.4.3 lädt `weights.safetensors` oder `weights.npz`;
erst der [Commit vom 15.12.2025](https://github.com/ml-explore/mlx-examples/commit/e52c128d113f10546f0fa391f87edcc58d3880cb)
ergänzt `model.safetensors`. Ein heutiger Hub-Snapshot kann deshalb Anpassung
oder einen jüngeren, explizit geprüften Quellpin erfordern.

Sprache, Transkriptionsmodus, Kontextfreiheit, Sampling, Timestamp-Verhalten und
Stillebehandlung müssen ausdrücklich konfiguriert werden. Identische Zahlen
bei No-Speech/Logprob garantieren keine identische Semantik. Im geprüften
[MLX-Decoder](https://github.com/ml-explore/mlx-examples/blob/796f5b53cab69a3d48a44233ce21aae889e94a08/whisper/mlx_whisper/decoding.py)
wird No-Speech aus der SOT-Zeile berechnet. Das ist zum aktuellen lokalen
Patchthema relevant, ersetzt aber weder eine Releaseprüfung noch Stille-Tests.

Für den Mac-PyPI-Pfad nennt MLX aktuell Apple Silicon, natives Python ab 3.10
und macOS ab 14. Unser Projekt zielt bereits auf Apple Silicon/macOS 27;
die Hardwareausrichtung passt. Die konkrete Kombination ist dennoch ungeprüft.
[MLX-Installationsdokumentation](https://ml-explore.github.io/mlx/build/html/install.html).
Swift/C++-MLX würde Python vermeiden, aber nicht diese Python-Whisper-Pakete
direkt nutzbar machen; ein solcher separater Port ist in der Schätzung nicht enthalten.

## Performance: Was wäre tatsächlich ein Gewinn?

Lightning bewirbt 10× gegenüber Whisper CPP und 4× gegenüber damaligem MLX
Whisper. Das sind historische Herstellerangaben. Das
[README](https://github.com/mustafaaljadery/lightning-whisper-mlx/blob/5a4d978bcac88af22e612a3e58dfe07dbb3294ba/README.md)
nennt Batching, destillierte Modelle und Quantisierung. Höherer Durchsatz durch
Batches bedeutet nicht automatisch kürzere Stop-bis-Text-Latenz eines einzelnen
kurzen Diktats. Die dortige Modelltabelle nennt kein Turbo; ein destilliertes
englisches Modell wäre kein Qualitätsvergleich zu unserem deutschen Diktat.

Ein fairer Versuch trennt:

1. **Backendvergleich:** dieselbe Modellfamilie, Sprache und möglichst
   vergleichbare Präzision/Parameter; Unterschiede der Quantisierung offenlegen.
2. **Produktvergleich:** aktuelle ausgelieferte Kandidatenkonfiguration gegen
   eine konkret ausgewählte MLX-Konfiguration. Eine größere MLX-FP16-Datei gegen
   unser Q5-Modell ist als Produktvergleich erlaubt, aber kein isolierter
   Nachweis eines schnelleren Backends.

Priorität hat Stop bis vollständiger Text bei warmem Modell, einschließlich
IPC, Warteschlangen, Drain/Flush und Ergebnisvalidierung. Kalter Start,
Download-/Bundlegröße, Gesamt-Prozessspeicher und Energieverbrauch getrennt
ausweisen. Kurze Äußerungen, 5/10/15/30 s und 5 min sowie Stille/Rauschen,
Deutsch und Englisch prüfen. Keine Beschleunigung durch weggelassene letzte
Wörter, geänderte Textnormalisierung oder schlechtere Zahlen/Negationen zulassen.

Die vorhandenen Kriterien in `IMPLEMENTATION_PLAN.md`, Abschnitt 13, bieten
bereits den Rahmen: unter anderem warme 5–15-s-Fälle mit Median höchstens 1 s
und p95 höchstens 1,5 s; unabhängige Qualitäts-/Stillefälle. Als vorgeschlagenes
zusätzliches Entscheidungskriterium für den Backendwechsel wäre mindestens
20 % weniger aggregierte Stop-bis-Text-Latenz bei erfüllten Einzelgrenzen und
höchstens 0,5 Prozentpunkten WER-Verschlechterung je Sprache/Testklasse sinnvoll.
Das ist ein Vorschlag, keine bereits beschlossene Projektanforderung. Bei
kleinerem Gewinn könnte die zusätzliche Runtime mehr kosten als sie nützt.

## Aufwandsschätzung

Eigene technische Schätzung für eine mit Rust/Tauri und Python-Packaging
vertraute Person, ein Apple-Silicon-Zielgerät und MLX Whisper als Kandidat.
Keine gemessenen Implementierungszeiten oder Zusage.

| Arbeit | Entwicklertage |
|---|---:|
| Isolierter Vergleich mit vorhandenen öffentlichen Audios, warm/kalt, Qualität und Baseline | 1–2 |
| Dauerhafter Decoder-Hilfsprozess, begrenztes IPC, Lifecycle, Cancel/Fehlerfälle | 2–3 |
| MLX-Modellmanifest, überprüfter Download, Cache-/Offlineintegration | 1–2 |
| Python-/Metal-Bundle, Signatur, reproduzierbarer Build, Dependency-/Lizenzinventar | 2–4 |
| Differentialmessung und migrationsbedingte Produkt-/Installationsprüfung | 2–4 |
| **Gesamt einschließlich erstem Vergleich** | **8–15** |

Die ersten 1–2 Tage liefern eine Fortsetzen/Verwerfen-Entscheidung, keinen
fertigen Umbau. Paketierungsprobleme oder ungeeignete Modelle können die
Gesamtschätzung überschreiten. Allgemein noch offene Produktabnahmen bestehen
zusätzlich; sie sind nicht durch einen Backendwechsel erledigt.
Für Lightning wäre ohne Lizenzklärung und qualifizierten Dependency-Pin keine
belastbare Produktschätzung möglich; ein eigener Fork erhöht die Folgekosten.

Empfehlung für die Python-Variante: Whisper.cpp als Baseline erhalten und
zuerst einen begrenzten Vergleich durchführen. Übergreifend gilt die ergänzte
Priorität am Dokumentanfang: zuerst einen nativen Rust-Kandidaten untersuchen.
Erst bei nachgewiesenem End-to-End-Gewinn bei gleicher brauchbarer Qualität
die Integrations- und Releasearbeit übernehmen.
