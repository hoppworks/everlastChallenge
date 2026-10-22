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

## Historische Versuche lesen

Die datierten Dateien unter `verification/` sind technische Versuchsprotokolle.
Sie bewahren auch Fehler, verworfene Varianten und frühere Oberflächen. Ihre
Ergebnisse gelten ausschließlich für die jeweils genannten Quellen,
Binärdateien, Modelle und Messbedingungen. Ein damaliges „offen“ ist kein
aktueller Statusbericht; ein damaliges „bestanden“ ist keine pauschale Aussage
über spätere Versionen.

`P01` bis `P06` und `A…` sind historische Kennungen von Versuchen und
Prüfkriterien. Sie sind keine Fortschrittsanzeige des aktuellen Produkts.
Die aktuelle Oberfläche ist die kompakte React-/TypeScript-Palette in Tauri;
frühere AppKit-Oberflächen sind in ihren datierten Berichten dokumentiert.
Eigene aktive Werkzeuge verwenden TypeScript. Frühere Python-Kommandos in
Versuchsprotokollen gehören zu deren ursprünglicher Git-Revision; die heutigen
Kommandos stehen in [tooling.md](tooling.md).

Rohaufnahmen und lokale Transkripte werden nicht veröffentlicht. Wo nur
inhaltfreie Hashberichte vorliegen, ist deren Nachweisgrenze im jeweiligen
Bericht benannt. Die veröffentlichten Vergleichszahlen dürfen deshalb nur mit
den zugehörigen Einschränkungen übernommen werden.
