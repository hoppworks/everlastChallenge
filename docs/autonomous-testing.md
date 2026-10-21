# Automatisierte Prüfung

Die verfügbaren Befehle stehen in `package.json`. Voraussetzungen und Setup:
[README](../README.md#entwickeln-und-bauen).

## Schnelle Softwareprüfungen

```sh
npm run check:fast
npm run test:portable
npm run check:secrets
```

`check:fast` prüft TypeScript, Lints, Format, Modulgrenzen und Abhängigkeiten.
`test:portable` prüft Frontend- und Werkzeugverträge. Der Secret-Check umfasst
sowohl den aktuellen veröffentlichbaren Quellstand als auch die erreichbare Historie.

## Native Prüfung

```sh
npm run check
npm run test
npm run test:autonomous
```

Die nativen Befehle benötigen die im README beschriebene macOS-/Xcode-/Rust-Umgebung.
Die autonome Testsuite verwendet kontrollierte Audio-/Worker-/Empfängergrenzen;
sie ersetzt nicht automatisch ein reales Mikrofon, eine physische Tastaturaktion
oder den beobachteten Texteingang in einer fremden Anwendung.

Modelle und Messaudios gehören nicht automatisch zum Repository. Fehlende
Voraussetzungen sind im jeweiligen Bericht als solche zu dokumentieren.
Zeitmessungen laufen ohne parallele Compiler- oder Benchmarklast. Nachweise
sind nur für ihre benannte Quellrevision, Modellwahl und Messstrecke gültig.
