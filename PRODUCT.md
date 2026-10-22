# Product

<!-- impeccable:product-schema 1 -->

## Platform

Native macOS desktop app (Tauri 2, React/TypeScript UI)

## Users

Menschen, die auf macOS kurze Texte diktieren und in anderen Anwendungen verwenden.
Die Oberfläche dient der kompakten Bedienung der nativen Coding-Challenge-App.

## Product Purpose

Ein kleines Diktierwerkzeug, das nach dem Start in den Hintergrund treten kann.
Kompakte Bedienung und Progressive Disclosure begrenzen die sichtbare Komplexität.

## Operating Context

Die Oberfläche läuft produktiv in einer nativen Rust-/Tauri-2-App auf Apple Silicon
und macOS 27. Die WebView zeigt die React-/TypeScript-Oberfläche;
Mikrofonaufnahme, Inferenz und Einfügen laufen im nativen Prozess.

## Capabilities and Constraints

Rust besitzt Mikrofon, Stilleerkennung, lokale Transkription, globale Tastenkürzel
und Einfügen. Die freigegebene Palette ist nativ umgesetzt und geprüft. Browser-Vorschauen
bleiben Simulationen. Der tatsächliche Prüfstand und seine Grenzen stehen in
[Palette-Verifikation](docs/verification/palette-2026-10-21.md).

## Brand Commitments

Produktname: Everlast Coding Challenge. Sehr simple Hauptansicht; Einstellungen,
Modelldetails und Diagnose werden erst bei Bedarf offengelegt. Die native Palette verwendet
Hell-/Dunkelmodus nach Systemeinstellung und bestehender macOS-Systemschrift.
Die Testgrenzen stehen in den jeweils versionsgebundenen Prüfberichten.

## Evidence on Hand

README.md, docs/code-map.md, src/App.tsx, src/features/app/AppView.tsx und die am
21.10.2026 geprüfte installierte App. Keine erfundenen Leistungs- oder Erfolgsversprechen.

## Product Principles

- Diktieren ist die Hauptaufgabe.
- Details dürfen erst bei Bedarf erscheinen, Fehler müssen dennoch sichtbar sein.
- Oberfläche und native Funktionen behalten klare Zuständigkeiten.
- Die Gestaltung bleibt auf die Diktieraufgabe konzentriert.
