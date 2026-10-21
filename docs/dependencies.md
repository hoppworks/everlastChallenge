# Abhängigkeiten und Herkunft

## ARM-SHA-256-Beschleunigung

`sha2` bleibt auf 0.10.9. Sein offizielles `asm`-Feature aktiviert auf Apple
Silicon den zur Laufzeit erkannten SHA-256-Hardwarepfad; ohne passende
CPU-Fähigkeit bleibt der Software-Fallback erhalten. Neu ist die durch
Cargo.lock gepinnte transitive MIT-Crate `sha2-asm` 0.6.4. Herkunft und
unveränderter Lizenztext stehen im
[Addendum](licenses/third-party/sha256-acceleration.json).
Dateigrößen, vollständige SHA-256-Prüfung und Fehler-/Cancel-Verhalten bleiben
erhalten. Es gibt keinen Cache, der die Integritätsprüfung überspringt.

## P01-Auswahl und Nachprüfung

- CPAL 0.15.3 für systemnahe Aufnahme, rtrb 0.3.5 für den SPSC-Ring, rubato 0.16.2 für zustandsbehaftetes gefiltertes Resampling außerhalb des Callbacks. Der zunächst aufgelöste rtrb-Stand 0.3.2 wurde nach dem fehlschlagenden Advisory-Gate vor Produktverwendung auf den korrigierten Stand angehoben: [RUSTSEC-2026-0274](https://rustsec.org/advisories/RUSTSEC-2026-0274.html). Keine Advisory-Ausnahme.
- whisper-rs 0.16.0 / whisper-rs-sys 0.15.0 mit Metal; jeweils Unlicense laut tatsächlich geladener Registry-Quelle. Gebündeltes whisper.cpp separat MIT. Lizenztexte werden für das Release-Bundle erhalten. Der fragliche Abort-Callback wird nicht verwendet.
- reqwest 0.12.28 mit rustls; Herausgeber-TLS-Vertrauen aus webpki-roots 1.0.10. Dessen CDLA-Permissive-2.0 erlaubt Weitergabe mit beigefügtem Lizenztext; deshalb eine auf genau dieses Paket/diese Version begrenzte Policy-Ausnahme statt allgemeiner Freigabe unbekannter Lizenzen.
- ts-rs 11.1.0 als geplante Quelle generierter TypeScript-Verträge; serde-kompatible Serialisierung muss am ersten tatsächlichen DTO nachgewiesen werden.
- TypeScript zunächst 7.0.2 aufgelöst; dessen npm-Paket enthält keine verwendbare Compiler-API. Für dieselbe Compiler-/Syntaxbaumgrundlage der Importprüfung stattdessen 5.9.3 festgeschrieben und Typecheck/Build/Regel-Fixtures erneut geprüft. Dies ist eine bewusste Kompatibilitätsentscheidung.

Vollständige Lizenzsammlung aller tatsächlich ausgelieferten transitiven Bibliotheken, Modelle und Binärbestandteile bleibt ein Release-Gate. Diese Notiz behauptet noch kein vollständiges Lizenzverzeichnis.

## Lokaler Hotkey-Patch

`global-hotkey` bleibt auf 0.8.0 unter `tauri-plugin-global-shortcut` 2.4.1. Die
versionierte Original-Crate in `vendor/global-hotkey` wird über `[patch.crates-io]`
verwendet; MIT-, Apache- und SPDX-Lizenzen liegen bei. Herausgeberarchiv:
SHA-256 `8c386b0a4a70cb2d39fffd74480f985b6f0bfbcb934b6a6b6b7e630e448f242e`,
VCS-Stand `2a620bf3852008b568f6d36c2baedcc3dd0822f2`. Der enge macOS-Patch erhält
numerische Carbon-OSStatus-Fehler und entfernt einen Registrierungs-Handle erst nach
erfolgreichem Unregister. Geänderte Dateien, Herkunft und der kontrollierte
Fehler-/Retry-Test stehen in [PATCHES.md](../vendor/global-hotkey/PATCHES.md).
Der Test beweist Ressourcenbesitz bei simuliertem Fehler; Zeichenunterdrückung,
Secure Input und echte Betriebssystemfehler benötigen weiterhin native Abnahme.

## Lokale TLS-Prüffixture

`rustls` 0.23.46 war bereits transitiv über reqwest aufgelöst und wird zusätzlich
als exakt gebundene normale Abhängigkeit für die typisierte TLS-Fehlerklassifikation verwendet; zusätzlich nutzt der lokale TLS-Testserver dieselbe Version.
Dadurch kommt keine neue Paketversion hinzu. Zertifikat und Schlüssel unter
`src-tauri/tests/fixtures/tls/` sind eigens erzeugte öffentliche Testdaten. Nur
ein expliziter Testclient vertraut diesem Zertifikat; Systemvertrauen und
Produktclient bleiben unverändert. Die Fixture ist kein Bestandteil des
App-Bundles.

## Lokale No-Speech-Korrektur und VAD

`whisper-rs-sys` 0.15.0 ist jetzt als vollständige unveränderte Archivbasis mit
eng begrenzten lokalen Korrekturen gepinnt: initiale SOT-Logit-Zeile und
VAD-Fehlerstatus. Herkunft, Original-/Patchhashes und Lizenztexte stehen in
[vendor/whisper-rs-sys/PATCHES.md](../vendor/whisper-rs-sys/PATCHES.md).

## Lokaler Rust-VAD-Wrapperfix

Auch `whisper-rs` 0.16.0 wird als vollständige gepinnte Paketbasis lokal
eingebunden. Die einzige Laufzeitänderung behebt einen verlorenen CString-
Modellpfad im VAD-Konstruktor, auf Erfolgs- und Fehlerpfad. Herkunft und
Unlicense bleiben erhalten: [PATCHES.md](../vendor/whisper-rs/PATCHES.md).
Original-RED/Patched-GREEN, Cargo-Auflösung und echte native Smoke-Läufe sind
in [wrapper-lifetime-fix.md](verification/wrapper-lifetime-fix.md) dokumentiert.

## Vorbereitete Release-Lizenzsammlung (b34914c)

Die [Komponentenzuordnung](licenses/third-party/README.md) und ihr
[hashgebundenes Inventar](licenses/third-party/components.json) ergänzen die
unveränderten Originaltexte der produktiven JS-Pakete und konservativ
eingeschlossener nativer Abhängigkeiten. Die tatsächliche finale arm64-
Linkclosure und das Release-Bundle sind damit noch nicht nachgeprüft. Die
Original-Lizenz-/Copyrightdatei von realfft3.5.0 bleibt als Quellenbefund
unbekannt; ein deutlich bezeichneter standardisierter MIT-Permissiontext
ergänzt die deklarierte Lizenz, ohne Inhaber/Jahr zu erfinden oder eine
juristische Freigabe-/Sperrentscheidung abzuleiten.

[Modellbereitstellung](licenses/MODELS.md) trennt OpenAI-Gewichte, ggml-Code
und Silero. Die Bundlekonfiguration nimmt diese Lizenzdateien unter licenses/
auf, enthält aber keine Gewichte. Herkunft und tatsächliche lokale Änderungen
der drei Vendorpakete bleiben in ihren bisherigen PATCHES.md erhalten.

## Ergänzung für den nativen Testkandidaten (19.10.2026)

Der [Laufzeithelfer-Nachweis](verification/release-license-runtime-helper-v1.md)
ergänzt Vite 8.3.4 und dessen tatsächlichen Rolldown-Provider 1.2.14: Der
gebündelte Modulepreload-Helfer stammt trotz der Entwicklungsabhängigkeit aus
diesen Werkzeugen. Das Inventar enthält nun 317 Komponenten. Der
[statische Zielgraph-Abgleich](verification/release-static-target-v1.md)
prüft dessen Abdeckung für den aufgelösten macOS-arm64-Graphen; er beweist
keine exakte finale statische Linkclosure und keine juristische Freigabe.

## Aktueller Rust-/MLX-Hybrid

Dieser Abschnitt beschreibt die nativen Bestandteile des freigegebenen Hybrids.
Rust 1.99.1 ist lokal gepinnt. Der Inferenzpfad benötigt keinen Python-, Torch- oder Lightning-
Unterbau, enthält aber weiterhin native C-/C++-/Metal-Komponenten.

Verwendet werden das veröffentlichte `mlxrs 0.2.0` und `mlxrs-sys 0.2.0` aus
[dem festgelegten Herausgeberstand](https://github.com/findit-studio/mlxrs/tree/38744eb0dc1b91750991b12de0e24b1d0e475a6f).
Die Rust-Crate ist ein Communityport; daraus folgt keine Zusage der vollständigen
Whisper-Funktionsparität oder der Pflege durch Apple/OpenAI. Die ursprünglichen
Archive, Registrychecksummen, Quellstände und lokalen Änderungen bleiben in den
Vendorreceipts erhalten. `mlx-rs` und `mlxrs` sind unterschiedliche Crates.

| Risiko                   | Tatsächlicher Befund und Begrenzung                                                                                                                                                                                                                                                                                                                                                                                                        |
| ------------------------ | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| Numerische Qualität      | Die eigenständigen MLX-Decoder erzeugten neue kritische Inhaltsfehler und sind ausgeschieden. Ein historischer Qualifikationskandidat nutzte einen F32-MLX-Encoder mit aus Q5 abgeleiteten Koeffizienten. Dessen 124 geprüfte Ausgaben/Status waren zur jeweiligen Referenz identisch; das ist kein universeller Gleichheitsbeweis. Ein Update benötigt neue gepaarte Belege. |
| Thread-/FFI-Besitz       | MLX-Arrays und Streams sind threadgebunden. Initialisierung, Nutzung, Synchronisierung und Freigabe erfolgen im selben permanenten Worker; nur Factory und PCM-/Textnachrichten wechseln Threads. Native asynchrone Kernelfehler können laut Herausgeber den Prozess beenden. Rust-Result und Cancel versprechen dafür keine harte Unterbrechung.                                                                                          |
| Lokale Patchpflege       | Der MLX-Port hat eng begrenzte Änderungen an Wiedergabeabhängigkeiten, periodischem Hann-Fenster, optional gelieferten Encoderpositionen und EOS-Absicherung. Der sys-Patch pinnt native Buildinputs. Zusätzlich existiert die additive Encoder-Bridge im bereits venderten whisper.cpp-/Rust-Wrapper. Herkunft und Hashes sind erhalten; Aktualisieren erfordert einen bewussten Port dieser Änderungen und erneute Tests.                |
| Native Buildinputs       | JSON-/Metal-C++-Archive sind mit SHA-256 und fmt mit vollständigem Commit gebunden. `config/mlx-build-inputs.json` und der sys-Receipt dokumentieren die tatsächlich verwendeten Eingaben. Xcode 27.1 und dessen Apple-Metal-Toolchain-Baustein gehören zur geprüften Umgebung. Eine gepinnte Cargo-Version allein hätte diese Ebene nicht reproduzierbar gemacht.                                                                         |
| Tokenizer-Abhängigkeit   | Der lokale Tokenizers-Patch ersetzt nur den unmaintained Paste-Makronamen durch das kompatible, gepinnte Pastey-Paket. Die Tokenizerlogik wird damit nicht umgeschrieben. Advisorygrenzen wurden nicht gelockert.                                                                                                                                                                                                                          |
| Modell-/Speicherbedarf   | Der aktuelle FP16-Modellsatz umfasst 1.617.856.158 Byte, darunter 1.613.977.612 Byte Encodergewichte. Der optionale historische Koeffizientenpfad umfasst 3.813.494.037 Byte inklusive 3.235.574.296 Byte F32-Gewichten. Zwei ASR-Modelle und zwei VAD-Kontexte bleiben geladen; je Auftrag wird ein VAD verwendet. Die native Speicher-/Ladezeitabnahme ist von der Textqualität getrennt. Bisher wurde nur der M5 mit 32 GiB geprüft.                                                                                                   |
| Paketierung              | Die 183.778.048 Byte große MLX-Metallib muss aus dem aktiven Cargo-OUT_DIR neben das Executable gelangen. Metallib-Signaturen liegen in xattrs; gewöhnliches Kopieren verlor sie im tatsächlichen Fehlversuch. Der Releasehelper kopiert zuerst, signiert die kopierte Bibliothek und dann die App und prüft das Ergebnis strikt. Spätere Relokation muss xattrs erhalten.                                                                 |
| Lieferkette und Lizenzen | Cargo-/npm-Prüfungen sowie Quell-/Archiv-/Assethashes sind konkrete Gates, keine vollständige Sicherheits- oder Rechtsfreigabe. Die MLX-Inventare erfassen konservativ den aufgelösten Graphen; sie sind keine exakt nachgewiesene finale Linkclosure. Noch nicht vollständig rekonstruierte native Lizenz-/Attributionstexte bleiben explizit markiert. Eine Veröffentlichung ist nicht Teil dieses Auftrags.                             |

Die sichere lokale Modellkonvertierung, Hashprüfung, atomare Veröffentlichung,
Abbruchbehandlung und getrennten Cacheverzeichnisse sind tatsächlich geprüft.
Nur der optionale Koeffizientenpfad verwendet abgeleitete F32-Gewichte ohne Downloadendpunkt; die Einrichtung
erzeugt sie aus der zuvor vollständig verifizierten Q5-Eingabe. Ein Abbruch
entfernt ausschließlich die eigene Teildatei und erhält ein vorhandenes
verifiziertes Ziel.

Belege: [native Buildpins](../config/mlx-build-inputs.json),
[MLX-Patchreceipt](../vendor/mlxrs/EVERLAST-PATCH.json),
[sys-Patchreceipt](../vendor/mlxrs-sys/EVERLAST-PATCH.json),
[Encoder-Bridge](../vendor/whisper-rs-sys/PATCHES.md),
[zusätzliches Lizenzinventar](licenses/third-party/mlx-components.json) und
[native Herkunfts-/Attributionsgrenzen](licenses/third-party/mlx-native-provenance.json).
Die unabhängigen, begrenzten Reviews und die tatsächlichen RED/GREEN-/Build-/
Korpusbelege liegen unter `artifacts/implementation/` beziehungsweise
`artifacts/mlx-qualification/`. Die aktuellen Qualitäts-, Zeit- und Ressourcenbelege des F16-/Cache256-Kandidaten
sind in der README verknüpft; sie ersetzen keine universelle Sicherheits- oder
Rechtsfreigabe.
