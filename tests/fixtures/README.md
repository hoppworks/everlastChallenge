# Öffentliche Inferenz-Smoke-Fixture

`jfk.wav` stammt unverändert aus dem offiziellen whisper.cpp-Repository, Tag v1.7.6: [Originaldatei](https://github.com/ggml-org/whisper.cpp/blob/v1.7.6/samples/jfk.wav). Der zu diesem Quellstand gehörende unveränderte MIT-Lizenztext liegt in LICENSE.whisper-cpp.txt. Die Aufnahme ist das vom Upstream selbst verwendete öffentliche englische JFK-Beispiel; sie enthält keine privaten Nutzerdiktate.

SHA-256: `59dfb9a4acb36fe2a2affc14bacbee2920ff435cb13cc314a08c13f66ba7860e`.

Die explizite Entwicklerprobe prüft die bekannte Phrase „ask not what your country can do for you“ in zwei aufeinanderfolgenden frischen Whisper-States. Sie protokolliert nur Erfolg, Textbytezahl und Dauer. Das ist ein Funktions-/State-Smoke-Test, kein Abnahmekorpus und keine WER-, Sprachqualitäts- oder Echtzeit-Latenzabnahme. Der verbindliche deutsche/englische Korpus aus P05 bleibt separat aufzubauen, zu lizenzieren und vor Vergleichen einzufrieren.

## Entwickler-Datei-Replay

Eine Aufnahme kann einmal als Datei bereitgestellt und anschließend wiederholt durch den produktiven `InferenceService` verarbeitet werden. Jeder Auftrag verwendet einen frischen Whisper-State. Die Probe nimmt kein Mikrofon auf, spielt kein Audio ab und sendet keine Einfügebefehle. Ausgabe: Lade-/Verarbeitungszeiten, Textbytezahl und gegebenenfalls Referenz-Matchstatus; Transkripte und Referenzinhalte werden nicht ausgegeben oder gespeichert.

Das bisherige Zwei-Argument-Kommando bleibt unverändert: Englisch, zwei Durchläufe und die bekannte JFK-Phrasenprüfung.

```sh
cargo run -p everlast-dictation --release --example transcribe_probe -- MODEL_PATH tests/fixtures/jfk.wav
```

Für andere lokale Dateien ist die Sprache ausdrücklich anzugeben. Unterstützt werden `de` und `en`; Wiederholungen liegen zwischen 1 und 30, Standard ist 2. Es wird weiterhin ausschließlich das hashgeprüfte Referenzmodell `large-v3-turbo-q5_0` geladen.

```sh
cargo run -p everlast-dictation --release --example transcribe_probe -- MODEL_PATH artifacts/replay/input.wav --language de --repeat 3
cargo run -p everlast-dictation --release --example transcribe_probe -- MODEL_PATH artifacts/replay/input.wav --language de --repeat 3 --reference artifacts/replay/reference.txt
```

Die Eingabe muss eine reguläre WAV-Datei mit PCM16, einem Kanal und 16.000 Hz sein: mindestens ein Sample, höchstens 300 Sekunden, Dateigröße höchstens 9.665.536 Bytes einschließlich Header/Metadaten. Die Probe konvertiert keine anderen Formate. Eine Referenz muss eine reguläre, nicht leere UTF-8-Textdatei bis 256 KiB sein. Der Vergleich erhält Groß-/Kleinschreibung und Satzzeichen; allein Folgen von Whitespace werden gleichgesetzt. Ein Referenzfehler beendet die Probe mit einem Fehlerstatus. Das ist kein WER-Verfahren.

Private Audio-/Referenzdateien ausschließlich unter dem bereits ignorierten `artifacts/` ablegen; sie gehören nicht in Git. Die Probe lädt nur die ausdrücklich angegebenen Dateien und hat keine Produkt-IPC oder Renderer-Schnittstelle.

**Datei-Replay belegt Inferenz und wiederholte Worker-Aufträge. Es ersetzt weder die reale Mikrofonabnahme noch Hotkey-/TextEdit-End-to-End oder das zeitgesteuerte Echtzeit-Messprotokoll.** Auch ein Release-Example ist kein installierter Bundle-Nachweis und keine SLA-Messung.
