# P05: Zusatzkosten der Ganzaufnahme-VAD

Stand: 17.10.2026. Komponentenmessung auf dem dokumentierten Apple M5,
keine Ressourcenfreigabe der vollständigen App.

## Konstruierter Release-Workload

`vad_resource_probe` lädt den gepinnten VAD einmal warm auf CPU/einem Thread.
Zwei getrennte Prozesse erzeugen dasselbe künstliche periodische Signal:
einmal 3.200 Samples und einmal 4.800.000 Samples, jeweils Original plus getrennte
Peak-0,95-Kopie. Danach folgen 100 weitere 3.200-Sample-Calls auf demselben Kontext.
Keine Sprachreferenz, keine echte Aufnahme, kein ASR, kein Mikrofon und keine
Textübergabe. Der lange Workload ist ausdrücklich kein natürlicher Monolog.
Alle Scorelängen und Wertebereiche wurden nach jedem API-Erfolg validiert.

Befehle aus dem Repositoryroot:

```sh
cargo build --release --locked --example vad_resource_probe
EVERLAST_BENCHMARK_REVISION=97376d6+vad-resource-probe /usr/bin/time -l target/release/examples/vad_resource_probe artifacts/models/ggml-silero-v5.1.2.bin artifacts/corpus/vad-resource-short100.json
EVERLAST_BENCHMARK_REVISION=97376d6+vad-resource-probe /usr/bin/time -l target/release/examples/vad_resource_probe artifacts/models/ggml-silero-v5.1.2.bin artifacts/corpus/vad-resource-max300-short100.json max
```

Die tatsächlichen Läufe leiteten die time-Ausgabe in die unten gehashten
Dateien um. Ausgabepfade müssen neu sein; JSON wird mit 0600 angelegt. Probe-
und C++-Quellhash sind im JSON eingebettet, Modelle werden vor Verwendung
geprüft und aus einem privaten Snapshot initialisiert. Die aktive Release-
OUT_DIR-C++-Kopie hat denselben fc7f40c5…-Hash wie die integrierte Quelle.

| Workload | Initialisierung | Vorbereitung | erster VAD-Call | weitere 100 Calls | Original-/Scratchkapazität | Prozessmaximum RSS |
|---|---:|---:|---:|---:|---:|---:|
| 0,2 s + 100 kurze | 203.785 ms | 0.001 ms | 0.383 ms | 26.411 ms | je 12.800 B | 43335680 B |
| 300 s + 100 kurze | 187.950 ms | 1.239 ms | 347.896 ms | 26.607 ms | je 19.200.000 B | 81199104 B |

Das sind Einzelläufe, keine Latenzverteilung oder garantierten Worst-Case-Zeiten.
Der 300-s-Call liefert 9.375 Fenster; die kurzen Calls liefern jeweils sieben.
Das RSS stammt aus `/usr/bin/time -l` auf macOS und umfasst den ganzen
Prozess dieses Werkzeugs samt Bibliotheken, VAD und zwei PCM-Puffern. Es ist
weder isolierter Modellbedarf noch Speichermaximum von SpeechGate, ASR oder App.
Die 100 warmen Calls belegen Ausführbarkeit und wiederholte vollständige
Rückgaben; ein kontinuierlicher Heap-/RSS-Leaktest oder hundert native
Start/Stop/Cancel-Sitzungen wurden damit nicht ersetzt.

## Zusätzliche Puffer im Produktpfad

SpeechGate hält genau einen wiederverwendbaren Scratch-Vektor. Jeder Aufruf
leert die Länge und reserviert fallibel mit try_reserve_exact; die angeforderte
Kapazität ist durch 4.800.000 f32 begrenzt. Nach einem langen Diktat darf diese
Kapazität im warmen Worker erhalten bleiben; sie gehört ausdrücklich zum Budget.

| Bereits geprüfte bzw. ergänzte angeforderte Nutzkapazität | Bytes |
|---|---:|
| maximal zwei Audiojournale | 38.400.000 |
| größter unterstützter Ein-Sekunden-Ring | 1.536.000 |
| Reader-Transferpuffer | 65.536 |
| zusätzlicher VAD-Scratch | 19.200.000 |
| mögliche Kurzäußerungs-Paddingkopie | 64.000 |
| Summe dieser benannten Puffer | 59.265.536 |

Die ersten drei Werte stammen aus dem unveränderten geprüften Audioowner.
Die Summe ist ausdrücklich keine Obergrenze aller Allokationen: Resampler-
FFT-/Arbeitsbereiche, C++-Vektorkapazitäten, native Gewichte/Graphen, ASR-
States/Mel-/Backendpuffer, Allocator, CPAL und OS fehlen darin. Tatsächliche
Prozessmaxima sind für die vollständige Konfiguration separat zu messen.
Beim VAD-Laden kommen eine private 885.098-Byte-Modellkopie auf Disk und ein
65.536-Byte-Transferbuffer hinzu; es wird keine PCM-Datei geschrieben.
Der Snapshot wird nach Initialisierung sowie auf Fehlerpfaden per RAII entfernt.

Der synchrone VAD besitzt keinen Abbruchcallback. Er läuft ausschließlich im
seriellen Inferenzworker. Die bestehende Tokeninvalidierung bleibt wirksam;
höchstens ein alter vollständiger Workerauftrag darf auslaufen. Dieser Versuch
misst keine sichtbare Cancel- oder Startlatenz und ersetzt ihre Abnahme nicht.

Zum Zeitpunkt dieser Messungen war die im Quellenbericht benannte CString-
Verluststelle des Rust-VAD-Konstruktors unverändert. Am 17.10.2026 wurde sie
separat korrigiert und mit Original-RED/Patched-GREEN sowie echten nativen
Läufen geprüft; siehe [wrapper-lifetime-fix.md](wrapper-lifetime-fix.md).
Die historischen Messwerte oben bleiben unverändert. Die vollständige
Modell-Lifecycle-/Heap-Abnahme und P05 bleiben offen.

## Lokale Artefakte

| Datei | SHA256 |
|---|---|
| vad-resource-short100.json | `d81ed722380860ffd10d57fc1132375d142313c8e132fe2448eec0d7879c9ff8` |
| vad-resource-short100.time.txt | `2e56792a579b6d606570e259c883f5a72b5ce49a38d1b71e18b0291ad479b0df` |
| vad-resource-max300-short100.json | `3b3e8bae438d2787e00f195b39d7891986724989e8e9044c8b738c3f8080cd6f` |
| vad-resource-max300-short100.time.txt | `66e5e0b31534a6ddf7e9d9f473fbe82eea63e8130a13d99854aff9ef584a668f` |
