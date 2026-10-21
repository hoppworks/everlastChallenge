# Product

<!-- impeccable:product-schema 1 -->

## Platform

web

## Users

## Product Purpose

Ein kleines Diktierwerkzeug, das nach dem Start in den Hintergrund treten kann.
Der Nutzer hat am 21.10.2026 kompakte Bedienung und Progressive Disclosure bestätigt.

## Operating Context

Die Oberfläche läuft produktiv in einer nativen Rust-/Tauri-2-App auf Apple Silicon
und macOS 27. `web` bezeichnet hier die React-/TypeScript-Oberfläche und ihre
Browser-Entwürfe; es ist keine Freigabe für eine browserbasierte Audio-Pipeline.

## Capabilities and Constraints

Rust besitzt Mikrofon, Stilleerkennung, lokale Transkription, globale Tastenkürzel
und Einfügen. Die native Übernahme der Palette ist freigegeben und wird umgesetzt.
Browser-Vorschauen bleiben Simulationen; native Aufnahme, Build und Integration
müssen am Implementierungskandidaten gesondert verifiziert werden.

## Brand Commitments

Produktname: Everlast Coding Challenge. Sehr simple Hauptansicht; Einstellungen,
Modelldetails und Diagnose werden erst bei Bedarf offengelegt. Palette ist die vom Nutzer freigegebene Richtung für die native Umsetzung, mit
Hell-/Dunkelmodus nach Systemeinstellung und bestehender macOS-Systemschrift.
Diese Freigabe ersetzt keine technische Abnahme des Implementierungskandidaten.

## Evidence on Hand

README.md, docs/code-map.md, src/App.tsx, src/features/app/AppView.tsx und die am
21.10.2026 geprüfte installierte App. Keine erfundenen Leistungs- oder Erfolgsversprechen.

## Product Principles

- Diktieren ist die Hauptaufgabe.
- Details dürfen erst bei Bedarf erscheinen, Fehler müssen dennoch sichtbar sein.
- Oberfläche und native Funktionen behalten klare Zuständigkeiten.
- Varianten werden vor der Umsetzung in die produktive App verglichen.
