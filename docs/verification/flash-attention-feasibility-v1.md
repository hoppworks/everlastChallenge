# Flash-Attention im gepinnten Metal-Backend: Machbarkeit

Stand:18.10.2026. **NOT RUN / NOT QUALIFIED.** Separate Quellenrecherche,
keine Implementierung, Aktivierung oder gemessene Verbesserung. Keine Modelle
initialisiert, kein Cargo-/Benchmarklauf. Anwendungsbezug ist die unveränderliche
Revision `688303557f483b6145bf2b4ff871b47537afda3d`, Planbezug `3304146`.
Parallel entstehende Vorabtranskription ist nicht Gegenstand dieser Notiz.
Plan-SHA256 dieses Stands:
`9d514e4e26e8ae12dcb2a11e5e29b6f8147b78fb532bdf0a91ed50c49efecd8f`.

## Ergebnis und genau eine mögliche Variante

Ein isolierter Kandidat ist über die vorhandene sichere API technisch plausibel:
beim Erzeugen eines **neuen** `WhisperContext` ausschließlich
`WhisperContextParameters.flash_attn` von `false` auf `true` setzen. Dasselbe
Reference-Modell, dieselben Metal-Buildfeatures, `use_gpu=true`, GPU-Gerät,
FullParams, Sprache, Energie-/VAD-/Paddingvorbereitung und PCM bleiben erhalten.
`audio_ctx=0` bleibt der Modellstandard1500; keine Kombination mit dem verworfenen
1024-Kontext oder neuer Segmentierung. Dies ist eine untersuchbare Konfiguration,
kein vorhandener Kandidat und keine Zusage, dass jeder Graphknoten auf Metal läuft.

`Engine::load` setzt derzeit ausdrücklich `flash_attn:false`
([App:83–89](../../src-tauri/src/transcription.rs#L83)). Der Rust-Wrapper0.16.0
hat ebenfalls Defaultfalse; das gebündelte C++ hat Defaulttrue. Der Wrapper
übergibt seinen eigenen Boolwert, nicht den C++-Default. Für den historischen
Grund der App-Einstellung ist hier kein Beleg festgestellt; daraus folgt weder
ein Bug noch eine Rechtfertigung, die Baseline still umzustellen.
([Wrapper:458–495,584](../../vendor/whisper-rs/src/whisper_ctx.rs#L458),
[C++:3606–3610](../../vendor/whisper-rs-sys/whisper.cpp/src/whisper.cpp#L3606)).

## Tatsächlicher API-, Graph- und Lebensdauervertrag

- `flash_attn` ist ein **Kontextparameter**, kein `FullParams`-Feld. Der sichere
  Konstruktor übergibt die C-Struktur synchron; C++ speichert `ctx->params=params`.
  Ein Bool erzeugt keinen neuen Callback-/Pointer-Lebensdauervertrag. States
  halten den Kontext per `Arc` und geben ihren nativen State per Drop frei.
  Varianten daher über getrennte neu geladene Kontexte ausführen, dieselben
  seriellen Worker-/Joinverträge behalten; keine interne Kontextmutation oder
  State-Wiederverwendung über die Flaggrenze.
  ([Wrapper:49–64,504–591](../../vendor/whisper-rs/src/whisper_ctx.rs#L49),
  [C++:3722–3725](../../vendor/whisper-rs-sys/whisper.cpp/src/whisper.cpp#L3722),
  [create_state:466–472](../../vendor/whisper-rs/src/whisper_ctx_wrapper.rs#L466),
  [State:16–29](../../vendor/whisper-rs/src/whisper_state/mod.rs#L16)).
- Das Flag ersetzt Encoder-Self-Attention, Decoder-Self-Attention und
  Cross-Attention durch `ggml_flash_attn_ext`; die andere Variante bildet
  Matmul/Softmax/Matmul ab. Cross-K/V-Views und V-Layout unterscheiden sich,
  Audiokontext wird für Flash-Views auf256 Positionen gepaddet. GPU-KV-Padding
  wird im Metal-Pfad32 statt1. Es ist eine Änderung der Backendrechnung, nicht
  nur ein anderer Host-Aufruf desselben Graphen.
  ([C++:1139–1155,2141–2185,2320–2340,2607–2643,2680–2720](../../vendor/whisper-rs-sys/whisper.cpp/src/whisper.cpp#L1139)).
- **Keine behauptete Speicherersparnis:** KV-self/cross/pad werden bereits beim
  Statebau auch ohne Flash reserviert. Fused Attention vermeidet den expliziten
  KQ-/Softmax-Zwischengraphen, Metal reserviert dafür zusätzliche Pad-/Block-/
  temporäre Arbeitsbereiche und Threadgroup-Speicher. Tatsächliche Graph-/State-
  und Prozesspeaks müssen verglichen werden.
  ([Statebau:3384–3428](../../vendor/whisper-rs-sys/whisper.cpp/src/whisper.cpp#L3384),
  [Metal-Puffer:194–199](../../vendor/whisper-rs-sys/whisper.cpp/ggml/src/ggml-metal/ggml-metal.cpp#L194),
  [Scratch:2168–2280,2450–2468](../../vendor/whisper-rs-sys/whisper.cpp/ggml/src/ggml-metal/ggml-metal-ops.cpp#L2168)).
- Die Rechnung ist numerisch nicht als bitgleich belegt: der F16-Kernel nutzt
  Half-Query/Key/Value-Typen und Float für QK, Softmax und Akkumulation; Softmax
  wird blockweise online berechnet. Q5_0-Gewichte bleiben dieselbe Datei; die
  Zwischen-/KV-Typen sind davon getrennt (C++ `itype=F16`). Logits, Schwellen-
  und Textentscheidungen können sich ändern; keine Toleranz-/Schwellenänderung
  zum Grünmachen.
  ([C++:941–942](../../vendor/whisper-rs-sys/whisper.cpp/src/whisper.cpp#L941),
  [Metal:5658–5695,5963–5970](../../vendor/whisper-rs-sys/whisper.cpp/ggml/src/ggml-metal/ggml-metal.metal#L5658)).
- Flash ist mit DTW-Tokenalignment inkompatibel; C++ schaltet DTW gegebenenfalls
  ab. Die feste Baseline hat DTW bereits aus. Ihre normalen Timestamp-Tokens
  (`no_timestamps=false`) sind etwas anderes und bleiben unverändert.
  ([Wrapper:463–467](../../vendor/whisper-rs/src/whisper_ctx.rs#L463),
  [C++:3708–3711](../../vendor/whisper-rs-sys/whisper.cpp/src/whisper.cpp#L3708)).

## M5: Quellseitig vorgesehen, konkreter Lauf noch unbelegt

Der Metal-Op-Support verlangt `has_simdgroup_mm`, gleiche K/V-Typen und einen
aufgeführten Head-Durchmesser, darunter64. `has_simdgroup_mm` wird mit
`supportsFamily:MTLGPUFamilyApple7` ermittelt. Der vorhandene gehashte Reference-
GGML-Header wurde ohne Initialisierung gelesen: Audio-/Text-State1280, jeweils
20 Heads, also64; Audiokontext1500. Der Metal-Shader enthält entsprechende
F16-64-Kernels. Diese Voraussetzungen passen zum vorgesehenen Modell.
Modellbindung: `ggml-large-v3-turbo-q5_0.bin`,574.041.195B,
SHA256 `394221709cd5ad1f40c46e6031ca61bce88931e6e088c188294c6d5a55ffa7e2`
wie im [P06-Runfreeze](../../tests/fixtures/corpus/p06-context-quality-run-v1.json).
([Support:1069–1092](../../vendor/whisper-rs-sys/whisper.cpp/ggml/src/ggml-metal/ggml-metal-device.m#L1069),
[Device:635–638](../../vendor/whisper-rs-sys/whisper.cpp/ggml/src/ggml-metal/ggml-metal-device.m#L635),
[Kerneltypen:5963–6066](../../vendor/whisper-rs-sys/whisper.cpp/ggml/src/ggml-metal/ggml-metal.metal#L5963),
[GGML-Headerreihenfolge:1509–1519](../../vendor/whisper-rs-sys/whisper.cpp/src/whisper.cpp#L1509)).

Apple nennt M5 als Apple10/Metal3&4 und SIMD-Matrixoperationen ab Apple7
([offizielle Feature-Tabelle,21.05.2026,S.2/4](https://developer.apple.com/metal/Metal-Feature-Set-Tables.pdf)).
Das begründet die Hardware-Erwartung, ersetzt aber keinen tatsächlichen
Capability-/Kernel-/Fehlernachweis am verwendeten M5/macOS/Build. GGML hat auch
CPU-Fallback; das gesetzte Flag allein beweist keine vollständig GPU-ausgeführte
Inferenz. Der getrennte M5/Metal4-Tensor-API-Zweig ist keine automatische
Flash-/Neural-Accelerator-Garantie.
([M5-Tensorwahl:647–665](../../vendor/whisper-rs-sys/whisper.cpp/ggml/src/ggml-metal/ggml-metal-device.m#L647),
[Scheduler:1077–1098,1177,1638](../../vendor/whisper-rs-sys/whisper.cpp/ggml/src/ggml-backend.cpp#L1077)).

## Erst ein eigener prospektiver Vergleich könnte qualifizieren

Plan§8/§13 (historischer oder lokaler Beleg, nicht mitgeliefert) erlaubt eingefrorene
Backendparameter, verbietet hier aber CoreML-/MLX-Wechsel. Vor einem Lauf:
genau diese Flagvariante, vollständige Kontext-/FullParams, Binary, Buildfeatures,
Quellen, Modelle,100 unveränderte Qualitätsfälle und neue Ausgabeziele einfrieren.
Neue gemeinsame gleichcode-/gleichwrapper-Reference verwenden, historische
Ausgaben nur als Drift-/Inhaltsanker; Variantenreihenfolge vorab wechseln,
Fehler und jeden Block erhalten. Die100er-Dateiprüfung bewertet ursprüngliche90
DE/EN×clean/challenging separat; zehn Zusatzrollen verändern deren Nenner nicht.
Delta höchstens0,5 Prozentpunkte je Sprache/Klasse, clean absolut höchstens10%,
keine neuen unzulässigen Anker-/Segment-/Empty-/Kurz-/A22-Fehler. Kein neues
Nullwort-/Nullnamensfehler- oder Naturmonologgate.

Erst bei bestandener Qualität neue gepaarte vollständige Echtzeitmessungen:
derselbe Ring/DSP/Runtime/TC-Publisher, genau ein Warmup+6×30 zentrale Läufe,
übrige feste Fälle je ein Warmup und mindestens zehn gemessene Wiederholungen;
L-Gewinn mindestens10% und alle
absoluten Grenzen. State-/Graph-/Prozesspeaks,100 Start/Stop/Cancel-Folgen und
Lang-Cancel/Neustart müssen für die Flagänderung passen bzw. begründet neu
nachgewiesen werden. Ältere `flash_attn=false`-Backendpeaks allein qualifizieren
`true` nicht. Keine solche Implementierung oder Messung ist hier erfolgt.

## Unveränderliche Quellen und Hashes

whisper-rs0.16.0/sys0.15.0 stammen aus dem gepinnten Wrappercommit
`7558e1b72f54f2f22a53589afb77e65681834c36`; dessen whisper.cpp-Pin ist
`2eeeba56e9edd762b4b38467bab96c2517163158`. Lokale SDK-Patches betreffen
SOT/VAD/Lifetime, keine Flash-Änderung; Herkunft und Unterschiede stehen in
[sys-PATCHES](../../vendor/whisper-rs-sys/PATCHES.md) und
[Wrapper-PATCHES](../../vendor/whisper-rs/PATCHES.md).
Folgende vollständige Dateien wurden gegen Git `6883035` gehasht; Zeilenlinks
oben gelten für diesen Stand, spätere Produktänderungen erfordern Nachprüfung.

| Datei | SHA256 |
|---|---|
| `src-tauri/src/transcription.rs` | `fb86b02866f29fbd21fcdc4ea99089fd03332c72d95eb533469c7eb3aff7eb77` |
| `vendor/whisper-rs/src/whisper_ctx.rs` | `1970ce436a6213df24718e53eb0c2c23962fe95d561c90642eaed733551d5b48` |
| `vendor/whisper-rs/src/whisper_ctx_wrapper.rs` | `6ff5fc0e0bed589e2f8f2bf4a99ee01faba84e58485a67545b6d49f24d10ec17` |
| `vendor/whisper-rs/src/whisper_state/mod.rs` | `3bb8bda10aabe8bb46ba0c1cb5da68128284dfe33b4ef7cc5d8654acee63bdd9` |
| `vendor/whisper-rs-sys/whisper.cpp/src/whisper.cpp` | `fc7f40c5d5022f45920164bfe8261de110d98a4fb88fd1ee97ef526ab00e682e` |
| `ggml/src/ggml-metal/ggml-metal-device.m`¹ | `568c08dab4df586ddb68cae95b36cf84e8fc80b34757a9d243ede4cb5f9d5280` |
| `ggml/src/ggml-metal/ggml-metal-ops.cpp`¹ | `5f40010bb2daac7a35dc89ee52766581ae206680be315cd6728523510c2535d1` |
| `ggml/src/ggml-metal/ggml-metal.metal`¹ | `fb95967219a2af1e7b7ab1cd938ed323a50ccd4df11da98758efee42200bf4ad` |
| `ggml/src/ggml-metal/ggml-metal.cpp`¹ | `a02d2675c0d5b8f3828fec9b74f4cd7e55d5b1bac70683a413e09881c57cab4e` |

¹ Relativ zu `vendor/whisper-rs-sys/whisper.cpp/`. Kein Produkt-/Planedit,
kein fremdes MLX-Dokument verändert.
