# P03: automatisches Ende und wiederholter Neustart

Stand: 17.10.2026, Ausgangsrevision `5f7497b`. Dieses Paket ergänzt fehlende
Hostvertragsnachweise, ohne Produktverhalten, Parameter oder Abhängigkeiten zu
ändern. Die echten Runtime-Actor-, Controller- und InferenceService-Pfade
laufen mit kontrollierten Audio-/Decoder-/NativeEndpoint-Grenzen. Es gibt keine
Mikrofonaufnahme, GUI-Steuerung oder tatsächliche Texteingabe.

## Automatisches Aufnahmeende

Der Test startet Session 1 über den öffentlichen Client mit Zielerfassung und
bestätigtem Audiostart. MaximumWarning muss im Recording-Snapshot ankommen;
bis dahin darf weder Inferenz noch Stop ausgelöst sein. Anschließend meldet
der kontrollierte Audioowner Finished mit maximum=true und bekanntem Audio,
ohne vorherigen Client::stop. Ein zweites Finished mit Streamfehler folgt.

Erwartet werden genau ein Decoderaufruf, der unveränderte bekannte Audiobereich,
genau ein Deliver mit dem erwarteten Text und exakt demselben gemeinsamen
DeliveryToken wie bei der Zielerfassung. Ein später Benutzer-Stop darf dies
nicht verändern. Nach bestätigter Lieferung bleiben Text und Sent-Ergebnis
korrekt; ein bestätigter weiterer Controllerauftrag überquert die Verarbeitung
der späten Ereignisse. Es gibt keinen zweiten Decoderaufruf, Stop oder Deliver.
Der abschließende Runtime-Join bestätigt Audio-Shutdown und geschlossenes Ende.

Dieser Test verwendet **800 Samples**, nicht 300 Sekunden. Das Maximaldauer-Fact
wird kontrolliert eingespeist. Er belegt die bisher ungetestete Hostverkabelung,
keine Uhr-, Samplegrenz-, CPAL- oder echte Fünf-Minuten-Abnahme. Die vorhandenen
RecordingLimits-/Ringtests prüfen diese jeweils gesonderten kontrollierten Grenzen.

## Abbruch–Neustart–Abbruch–Neustart

Session 1 wird aufgenommen und finalisiert. Ihr echter serieller
InferenceService-Worker erreicht den kontrollierten Decoder und bleibt dort
an einer begrenzten Barriere blockiert. Cancel 1 widerruft den Liefer-Token.
Session 2 kann anschließend aufnehmen, obwohl dieser alte Worker noch läuft.

Cancel 2 widerruft ebenfalls sofort den Liefer-Token. Während die Audiofreigabe
von Session 2 noch fehlt, muss ein weiterer Start mit bestätigter Ablehnung
enden. Ein verspätetes Finished von Session 1 darf Session 2 nicht freigeben.
Eine Poll-/Request-Ack-Barriere beweist die tatsächliche Verarbeitung dieses
veralteten Facts. Erst Finished 2 erlaubt Session 3.

Session 3 nimmt auf und finalisiert, während Decoder 1 weiterhin nachweislich
blockiert bleibt. Nach Freigabe dürfen exakt die Audiobereiche 1 und 3 den
Decoder erreichen, niemals Audio 2. Ausgegeben werden darf ausschließlich Text 3
mit Session 3 und exakt demselben gemeinsamen Token wie bei Capture 3.
Ein verspätetes Delivered 1 wird über zwei tatsächliche Poll-Barrieren verarbeitet;
Session 3 bleibt dabei Delivering mit unverändertem Text, bis ihre gültige
Lieferantwort eintrifft. Nach Runtime-Join sind Audio-Shutdown und Decoder-Drop
bestätigt und alle beobachteten Ereigniskanäle ohne zusätzliche Arbeit.

Der unabhängige Review beanstandete die zunächst bloße Gleichheit der
Sitzungsnummern beim Tokenvergleich. Die endgültige Assertion verwendet
DeliveryToken::same_work und prüft damit auch die gemeinsame Tokeninstanz.
Der gezielte Lauf besteht mit dieser verschärften Assertion.

## Ausgeführte Prüfung und Quellstand

```sh
cargo test --offline --locked -p everlast-dictation --lib runtime::tests::maximum_contract --no-default-features
cargo test --offline --locked -p everlast-dictation --lib runtime::tests::restart_contract --no-default-features
cargo test --offline --locked -p everlast-dictation --lib runtime::tests --no-default-features
```

Beide neuen Einzeltests bestehen; die finale Runtime-Suite besteht mit
**21 Tests**. Die Testmodule sind gewöhnliche externe Rust-Module unter
runtime/tests; cargo fmt folgt ihnen automatisch. Gesamtcheck, gesamter
Testlauf und Secretprüfung wurden zusätzlich ausgeführt. Es wurde kein
künstlicher RED oder ein behobener Produktfehler behauptet: Dies sind zuvor
fehlende Vertragsprüfungen vorhandenen Verhaltens.

| Quelldatei | SHA256 |
|---|---|
| src-tauri/src/runtime.rs | `bcccfc3f5138811e2e3f15759ccd5a2251e93ef84107b5d323d147a6f895ac3e` |
| src-tauri/src/runtime/tests/maximum_contract.rs | `64e693f65fd8b4a2edb5cf0618ebdbe5d057d32e51636528ef2ba1604aa9a40f` |
| src-tauri/src/runtime/tests/restart_contract.rs | `f4b06af8f5086dfa33a3d1b6b0ab13edbedd6a8cd72061f10991c1b5a40c51a6` |

## Verbleibende Grenzen

Die Runtime-Seam übergibt Vec-Audio; ihre interne Vec→Arc-Umwandlung ist hier
nicht als Weak-Referenz beobachtbar. Dieser Test beweist deshalb keine allgemeine
Journalfreigabe, Speicherobergrenze oder Heap-/RSS-Stabilität. Die gesonderte
[Engine-Lifecycleprobe](inference-lifecycle.md) belegt direkte Service-Slotfreigabe;
sie wird nicht zu einem Runtime-Speichernachweis umbenannt. Decoder 1 ist ein
kontrollierter Rust-Decoder, kein nativer Whisper-Kernel. Reale 100
Aufnahmesitzungen, Mikrofon-/Bluetoothwechsel, OS-Sleep sowie das installierte
App-Ende sind durch diesen isolierten Test nicht belegt.
