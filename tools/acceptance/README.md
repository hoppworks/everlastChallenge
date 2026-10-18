# Separater Adapter fuer native Textuebergabe

`transport-v3.patch` und `transport-v3.json` halten den auf dem Mac kompilierten,
unabhaengig geprueften Testadapter reproduzierbar fest. Der Adapter ist noch
nicht gestartet. Er ersetzt keine installierte Produktabnahme mit Mikrofon
und lokaler Inferenz.

Die Quelle ist exakt Commit
`d2546c438b8f5079f52b5415c181a0fe9129d98b` plus Patch, ausschliesslich in einem
separaten Checkout. Der Patch fuegt sieben Dateien hinzu und aendert drei
Integrationsstellen. Er gehoert nicht in den Produktbaum: Zwei Manifeste und
der native Modulexport sind dort gebundene Qualitaetsquellen. Fuer den Adapter
wird keine Uebertragung der alten 47-Quellen-Qualitaetsfreigabe behauptet.

## Reproduktion

1. Einen frischen separaten Checkout des genannten unveraenderlichen Commits
   anlegen. Keine Audios, Modelle oder Appdaten werden dafuer benoetigt.
2. Patchhash sowie alle Basisbindungen aus `transport-v3.json` pruefen;
   `git apply --check` und dann `git apply` mit der Patchdatei ausfuehren.
   Alle zehn Ergebnisdateien und sieben Supportbindungen danach exakt pruefen.
3. Im separaten Checkout mit gepinnter Toolchain und Lockfile bauen:

   ```sh
   cargo build --locked --offline -p everlast-dictation --features measurement --example transport_acceptance
   ```

Die tatsaechlichen Mac-Belege umfassen sechs Zustands-/Exit-Tests, einen
Wrong-Thread-Test vor jedem NSBundle-Zugriff, den Dev-Build, Strict Clippy
fuer Example in Dev-/Testprofil sowie Native-Library/Tests und die
Quellformatpruefung. Die erhaltenen initialen Compile-/Lintfehler wurden
behoben. Ein spaeterer Releasebuild braucht eine eigene Binarybindung.
Gewoehnliche leere Unified-Diff-Kontextzeilen enthalten ein fuehrendes
Leerzeichen. Ihre Bytes bleiben Teil des geprueften Patches; sie sind kein
nachtraeglicher Whitespacefehler der angewandten Rust-/TOML-Quellen.

## Noch offene native Ausfuehrung

Zunaechst ein frisches, separat benanntes Appbundle mit eigener Kennung
`de.hoppworks.everlast-transport-acceptance`, mitgelieferter Info.plist und
dem genau gebundenen Binary herstellen. Originale Lizenztexte und Notices aus
dem Basisstand unter `Contents/Resources/licenses` aufnehmen; tatsaechliche
Ressourcen, Signatur/CDHash, Quelle, Binary und feste Fixture neu binden.
Eine Tauri-Konfiguration ersetzt nicht die reale NSBundle-/PID-Pruefung.
Der ungebuendelte Binary scheitert absichtlich vor Aktivierung.

Erst eine zulaessige eigene Benutzeraktion startet den Adapter und vergibt
gegebenenfalls seine eigenen Bedienungshilfenrechte. Keine Produktrechte
werden auf diese Kennung uebertragen. Im Adapter einmal vorbereiten, selbst
das gewuenschte Textfeld fokussieren und physisch Control+Option+Shift+D
druecken. Genau die eingebettete 68-Byte-UTF-8-Fixture mit Hash
`78ce796361f50b77b1c86a693c9ba3bb02be2be8dc7fcd0e5ddfd4305da0c5b7`
darf ueber den bestehenden NativeService gesendet werden. Kein Freitext-IPC,
Modell, Mikrofon, Retry oder alternativer Einfuegeweg ist enthalten.

Der Einmal-Gate, eigene NativeService-Owner, Widerruf/Join und bestehende
Clipboard-/Fokus-/Rechte-/Layoutregeln bleiben wirksam. Eine Versandmeldung
bestaetigt keinen Empfang: Umlaute, Ziffern, Zeilenumbruch, genau eine
Einfuegung, Ziel/Fokus und Clipboardwiederherstellung muessen separat am
wirklichen Empfaenger geprueft werden. Der Adapter liest keine Feldinhalte.
Keine Shell-, Hilfsprogramm- oder UI-Umgehung frueherer Automationsablehnungen.

Installation, Appstart, Signatur, TCC, Hotkey, Empfaenger und der getrennte
Mikrofon-/Inferenzdurchlauf sind durch die vorhandenen Buildbelege noch
nicht abgenommen. Produktcode spaeterer Commits wird nicht stillschweigend
in diesen festgelegten Adapterstand uebernommen.
