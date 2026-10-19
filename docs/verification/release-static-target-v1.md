# Statischer Zielgraph-Abgleich für den nativen Testkandidaten

Am 19.10.2026 offline am Quellstand `2fc213c` ausgeführt, ohne Build,
Installation oder Eingriff in macOS-Berechtigungen. Produktquelle des
unveränderten Testbundles: `634d6c9`. Cargo.lock ist seit diesem Produktstand
unverändert. Dies ergänzt das konservative Lizenzinventar; es ersetzt keine
exakte finale statische Linkclosure oder juristische Prüfung.

## Tatsächlich geprüfter Umfang

`cargo tree --locked --offline -p everlast-dictation --target
aarch64-apple-darwin -e normal,no-proc-macro --prefix none --format '{p}|{f}'`
lieferte 219 eindeutige Pakete: drei Workspacepakete und 216 externe Pakete.
Derselbe Aufruf mit `-e normal` einschließlich Proc-Macros lieferte 289 Pakete:
drei Workspacepakete und 286 externe Pakete. Beide Befehle endeten erfolgreich.
Alle externen Name-/Versionspaare beider Graphen sind im aktuellen Inventar
enthalten. Die dortigen 311 Cargo-Komponenten sind konservativer als diese
aufgelöste Graphmenge; zusätzliche Inventareinträge werden nicht als tatsächlich
gelinkter Code ausgegeben. Build-/Dev-Abhängigkeiten sind nicht Gegenstand.

Der Fingerprint des tatsächlichen Release-Verzeichnisses
`artifacts/testversion-build-v1/target/aarch64-apple-darwin/release/`
(`everlast-dictation-f4ba12e092e123c6/bin-everlast-dictation.json`)
weist `features=["default"]` aus; `measurement` ist nicht aktiviert.
Fingerprint-SHA256:
`f6b2551b4ab1928cc4773c8b42c6f138ccdcb0394088a1c6ecc7e576e81b3c26`.
Ein älterer Fingerprint im Workspace-`target/` wurde ausdrücklich nicht als
Beleg für dieses Bundle verwendet.

`otool -L` am installierten Executable zeigt 25 dynamische Bibliotheken,
ausschließlich unter `/usr/lib/` oder `/System/Library/`. Das prüft dynamische
Suchpfade dieses Mach-O, nicht dessen vollständige statische Rust-/C++-Closure.
Executable-SHA256:
`45f4e463076c88e36adcc7ae95f810329686b32a69be374e0440d0ed7c0fd4ca`.

## Belege und Grenzen

Private Rohbelege liegen unter `artifacts/release-static-target-v1/`:

- `normal-runtime-tree.txt`, SHA256
  `c11cd89c45533ab0fac70f48a88d083f6bd55c1eaf52f946d5fd9c63b8c3dcae`.
- `normal-tree-with-macros.txt`, SHA256
  `c616225dcd76b253ea7cbbf9d9b04cb8e8c9f9767ee19b8f6b6ae74a2641a751`.
- Kanonische Auswertung `summary-v2.json`, SHA256
  `0b07b5714e36bb86d4ebcc012eeb14a600ab7be2bcd0eec3999a5a58d33fe0b7`.
- `installed-dynamic-libraries.txt`: tatsächliche `otool`-Ausgabe.

Die erhaltene `summary-initial.json` ist **ungültig**: Der erste Parser las
Featuretext hinter `|` als Teil der Version. V2 trennt das Feld vor dem
Name-/Versionsvergleich; die scheinbar fehlenden Einträge der ersten
Auswertung sind kein Lizenzbefund.

Der gesonderte [Laufzeithelfer-Nachweis](release-license-runtime-helper-v1.md)
belegt die hinzugefügten Vite-/Rolldown-Texte. Unbekannte Originaltexte von
realfft und finale Lizenz-/Releasefreigaben bleiben ausdrücklich offen.
