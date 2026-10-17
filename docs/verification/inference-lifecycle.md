# Echter Engine-Lauf: Abbruch, Neubelegung und Shutdown

Stand: 17.10.2026, Quellkennung `1244b6a+lifecycle-150c5df7`.
Dies ist ein Entwickler-Nachweis über die öffentliche InferenceService-Grenze,
keine vollständige P03-/P05-Produktabnahme.

## Verfahren vor Ausführung

Der Probe nimmt genau Modellpfad und autorisierte PCM16-Mono-16-kHz-WAV an.
Er verwendet das gepinnte Referenzmodell mit Deutsch und lädt den tatsächlichen
Engine einschließlich VAD. Eine direkte, unabgebrochene Verarbeitung derselben
Samples bildet die Wiederholungsbaseline. Sie ist kein unabhängiger Goldtext.

Ein Wrapper meldet den Eintritt des ersten Serviceauftrags und hält ihn an
**vor** dem tatsächlichen Engine-Aufruf. Bei gehaltener Barriere:

1. Aktiven Auftrag 1 abbrechen.
2. Auftrag 2 einreihen, dessen gehaltenes Journal beobachten, abbrechen und
   mittels Weak-Referenz sofortige Freigabe verlangen.
3. Auftrag 3 einreihen, während Journal 1 weiter gehalten wird und noch kein
   innerer Engine-Aufruf begonnen hat.
4. Barriere freigeben. Der alte Auftrag 1 verarbeitet absichtlich trotzdem sein
   Audio, liefert wegen ungültigem Token aber kein Ergebnis. Auftrag 2 darf den
   Decoder nie erreichen. Nur Auftrag 3 darf genau ein Ergebnis mit Session 3,
   Generation 71, Job 303 und unverändertem Samplebereich liefern; sein Text
   muss der direkten Baseline entsprechen. Beide Journale müssen freigegeben sein.
5. Service explizit joinen, bevor der zweite Engine geladen wird.

Ein zweites Szenario hält einen aktiven Wrapperauftrag und einen wartenden
Auftrag. Shutdown läuft auf einem beobachteten Cleanupthread. Es muss den
wartenden Puffer freigeben, den aktiven halten und während einer zusätzlichen
100-ms-Beobachtung unvollständig bleiben. Erst nach Freigabe darf der alte
Engine-Aufruf auslaufen und Shutdown den Worker joinen. Frühe Erfolgs- und
Fehlerantworten gelten als Fehler und bleiben für das abschließende Cleanup
erhalten; eigene modellfreie Tests prüfen beide Fälle.

Channel-/Polling-/Joinbeobachtungen sind begrenzt. Ein Timeout behauptet keinen
Hard-Abort: Ein weiterlaufendes Backend wird als nicht verifiziert gejoint
markiert. Das initiale Engine-Laden selbst ist synchron; der reale Prozesslauf
hatte zusätzlich eine äußere Frist von 300 s.

## Tatsächlicher V2-Lauf

```sh
cargo build --release --locked --example inference_lifecycle_probe
EVERLAST_BENCHMARK_REVISION=1244b6a+lifecycle-150c5df7 target/release/examples/inference_lifecycle_probe artifacts/models/ggml-large-v3-turbo-q5_0.bin artifacts/replay/neue-aufnahme.wav
```

Verwendet wurde die vom Nutzer freigegebene private Aufnahme mit 131.072 Frames.
Audio und Wortlaut bleiben außerhalb von Git. Die Ausgabe ist JSON ohne Text.
Alle folgenden Werte stammen aus dem tatsächlichen erfolgreichen Lauf, Exit 0:

| Beobachtung | Ergebnis |
|---|---|
| erster Modellload | 1.314 ms |
| Auftrag 2 vor Freigabe vollständig freigegeben | ja |
| Auftrag 1 vor Freigabe weiterhin gehalten | ja |
| Einreihen Auftrag 3 vor Freigabe | 32 µs |
| Wrapper-/innere Engine-Aufrufe im Neustartszenario | 2 / 2, zusätzlich direkte Baseline |
| einzige Ergebnisidentität | Session 3, Generation 71, Job 303 |
| Ergebnis genau einmal und gleich zur Baseline | ja |
| Neustartszenario anschließend gejoint | ja, Cleanup 4.411 µs |
| Shutdown vor Freigabe weiter blockiert | zusätzliche 100 ms beobachtet |
| innere Engine-Aufrufe im Shutdownszenario | 1 |
| Shutdown anschließend gejoint | ja, Gesamtbeobachtung 1.088.763 µs einschließlich Wartefenster |

Lokaler Bericht: `artifacts/replay/inference-lifecycle-v2.json`, SHA256
`e87303a44a91fc56cac431382dcb2421979bfe3a0108256be68d578a0e91889c`.
Probequelle SHA256
`4d2f2c51f1a38d2c115271458dd9f393db7c420c41d6f84ca9e705d271c8b644`,
Runnerquelle SHA256
`cbac95882d8bd953a260b3a73cdf8b73f110687f7f54c3b78151e8b74fa4e7be`.

Der erste lokale V1-Pilot bestand ebenfalls, prüfte das Nichtabschließen von
Shutdown vor Freigabe aber nicht ausdrücklich. Der unabhängige Review fand
an dieser Stelle ein mögliches Falschpositiv. V2 ergänzt diese Beobachtung und
Regressionen für frühes Ok/Err; V1 bleibt historisch und schließt dieses Tor nicht.
Neun modellfreie Probe-/Eingabetests, striktes Clippy, Gesamtcheck/-tests und
Secretprüfung bestehen. Es wurden keine Produktparameter oder Abhängigkeiten
geändert und kein App-Bundle ersetzt.

## Aussagegrenzen

Die künstliche Barriere liegt vor Engine::transcribe. Das ist keine Messung von
Preemption während eines bereits laufenden nativen Kernels. Der reale Engine
kann aufgrund Energie/VAD auch vor Whisper-Decodierung zurückkehren; die Probe
instrumentiert keinen nativen Kernelbeginn. Textgleichheit ist Wiederholbarkeit,
keine unabhängige WER oder Sprachqualifikation. 32 µs Einreihzeit ist weder eine
UI-Neustartlatenz noch Echtzeit-Stop-bis-Text-L. Die Beobachtung umfasst keine
100 nativen Aufnahmesitzungen, keinen Mikrofon-/Fokus-/Pastepfad, keine allgemeine
RSS-/Heapstabilität und keinen OS-Sleep. Für den Entwicklernachweis werden drei
unabhängige Journale plus WAV-Bytepuffer gehalten; daraus folgt kein höheres
zulässiges Journalbudget des Produktpfads. P03/P04/P05 bleiben offen.
