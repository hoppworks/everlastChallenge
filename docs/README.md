# Dokumentation

Der [Projektüberblick](../README.md) führt durch Installation und Bedienung.
Die folgenden Einstiege sind nach Leserinteresse geordnet; zum Ausprobieren der
App muss niemand die einzelnen Versuchsprotokolle durcharbeiten.

Dokumentation und Oberfläche sind auf Deutsch. Code, Kommentare, Dateinamen
und technische Bezeichner bleiben Englisch. Originalzitate, Befehle,
Lizenztexte und maschinenlesbare Schemafelder werden nicht übersetzt.

| Interesse | Einstieg |
|---|---|
| App installieren und ausprobieren | [Download, Voraussetzungen und Bedienung](../README.md#download-für-macos) |
| Umsetzung verstehen | [Technische Entscheidungen](engineering-decisions.md), [Code-Wegweiser](code-map.md) |
| Ergebnisse einschätzen | [Leistung](performance.md), [Vergleich mit Voicely und Testgrenzen](comparison.md) |
| Aktuellen Stand prüfen | [Native Prüfung und Paket 0.3.0](verification/release-0.3.0.md), [Lokale CI-Prüfung](verification/local-ci-2026-10-22.md), [Softwareprüfungen](verification/tooling-and-packages-2026-10-22.md), [Abhängigkeiten](dependencies.md) |
| Selbst weiterentwickeln | [Setup und Build](development.md), [Entwicklerwerkzeuge](tooling.md), [automatische Prüfungen](autonomous-testing.md), [Guardrails](guardrails.md) |
| Bedienungsdetails nachschlagen | [Aufnahme, Einstellungen und Mikrofonformate](usage.md) |
| Gestaltung nachvollziehen | [Produkt](../PRODUCT.md), [Designsystem](../DESIGN.md) |
| Herkunft und KI-Nutzung prüfen | [Lizenzen](licenses/third-party/README.md), [KI-gestützte Arbeit und Verifikation](ai-review.md) |

## Nachweise einordnen

Technische Prüfberichte gelten für die jeweils genannten Quellen,
Binärdateien, Modelle und Bedingungen. Aktuelle Leistungswerte stehen im
[Vergleich](comparison.md); funktionale Tests und Coverage belegen ihre
jeweiligen Verträge, keine allgemeine Erkennungs- oder Latenzgarantie.

Rohaufnahmen und lokale Transkripte werden nicht veröffentlicht. Hashberichte
binden lokale Belege, ersetzen aber nicht deren vollständige öffentliche
Reproduktion. Die Grenzen stehen bei den jeweiligen Ergebnissen.

Quellrevisionen bezeichnen den entsprechenden Laufzeit-Quellstand in der
veröffentlichten Historie. Prüfsummen der Messdaten bleiben eigenständige
Nachweise; ein Quellverweis allein reproduziert keine vergangene Laufzeit.
