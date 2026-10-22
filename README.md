# Everlast Coding Challenge – lokale Diktier-App

Diktieren und den Text direkt in eine andere Anwendung einfügen. Die App läuft
nativ auf **macOS mit Apple Silicon**: Rust und Tauri 2 übernehmen Aufnahme,
Stilleerkennung, lokale Transkription und globale Tastenkürzel. Die kompakte
Oberfläche ist in React/TypeScript umgesetzt.

## Download für macOS

**[App herunterladen (.zip)](https://github.com/hoppworks/everlastChallenge/releases/latest/download/Everlast-Coding-Challenge-macOS-arm64.zip)** ·
[Release und Prüfsummen](https://github.com/hoppworks/everlastChallenge/releases/latest)

Benötigt **Apple Silicon und macOS 27 oder neuer**. ZIP entpacken und die App
nach Programme ziehen. Sie ist lokal signiert, aber nicht Apple-notarisiert;
macOS kann beim ersten Öffnen einen Sicherheitshinweis anzeigen.

## Loslegen

1. App öffnen und die Modellgewichte einmalig herunterladen. Danach erfolgt
   die Spracherkennung lokal und offline.
2. Mikrofonzugriff und für automatisches Einfügen die macOS-Bedienungshilfen
   freigeben.
3. Ein Textfeld in der Zielanwendung fokussieren, **⌥D** drücken und sprechen.
4. **Nochmals ⌥D:** stoppen, transkribieren und einfügen. **Escape:** abbrechen.

Bei verändertem oder nicht sicher prüfbarem Ziel bleibt der Text in der App
kopierbar. Der Aufnahmebutton erzeugt ein lokales Diktat ohne automatisches
Einfügen. Nach fünf Minuten endet die Aufnahme automatisch.

## Umsetzung und Grenzen

- **Lokaler MLX-Hybrid:** optimierter MLX-Encoder mit whisper.cpp-Decoder;
  ausschließlich dieser Modellpfad ist in der App verfügbar.
- **Native Audio- und Systemfunktionen:** keine Browser-Mikrofonaufnahme.
  Hell-/Dunkelmodus folgen macOS, Details erscheinen erst bei Bedarf.
- **Deutsch ist die geprüfte Produktsprache.** Erkennungsfehler bleiben möglich;
  andere Sprachen, Geräte und Zielanwendungen sind nicht umfassend qualifiziert.

Der [historische Voicely-Helfervergleich](docs/comparison.md) zeigt bei drei
kurzen Dateien 36,4–39,1 % kürzere Prozesszeiten. Das misst weder die vollständigen
Apps noch Aufnahme und Einfügen und ist kein Benchmark des aktuellen Releases.

## Weiterführende Dokumentation

| Thema | Einstieg |
|---|---|
| Architektur und Code | [Entscheidungen](docs/engineering-decisions.md) · [Code-Wegweiser](docs/code-map.md) |
| Selbst bauen und testen | [Entwicklung](docs/development.md) · [Werkzeuge](docs/tooling.md) |
| Bedienung und Fehlerfälle | [Bedienungsdetails](docs/usage.md) |
| Verifikation und KI-Nutzung | [Version 0.2.0](docs/verification/release-0.2.0.md) · [Softwareprüfungen](docs/verification/tooling-and-packages-2026-10-22.md) · [KI-Review](docs/ai-review.md) |
| Weitere technische Belege | [Dokumentationsindex](docs/README.md) |

Der Anwendungscode steht unter [MIT](LICENSE). [Drittanbieter-Lizenzen](docs/licenses/third-party/README.md),
[Abhängigkeiten](docs/dependencies.md) und die verbleibende
[realfft-Herkunftsgrenze](docs/verification/license-completion-2026-10-21.md)
sind separat dokumentiert.
