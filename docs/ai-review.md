# KI-Nutzung und technische Prüfung

KI-Werkzeuge wurden für Rust-/TypeScript-Implementierung, Testentwurf,
Fehlerdiagnose, Dokumentation, Designvarianten und Code-Reviews eingesetzt.
Die Änderungen wurden mit automatisierten Checks und gezielten nativen Tests geprüft.

## Beispiele

- [Technische Korrekturen](verification/release-fixes-2026-10-21.md): Fehlerbefunde,
  Regressionstests und überprüfte Laufzeitverträge.
- [Palette](verification/palette-2026-10-21.md): simulierte Browserzustände,
  native Datenanbindung und Bedienungstests sind getrennt beschrieben.
- [Entscheidungen](engineering-decisions.md): Begründungen und verworfene Alternativen.
- [Messungen](performance.md): revisionsgebundene Ergebnisse mit Grenzen.

Automatisierte Tests decken die geprüften Verträge ab, nicht jede Mikrofon-/App-
Kombination. Agentenreviews bleiben fehlbar. Historische Ergebnisse gelten nur
für ihre angegebenen Quellen, Modelle und Messbedingungen.

## Paket- und Werkzeugmigration 0.2.0

Codex unterstützte die Bibliotheksanpassungen und TypeScript-Migration. Ein
unabhängiges kombiniertes Code-Review prüfte Anforderungen, Fehlerpfade und
Projektregeln. Dabei wurden ein zweiter ungebundener Dateilesezugriff, zu
weiche Feldtypprüfung, unvollständige Fall-ID-Prüfung und fehlende Eingabehashes
korrigiert. Die TLS-Initialisierung wurde zusätzlich anhand der fehlschlagenden
Transporttests berichtigt. Gezielte Regressionen, der gemeinsame Softwarelauf
und die tatsächlich erreichten Abdeckungsgrenzen stehen im
[technischen Prüfbericht](verification/tooling-and-packages-2026-10-22.md).
Das Review ersetzt diese ausführbaren Nachweise nicht.

Der [native Prüfbericht 0.2.0](verification/release-0.2.0.md) ergänzt die
beobachtete Einstellungsübernahme, Aufnahme, Modellproben und Paketprüfung.
Er grenzt die fehlende neue Hotkey-/Empfangsprüfung ausdrücklich ab.

Beim lokalen CI-Lauf wurde außerdem ein zeitabhängiger Cache-Sperrenfehler
gefunden. Eine deterministische Reproduktion mit einem eigenen Kindprozess
scheiterte vor der Korrektur und bestand danach. Der
[Nachweis zur Sperrenlebensdauer](verification/cache-lock-lifetime-2026-10-22.md)
trennt diesen belegten Mechanismus von der nicht vollständig aufgezeichneten
ursprünglichen Prozessabfolge.

## Deterministische Textprüfung

Codex unterstützte die reinen Rust-Verträge und Tests für begrenzte Textänderungen.
Eine unabhängige Review entdeckte Eingriffe in Links, unvollständig erfasste Zahlen,
verwaiste Kommas und einen fehlenden positiven Selbstkorrekturfall. Die Befunde
wurden mit fehlschlagenden Regressionen reproduziert und korrigiert. Der
[Prüfbericht](verification/refinement-core-2026-10-22.md) trennt diese Regeln
ausdrücklich von noch ausstehenden Modell- und Integrationsnachweisen.

Für das getrennte Worker-Protokoll wurden ebenfalls Tests vor der jeweiligen
Implementierung und eine unabhängige Review eingesetzt. Ein negativer Test
zeigte, dass eine parameterlose Serde-Variante zusätzliche Felder akzeptierte;
eine explizite leere Struktur verhindert dies. Die Protokolltests prüfen
Speichergrenzen, Teilrahmen, Versionen und unbekannte/doppelte Felder in
kontrollierten Speicherstreams; sie ersetzen keine Prozess- oder Modellprüfung.

## Isolated local text worker

Codex assisted the MLX worker, bounded transport, model snapshot and focused
regression tests. Independent review identified blocked control paths and two
model-file ownership issues; targeted follow-up checked their corrections.
A real model smoke test still missed two required corrections. The
[worker report](verification/refinement-worker-2026-10-22.md) preserves that
limitation and does not claim integrated product or quality acceptance.
