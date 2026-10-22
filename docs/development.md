# Entwickeln, bauen und testen

Diese Anleitung betrifft den **Build aus dem Quellcode**. Die fertige App aus
[dem Release](https://github.com/hoppworks/everlastChallenge/releases/latest)
benötigt keine Entwicklungswerkzeuge.

## Voraussetzungen

Geprüft ist Apple Silicon mit macOS 27.0.1, Xcode 27.1 (27A9275), separat
installierter Metal Toolchain, Node 22.23.1 / npm 10.9.8, Rust 1.99.1 und
CMake 4.4.4. Andere Hostkombinationen sind nicht als getestet ausgewiesen.
Unterstützt sind Node 22 ab 22.22.2, Node 24 ab 24.15 sowie Node ab 26;
CMake benötigt mindestens Version 3.25.

1. Xcode 27 installieren, einmal öffnen und die Erstinstallation abschließen.
   Bei mehreren Installationen unter Settings → Locations → Command Line Tools
   das passende Xcode auswählen.
2. Die Metal Toolchain über Xcode installieren oder
   `xcodebuild -downloadComponent MetalToolchain` ausführen.
   `xcrun --find metal` muss erfolgreich sein. Der MLX-Build kompiliert damit
   die `mlx.metallib`, die später im App-Paket enthalten ist.
3. Node.js mit npm, Git, rustup und CMake installieren. CMake muss im `PATH`
   stehen, bei vorhandenem Homebrew etwa über `brew install cmake`.
4. Im Checkout `rustup show` ausführen. `rust-toolchain.toml` stellt die gepinnte
   Toolchain samt rustfmt, Clippy, LLVM-Werkzeugen und Apple-Silicon-Target bereit.

## Starten

```sh
npm run doctor
npm run setup
npm run tauri -- dev
```

`doctor` prüft die Voraussetzungen ohne Installation oder Systemänderung.
`setup` lädt die gepinnten npm-/Cargo-Abhängigkeiten, installiert cargo-machete
0.9.2 und richtet den lokalen Commit-Hook ein. Dafür ist Internet erforderlich.
Der Modelldownload in der App ist davon getrennt. `npm run dev` startet nur die
Oberfläche; native Funktionen benötigen den Tauri-Prozess.

## Release bauen

```sh
npm run release
```

Das Ergebnis liegt unter
`target/aarch64-apple-darwin/release/bundle/macos/Everlast Coding Challenge.app`.
Der Releasehelper bindet die tatsächlich gebaute Metal-Bibliothek ein und prüft
Bundle und Mikrofondeklaration. Er installiert und veröffentlicht nichts.

Die Standardsignatur ist ad hoc, keine Apple-Notarisierung.
`npm run release:local` verwendet eine separat konfigurierte lokale
Entwicklungsidentität. Schlüssel und `.everlast-signing.json` gehören nicht ins
Repository. Signierte Apps mit erhaltenen Metadaten kopieren, etwa mit `ditto`.

## Prüfen

```sh
npm run check:fast
npm run test:portable
npm run check
npm run test:autonomous
```

`check:fast` prüft Typen, Lints, Formatierung, Architektur und JS/TS-Abhängigkeiten.
`check` ergänzt Rust-Clippy, generierte Verträge und Rust-Abhängigkeitshygiene.
Die [Testanleitung](autonomous-testing.md) erklärt die Profile und ihre Grenzen;
[Guardrails](guardrails.md) beschreiben die erzwungenen Regeln. Der
Entwickler-Steuerkanal bleibt im normalen Produktbuild deaktiviert.

Vorhandene Nachweise werden nur bei unveränderten Quell-, Modell- und
Parameterbindungen wiederverwendet. Dokumentation benötigt keinen neuen
Modelllauf. Zeitmessungen laufen ohne parallele Builds oder andere Benchmarks.
Ein lokaler Erfolg oder eine vorhandene Workflowdatei beweist keinen bestandenen
Remote-CI-Lauf.
