# Begrenzter Transfer der qualifizierten Referenzquellen

P05 bindet Produktquelle `b9799b6` und Archiventscheidung `3d4e6ab`.
Der selektive Vergleich bis `634d6c9` und zum neuen Kandidaten
`ee1a6a4f78fca3b45026a8de835e997c914b43ec` findet keine ungeklärte neue
Decoder-/Modelländerung im ausgeführten unsegmentierten Referencepfad.
[Maschinenlesbare Bindungen](reference-source-transfer-2026-10-19.json)
enthalten 16 Dateien, die historischen Revisionen, aktuelle Byte-/Hashwerte
und die Vendorbäume. Die ursprünglichen Messdaten werden nicht umetikettiert.

## Tatsächlich verglichene Grenzen

- `Engine::transcribe` bleibt exakt gleich. Der neue Loadhelper entspricht
  dem alten Body bei explizit eingesetztem `flash_attention=false`.
  Modell-/VAD-Definitionen und deren URLs bleiben exakt gleich.
- Greedy `best_of=1` und alle 14 bisherigen FullParams-Setter bleiben gleich.
  Reference delegiert mit `audio_ctx=0`; der neue Setter wird nur bei einem
  Wert ungleich null aufgerufen. `no_timestamps=false` bedeutet, dass native
  Timestamp-Tokens weiterhin aktiviert sind. Druckausgabe bleibt separat aus.
- Vorbereitung, SpeechGate und State/Text-Verarbeitung haben zusätzliche
  NoOp-Phasenränder und einen expliziten State-Drop. Eingabevalidierung,
  Speechrange, VAD-Scratch und Schwellen, Padding, frischer State pro Call,
  Segmentreihenfolge und 256-KiB-Grenze bleiben funktional gleich.
- DSP ist als ganze Datei geändert: `sealed_frames` ist neu. Der Reference-
  Pfad versiegelt keine Prefixe, daher bleibt dieser Wert null; für diese
  Invariante entsprechen Limit, Flush und Truncate den früheren Ausdrücken.
  `speech_range`, `push_interleaved` und Formatvalidierung sind exakt gleich.
- Die Service-/Workerübergabe ist geändert und besitzt eigene kontrollierte
  Regressionen. Reference lädt die normale Engine, verwendet keinen Prefixcache
  und keine private Context1400-Ableitung. Daraus wird keine neue Cold-/Hotkey-
  Zeitidentität abgeleitet. Der Copy/Clear-Fix ändert weder Eingabe noch Decode.
- Beide vollständigen Vendorbäume `whisper-rs` und `whisper-rs-sys` sind an
  allen verglichenen Revisionen identisch, ebenso Cargo.lock und Cargo-
  Zielkonfiguration. Bereits der frühere Rust-Full-Aufruf benutzte
  `whisper_full_with_state`. Keine Main-Branch-Änderung von FFI, C++ oder Metal-
  Feature wird aus privaten Diagnosederivaten abgeleitet.

Der Autorenaudit und ein unabhängiger zweiter Readback bestätigen diese
begrenzten Vergleiche; keine gesamte geänderte Datei wird als identisch
behauptet. Private Detailbelege: `artifacts/offline-continuation-v1/reference-transfer.*`.
Der neue JSON-Bericht ergänzt die inzwischen feste Commitbindung `ee1a6a4`;
der damalige 9a2a903-plus-WIP-Autorenbeleg bleibt unverändert erhalten.
