# Runtime- und Inferenzbesitzer: Lebensdauerprüfung

Stand: 17.10.2026. Ausgangsrevision `1244b6a`.

## Aktuelle echte Initialisierung und geordnetes Ende

Das Entwicklerwerkzeug `application_probe` wurde mit dem aktuellen lokalen
Wrapper und VAD-Gate gebaut:

```sh
cargo build --release --locked --example application_probe
target/release/examples/application_probe artifacts/models
target/release/examples/application_probe artifacts/models --fail-native
```

Beide tatsächlich ausgeführten Läufe endeten mit Exit 0. Der erste durchlief
Missing → Checking → Loading → Ready, veröffentlichte Runtime Idle und joint
Modell- sowie native Besitzer beim Ende. Der zweite erhielt den absichtlich
injizierten Native-Factory-Fehler nach Modellinitialisierung und räumte die
bereits initialisierte Runtime samt Modellworker auf. Die tatsächlichen
Prozessläufe waren extern auf je 120 s begrenzt.

Diese Werkzeuge fordern weder Mikrofonaufnahme noch Tastaturereignisse oder
Clipboardänderung an. Sie bauen kein App-Bundle. Sie beweisen den vorhandenen
normalen und fehlerhaften Initialisierungs-/Endpfad, keine native Aufnahmesitzung,
keinen OS-Sleep und keinen Controller-Panic während aktiver Besitzer.

## Reproduzierter und korrigierter Panic-Pfad

Im Runtime-Controller wird das explizite Audio-/Inferenz-Shutdown nach der
Actor-Schleife bei Rust-Unwinding übersprungen. Der RuntimeExit-Guard schließt
zwar Admission und beantwortet wartende Requests; die Service-Drops fordern
jedoch nur Stop an und warten auf keinen Besitzer. Runtime::join kann dadurch
einen Workerfehler melden, bevor die untergeordneten Besitzer beendet sind.
Der bestehende Panic-Test prüfte nur Admission/Acks. Drei neue Regressionen
halten einen Audioowner und einen echten InferenceService-Worker mit
kontrolliertem Decoder unabhängig blockiert. Sie verlangen geschlossene
Admission und beantwortete Requests vor Freigabe der Besitzer; der Runtime-Join
darf bis zu beiden Freigaben nicht zurückkehren.

```sh
cargo test --offline --locked -p everlast-dictation --lib runtime::tests::controller_panic --no-default-features
```

Der ursprüngliche Runtime-Quellstand aus `1244b6a` (SHA256
`4b4612d06281e94a0281d2f234fd39cfcdb4585cceb586e2cf2ae1a05de52434`)
mit den neuen Tests ergibt **0 bestanden / 3 fehlgeschlagen**, jeweils
„Runtime::join returned while both owners were blocked“. Nach Wiederherstellen
des Fixes ergibt derselbe Befehl **3 bestanden / 0 fehlgeschlagen**.
Die gesamte Runtime-Suite besteht mit 19 Tests; striktes Library-/Test-Clippy
besteht ebenfalls.

Der Controllerowner fängt Rust-Unwinding um die Actor-Schleife ab und behält den
Actor für das explizite Cleanup. Es gibt keine Wiederaufnahme eines möglicherweise
teilweise veränderten Controllers. Vor den Joins werden Ausgaben widerrufen,
Admission geschlossen und wartende Requests beantwortet. Audio- und
Inferenzcleanup werden jeweils separat ausgeführt und gegen Rust-Unwinding
abgegrenzt. Auch ein Audio-Cleanupfehler oder eine injizierte Audio-Cleanup-Panic
überspringt den Inferenzjoin nicht. Der ursprüngliche Controller-Workerfehler
bleibt dabei der zurückgegebene Fehler.

Die beiden zusätzlichen Fehlerfälle erreichen nachweislich den Fehler- bzw.
Panic-Zweig nach dem Audiojoin und warten danach weiterhin auf Inferenz-Drop.
Damit ist keine Garantie für beliebigen fremden Code gegeben, der vor eigener
Ressourcenfreigabe panikt, und kein harter Abbruch nativer Backendaufrufe
implementiert. Die Regression prüft kontrollierte Hostbesitzer, keine echte
Mikrofonaufnahme, OS-Sleep oder Panic durch eine FFI-Grenze. Der GUI-Thread joint
weiterhin keinen dieser Worker.

## Integrierter Prüfstand

Runtime-Quelldatei des geprüften Kandidaten, SHA256:
`150c5df7f3847bd316c60b8d445ddebac84aaf186661063221c74945434f0998`.
`npm run check`, `npm test` und Secretprüfung bestehen am integrierten Stand.
Der Release-Entwicklerbuild von application_probe wurde danach nochmals gegen
diesen Kandidaten geprüft: normaler Ablauf und absichtlich fehlgeschlagener
nativer Start enden erneut mit Exit 0 und gejointen Besitzern. Das ist kein
Release-App-Bundle und keine nachträgliche native Panic-Abnahme.
