# Paketstand 0.2.0 und Werkzeugmigration

Die eigenen aktiven Werkzeuge wurden von Python auf TypeScript umgestellt.
Historische P06-Kampagnenstarter und ihre ausschließlich zugehörigen Adaptertests
bleiben über die Git-Historie nachvollziehbar. Die vorhandenen Messdateien und
Parameter wurden nicht als Ergebnisse des neuen Paketstands ausgegeben.
[Werkzeuge und aktuelle Kommandos](../tooling.md) beschreiben die Grenzen.

## Verhalten und Schnittstellen

- Die Oberfläche bietet ausschließlich den MLX-Hybrid an. Bekannte ältere
  Modellwahlen werden beim Start umgestellt; Mikrofon, Sprache und physisches
  Tastenkürzel bleiben erhalten. Ungültige oder fremde Einstellungsformate
  werden weiterhin sichtbar zurückgewiesen.
- CPAL 0.18.2 verwendet seine neue Gerätebeschreibung, Fehlertype und
  Sampleraten-Schnittstelle. Die deterministische Wahl eines angebotenen,
  unterstützten Mikrofonformats bleibt erhalten.
- Rubato 5.0.1 arbeitet weiterhin mit fester Eingangsblockgröße, zwei Teilblöcken
  und dem bisherigen Blackman-Harris-Fenster. Die Signaltests prüfen Dauer,
  Durchlassbereich, Aliasunterdrückung, Impulse, Blockübergänge und Flush.
- reqwest 0.13.5 benötigt bei `rustls-no-provider` eine ausdrückliche
  Providerinitialisierung. Der Client installiert Ring, falls noch kein
  Prozessprovider existiert. Ein vorhandener oder gleichzeitig installierter
  Provider wird erhalten. HTTPS-, Redirect-, Größen- und Integritätsprüfungen
  bleiben aktiv. Die normale Verbindung nutzt den Plattform-Zertifikatsprüfer;
  ausschließlich die positive lokale TLS-Testkontrolle verwendet einen eigenen
  Trust Store mit dem veröffentlichten Testzertifikat.
- TypeScript 7 prüft App und Werkzeuge; die ESLint-/AST-Integration verwendet
  die separat gepinnte TypeScript-6-API. Direkte Compiler-Dateifixtures benutzen
  ausdrücklich `--ignoreConfig`, die Projektprüfungen ihre `tsconfig`-Dateien.

Direkte Versionen und transitive Auflösung stehen in den Manifesten und Lockfiles.
`generic-array` 0.14.7 bleibt wegen des exakten Pins in `crypto-common` 0.1.7 im
Tauri-Buildgraphen; 0.14.9 lässt sich innerhalb dieses Graphen nicht auswählen.
Die eigenen direkten Abhängigkeiten wurden aktualisiert. Die lokal gepatchten
Vendorquellen blieben unverändert. [Inventar und Originaltexte](../licenses/third-party/updated-dependencies-2026-10-22.json)
bleiben von einer behaupteten exakten Linkclosure getrennt.

## Ausgeführte Softwareprüfungen

| Prüfung | Ergebnis |
|---|---|
| Strikte Typprüfung, ESLint/Oxlint, Architektur, Format, Knip | bestanden |
| Cargo-Format, Clippy über Workspace/alle Targets, generierte IPC-Verträge | bestanden |
| Kern-/Audio-/Sitzungsverträge | bestanden |
| Native Bibliothek und lokale Adapterverträge | bestanden; einschließlich Clipboard-Grenzen und Cleanup |
| Hostverträge | bestanden; drei explizit modell-/fixtureabhängige Tests bleiben aus dem Standardlauf ausgeschlossen |
| TLS-Transport | 9 Tests bestanden, einschließlich Ablehnung untrusted TLS, positiver isolierter Trust-Kontrolle, Abbruch, Timeout und unverändertem Altmodell |
| React und IPC | 227 Tests bestanden |
| Bestehende Node-Werkzeuge | 158 bestanden, eine ausdrücklich optionale native Helper-Probe nicht aktiviert |
| Migrierte Werkzeuge | 36 Tests bestanden; zusätzlich gefrorene Unicode-, PCM-/Auswahl- und Mel-Referenzen |
| Instrumentierte Kernabdeckung | 96,94 % der erfassten Zeilen |
| Frontendabdeckung | 98,29 % Zeilen, 95,54 % Zweige |
| Host-Abdeckung | alle bestehenden modulspezifischen Untergrenzen eingehalten |
| Negative Guardrail-Fixtures, Secret-Quellscan, Cargo-Advisories/Lizenzen/Quellen, npm Audit | bestanden |

Die Abdeckungswerte gelten für den in `config/coverage.json` benannten Umfang;
bei den Hostdateien werden auch Inline-Tests erfasst. Sie beweisen keine globale
Produktabdeckung. Die Softwareprüfungen wurden zusammenhängend ausgeführt.
Nach konkreten Fehlern wurden die betroffenen Schritte fortgesetzt: insbesondere
Rustls-Initialisierung, die isolierte TLS-Testkontrolle, Compiler-Dateifixtures
und die gemeinsam genutzte Hashformatierung. Spezielle Dateitypen werden vor
dem Lesen abgelehnt; ein FIFO-Test bestätigt, dass dabei nicht auf einen
Schreiber gewartet wird. Die Schwellen wurden nicht gesenkt.

Die neue Unicode-Auswertung stimmt mit den eingefrorenen Ausgaben des früheren
Unicode-13-Auswerters überein. Alle drei unabhängigen Mel-DFT-Fälle stimmen
inklusive Float32-Eingaben und Ergebnisarrays exakt mit den bisherigen Goldens
überein. Keiner dieser Tests ist eine neue Mikrofon-, Hotkey- oder
Leistungsabnahme. Die [historischen Vergleichszahlen](../comparison.md) gelten
weiterhin nur für ihre dort genannten Binärdateien und Messbedingungen.
