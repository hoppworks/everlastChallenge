# Silero-VAD als mögliches Ganzaufnahme-Gate

Stand: 2026-10-17. Quellenprüfung zu `whisper-rs 0.16.0` /
`whisper-rs-sys 0.15.0`, gebündeltes whisper.cpp
`2eeeba56e9edd762b4b38467bab96c2517163158`. Vorbereitende Recherche mit
unabhängigem lokalem Rechercheagenten; **kein VAD-/ASR-Lauf, kein Produkt- oder
Cargo-Code geändert**. Ein einziges offizielles Modell wurde unter dem
10-MB-Budget geladen und kryptografisch geprüft.

## Entscheidungsvorlage

Die vorhandene API kann grundsätzlich vor der unveränderten Volltranskription
die Frage prüfen, ob **irgendwo** in der Aufnahme ein Sprachsignal erkannt wird.
`WhisperVadContext::detect_speech(&mut self, &[f32])` liefert dabei **nur
Ausführungserfolg**, keine Ja-/Nein-Sprachentscheidung. Danach stehen über
`probabilities()` frameweise Sprachwahrscheinlichkeiten bereit. Eine spätere
Gate-Regel könnte diese über die gesamte Aufnahme aggregieren. Schwelle und
Aggregation sind noch nicht qualifiziert; diese Recherche setzt sie nicht fest.
[Versionierte Rust-API](https://docs.rs/whisper-rs/0.16.0/whisper_rs/struct.WhisperVadContext.html),
[Wrapperquelle](https://docs.rs/whisper-rs/0.16.0/src/whisper_rs/whisper_vad.rs.html).

Wenn das Gate Sprache zulässt, erhält Whisper weiterhin die vollständige bisher
vorgesehene Aufnahme. Keine VAD-Schnitte, kein Entfernen innerer Pausen und keine
Änderung von Aufnahmeende oder Textnachbearbeitung. Die `full()`-Option für
integriertes VAD und die Segment-/Concatenation-APIs sind dafür nicht nötig.
Kein Speech-Nachweis im ersten Fenster darf spätere Sprache übergehen.

Der Auftrag meldet nach dem korrigierten No-Speech-SDK verbleibende unerwünschte
Ausgaben auf negativen Kalibrierungsfällen. Das ist ein konkreter Anlass, VAD
zu untersuchen; **Nutzen, Kurzsprachenerhalt, Kosten und Fehlerpfade sind noch
nicht gemessen**. Nach Plan §7 und den DSP-Kriterien ist das keine Freigabe zur
Produktintegration oder zur Anpassung der eingefrorenen Acceptance-Daten.

## Quelltextbefund: State, Zeitraster und Signal

Zeilenangaben hier beziehen sich auf die unveränderte gepinnte
[offizielle C++-Quelle](https://github.com/ggml-org/whisper.cpp/blob/2eeeba56e9edd762b4b38467bab96c2517163158/src/whisper.cpp).
Der lokale SOT-No-Speech-Patch ändert diesen VAD-Abschnitt nicht.

- **Reset pro Call:** `whisper_vad_detect_speech` löscht vor jedem Durchlauf
  den LSTM-Hidden-/Cell-State-Puffer (5099–5100). Die Gewichte und der
  Compute-Scheduler bleiben im Kontext. Ein serialisiert verwendeter Kontext
  kann daher mehrere unabhängige Jobs verarbeiten, ohne LSTM-Historie vom
  vorigen Job zu übernehmen. Innerhalb eines Calls bleibt der rekurrente State
  über die Frames erhalten. Separates `detect_speech` für jeden 32-ms-Block
  würde diesen State ständig zurücksetzen und wäre kein gleichwertiger Weg.
- **Ganzaufnahme:** Schleife über `ceil(n_samples / n_window)`; das geladene
  Modell definiert `n_window=512`. Bei 16 kHz ist ein Frame **32 ms**.
  300 s entsprechen **9.375 Frames**. Die Wahrscheinlichkeitsliste enthält
  genau einen Wert pro Frame; kein 30-s-Encoderfenster wie bei Whisper.
- **Letztes Teilfenster:** Ein Rest wird nicht verworfen, sondern auf ein
  vollständiges Fenster mit Nullen aufgefüllt (5125–5142). Sein Wert zählt zur
  Diagnose, seine zeitliche Reichweite endet aber bei der echten Audiodauer.
  Kurze Sprache ausschließlich am Aufnahmeende ist explizit zu testen.
- **0,2-s-Sprache:** 3.200 Samples ergeben sieben Modellschritte, davon sechs
  vollständige und einen mit 128 echten Samples plus Padding. Es gibt in
  `detect_speech` keinen 250-ms-Mindestdauerfilter. Die nachgelagerte
  Segment-API hat dagegen standardmäßig `min_speech_duration_ms=250` und
  verwirft kürzere erkannte Segmente. Dieser Default ist keine geeignete
  Regel für den geforderten Kurzsprachenerhalt. Auch sieben verfügbare
  Wahrscheinlichkeiten beweisen noch nicht zuverlässige Erkennung von 0,2 s.
- **Eingang:** Das Modell ist ausdrücklich `silero-16k`. Vorher vorhandenes
  Mono-16-kHz-PCM als endliche f32 im üblichen normierten PCM-Bereich verwenden;
  kein PCM16-Integermaßstab, kein ungeprüftes NaN/Inf, kein überlanger Cast nach
  C-`int`. Die Projektgrenze 4.800.000 Samples für 300 s liegt darin sicher.
  Der C++-Pfad kopiert die Samplewerte unverändert in den Frame-Tensor; keine
  automatische Peak-/RMS-Normalisierung oder AGC findet statt (5144–5146).
  Eine neue Normalisierung wäre eine zusätzliche Variable und müsste getrennt
  kalibriert werden; sie ist kein notwendiger Teil dieses Vorschlags.
- **Kein Sampling-Rate-Parameter:** Diese API resampelt nicht und kann eine
  falsch angelieferte Rate nicht aus den Daten erkennen. Upstream-Silero kann
  auch 8 kHz, der GGML-Konverter entfernt aber diesen Modellzweig.

[Reset und Framedurchlauf](https://github.com/ggml-org/whisper.cpp/blob/2eeeba56e9edd762b4b38467bab96c2517163158/src/whisper.cpp#L5086-L5166),
[Segment-Defaults](https://github.com/ggml-org/whisper.cpp/blob/2eeeba56e9edd762b4b38467bab96c2517163158/src/whisper.cpp#L4436-L4447),
[Konverter: 16-kHz-Zweig, Fenster und Kontext](https://github.com/ggml-org/whisper.cpp/blob/2eeeba56e9edd762b4b38467bab96c2517163158/models/convert-silero-vad-to-ggml.py#L12-L51).

Die Datei enthält zusätzlich `n_context=64`. Der lokale C++-Framedurchlauf
verwendet diesen Wert nicht für einen expliziten 64-Sample-Audioüberhang zwischen
Fenstern; er erhält den LSTM-State und verwendet reflektierendes STFT-Padding.
Bytegleiche Gewichte allein beweisen daher keine identischen Scores zur
Python-/ONNX-Pipeline. Der konkrete GGML-Pfad ist selbst zu kalibrieren.

## Konkrete Fehlerpfadgrenze vor einer Produktnutzung

**Nachgewiesene API-Lücke:** Bei Fehler von `ggml_graph_compute_helper` im
Frame-Loop protokolliert C++ den Fehler, führt `break` aus und liefert am Ende
dennoch `true` (5149–5165). `probs.resize(n_chunks)` hat die vollständige Länge
bereits vorher eingestellt. Nicht berechnete Plätze können Nullwerte oder bei
wiederverwendetem Kontext Werte eines früheren Jobs enthalten. Rust erhält
dann `Ok(())`; Länge, Endlichkeit und Wertebereich allein erkennen dies nicht.
Das könnte Sprache fälschlich als Stille behandeln oder eine fehlerhafte
Aufnahme aufgrund alter positiver Werte zulassen.

Ein späteres belastbares Gate muss diese Grenze vorher schließen oder einen
anderen belegbar vollständigen Ausführungspfad nutzen: etwa ein minimaler
lokaler Vendorfix, der nach Scheduler-Cleanup bei Computefehler **false**
zurückgibt, samt Fault-Injection-Regression. Ein Backendfehler darf nicht als
„keine Sprache“ gelten. Diese Recherche ändert den Vendor nicht.
[Exakter Fehlerpfad](https://github.com/ggml-org/whisper.cpp/blob/2eeeba56e9edd762b4b38467bab96c2517163158/src/whisper.cpp#L5149-L5165).

Weitere Integrationsgrenzen:

- Nichtleeren Input prüfen und `probabilities()` erst nach erfolgreicher
  Verarbeitung verwenden. Der Rustwrapper bildet einen Slice aus einem rohen
  Zeiger; vor erster Verarbeitung oder für leeren Input ist die Rückgabe nicht
  als robust dokumentierte leere Liste abgesichert.
- Methoden auf genau einem Worker serialisieren; nicht parallel auf denselben
  nativen Kontext zugreifen. Geliehene Wahrscheinlichkeiten gehören zum Kontext
  und werden beim nächsten Call überschrieben.
- Es gibt in dieser API keinen Abortcallback pro Job oder Frame. Ein 300-s-Call
  führt alle 9.375 Schritte synchron aus. Starts/GUI dürfen das nicht ausführen;
  Cancel-Latenz muss einschließlich VAD gemessen werden. Aufteilen in mehrere
  Calls als Abbruchtrick verändert wegen des Resets die Semantik.
- Die Zeitvariable `t_vad_us` akkumuliert über den Kontext; die native Logzeile
  ist deshalb bei Wiederverwendung keine isolierte Dauer des aktuellen Jobs.
  Eigene monotone Wallclock pro Call messen.
- `WhisperVadContext::new` verwendet im Wrapper `CString::into_raw()` ohne
  nachfolgende Rückgabe an `CString::from_raw()`. Wiederholtes Neuerzeugen
  verliert damit zumindest den Pfadstring. Ein wiederverwendeter Kontext
  vermeidet diese wiederkehrende Kleinallokation; das ist kein Grund, native
  Zustands-/Gewichtsallokationen pro Job als kostenlos anzunehmen.

## Gepinntes offizielles Modell und Lizenz

Gewählt wurde **`ggml-silero-v5.1.2.bin`**, weil das Downloadskript am
vorhandenen C++-Pin genau diese Version und die offizielle Quelle nennt.
Keine Modellkonvertierung oder Ausführung erfolgte lokal.

| Eigenschaft | Festgehaltener Wert |
|---|---|
| Herausgeber-Repo | `ggml-org/whisper-vad`, öffentlich und ohne Gating |
| HF-Revision | `9ffd54a1e1ee413ddf265af9913beaf518d1639b` |
| Datei | `ggml-silero-v5.1.2.bin` |
| Bytes | **885.098** |
| LFS-/lokaler SHA256 | **`29940d98d42b91fbd05ce489f3ecf7c72f0a42f027e4875919a28fb4c04ea2cf`** |
| Ignorierter lokaler Pfad | `artifacts/models/ggml-silero-v5.1.2.bin` |
| Lokaler Dateimodus | `0600` |
| Nicht ausführend gelesener Header | `silero-16k`, Version `5.1.2`, Fenster `512`, Kontext `64` |
| Upstream-Silero-Tag | `v5.1.2` → `6478567951ae5c9979ad7b234185b5515f4be7a1` |

[Offizielle gepinnte Datei](https://huggingface.co/ggml-org/whisper-vad/resolve/9ffd54a1e1ee413ddf265af9913beaf518d1639b/ggml-silero-v5.1.2.bin),
[gepinntes HF-Dateiverzeichnis mit LFS-Metadaten](https://huggingface.co/api/models/ggml-org/whisper-vad/tree/9ffd54a1e1ee413ddf265af9913beaf518d1639b?recursive=false&expand=false),
[offizielles Downloadskript](https://github.com/ggml-org/whisper.cpp/blob/2eeeba56e9edd762b4b38467bab96c2517163158/models/download-vad-model.sh#L6),
[Versionsliste im Skript](https://github.com/ggml-org/whisper.cpp/blob/2eeeba56e9edd762b4b38467bab96c2517163158/models/download-vad-model.sh#L33).

HF deklariert MIT. Das ursprüngliche Silero-Projekt enthält am gepinnten
v5.1.2-Stand eine **MIT-Lizenz**; bei einer späteren Weitergabe sind die dortigen
Copyright-/Lizenzhinweise vollständig beizulegen, zusätzlich zur Lizenz des
GGML-Konverter-/Runtimecodes. Der Konverter lädt `load_silero_vad()`, schreibt
den Versionsheader und konvertiert Convolutiongewichte nach FP16. Die Quellen
belegen den vorgesehenen Ursprung; eine vollständige Buildattestation oder eine
unabhängig nachgebaute Byteidentität des HF-Binaries wurde nicht gefunden.
[Original-Silero-Lizenz](https://github.com/snakers4/silero-vad/blob/6478567951ae5c9979ad7b234185b5515f4be7a1/LICENSE),
[Original-Projektversion](https://github.com/snakers4/silero-vad/blob/6478567951ae5c9979ad7b234185b5515f4be7a1/pyproject.toml#L6),
[Konverter](https://github.com/ggml-org/whisper.cpp/blob/2eeeba56e9edd762b4b38467bab96c2517163158/models/convert-silero-vad-to-ggml.py).

Das öffentliche Binary wurde einmal über den obigen **revisionsgebundenen**
Resolve-Link geladen: HTTP 200, begrenztes Lesen mit 10.000.000-Byte-Abbruch,
exakte Länge und SHA vor atomarer 0600-Veröffentlichung geprüft. Anschließend
wurde die finale Datei erneut gegen denselben Pin gelesen. Nur **885.098 Bytes**
Modellinhalt wurden heruntergeladen; kein zweites Modell, kein Login/Upload.
Reproduzierbare Prüfung der vorhandenen Datei:

```sh
shasum -a 256 artifacts/models/ggml-silero-v5.1.2.bin
wc -c < artifacts/models/ggml-silero-v5.1.2.bin
```

## CPU, RAM und noch nötige Nutzenmessung

Der lokale C++-VAD-Pfad erzwingt derzeit **CPU**, auch wenn `use_gpu` gesetzt
wäre (4665–4671); Rusts Standard ist ebenfalls CPU und vier Threads.
Gewichte können warm geladen bleiben. Der Dateiumfang von rund 0,89 MB ist
**keine Gesamtaussage über RAM**. Hinzu kommen Backend-Gewichte/Alignment,
Graphmetadaten, Compute-Scheduler, Arbeitsbuffer und die Zustände. Reine
Nutzdaten: zwei 128-f32-LSTM-Zustände = 1.024 Bytes; maximal 9.375 f32-Scores
= 37.500 Bytes; Framebuffer 512 f32 = 2.048 Bytes, beim letzten Teilfenster
nochmals ein solcher Puffer. Das ist keine Obergrenze für alle Allokationen.
[Kontext und Scheduler](https://github.com/ggml-org/whisper.cpp/blob/2eeeba56e9edd762b4b38467bab96c2517163158/src/whisper.cpp#L4662-L4715).

Upstream nennt für seine eigene Implementierung weniger als 1 ms pro
30+-ms-Chunk auf einem CPU-Thread. Dies ist **kein Benchmark dieses
GGML-/Apple-Silicon-Pfads** und keine RTF-/Cancelzusage.
[Silero-Leistungsbeschreibung](https://github.com/snakers4/silero-vad/blob/be95df9152c0d7618fa1edfeb296fc3dae32376f/README.md#L101).

Vor Produktentscheidung separat messen: Modellinitialisierung und warmer Call,
Peak-/residenter Speicher zusätzlich zu Whisper, 300-s-CPU-Zeit, Cancel-/Shutdown-
Latenz sowie Wiederholung ohne Wachstum. Nutzen auf unveränderten positiven und
negativen **Kalibrierungsfällen** vergleichen, einschließlich 0,2 s leiser Sprache,
Sprache nur im letzten Teilfenster, langer Stille vor später Sprache und
natürlichem Raumgeräusch. A22 und sprachliche Anker dürfen nicht verloren gehen.
Schwellen vor einer späteren Acceptance-Auswertung einfrieren. Ob das Gate die
verbleibenden Fehläußerungen zuverlässig beseitigt, bleibt vollständig offen.
