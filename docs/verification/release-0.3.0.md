# Native Prüfung von Version 0.3.0

Das geprüfte macOS-arm64-Paket enthält den lokalen
MLX-Hybrid für Spracherkennung und den optionalen, gebündelten MLX-Textworker.
Die App verwendet weiterhin ausschließlich native Mikrofon- und Systemzugriffe.
Der zugehörige Laufzeit-Quellstand ist
`0a9c535c1721e40e10b02ed1fcde06840af2c650`; sämtliche Laufzeitdateien sind bytegleich.
Spätere Änderungen betreffen ausschließlich Dokumentation und Nachweise.

## Textbereinigung

Eindeutige, vollständig validierte Korrekturen können direkt in Rust enden.
Andere Eingaben erhalten höchstens einen lokalen Modelldurchlauf. Beide Wege
verwenden denselben sitzungsgebundenen Übergabepfad. Unsichere Ergebnisse und
Abbrüche behalten das Original und halten automatisches Einfügen zurück.

Der [Direktpfad-Nachweis](refinement-direct-2026-10-22.md) prüft 71 bestehende
Textfälle und eine zusätzliche kombinierte Wochentags-/Mengenkorrektur.
Der [Modelldurchlauf-Nachweis](refinement-single-pass-2026-10-22.md) beschreibt
Modellvalidierung und deren Grenzen. Diese Textprüfungen sind keine allgemeine
Genauigkeitsrate für Mikrofonaufnahmen.

## Native Beobachtung

Auf macOS 27 wurde das installierte Paket über den globalen Aufnahme-Hotkey
bedient. Ein kontrollierter deutscher Testsatz enthielt eine Korrektur von Dienstag auf
Donnerstag. Im unabhängigen nativen Testfeld kam „Der Termin ist am Donnerstag um 10 Uhr.“ an. Beobachtet
wurde der vollständige Zieltext, nicht lediglich ein gesendeter Einfügebefehl.
Für diese Aktualisierung war keine erneute Berechtigungsvergabe erforderlich.
Das belegt den beobachteten Wechsel mit derselben lokalen Signieridentität,
nicht die Rechteübernahme auf anderen Rechnern.

Weitere akustische Proben zeigten weiterhin Erkennungsfehler: Ein gesprochener
Fülllaut erschien als „Im“, und ein Satzanfang erhielt unerwünschte Bindestriche.
Beide Fehler waren bereits im Originaltranskript vorhanden. Die konservative
Textbereinigung beseitigt solche Erkennungsfehler nicht zuverlässig. Zahlen,
Namen und wichtige Aussagen sollten vor Weiterverwendung geprüft werden.

## Paketintegrität

Die ZIP wurde erneut entpackt. Alle 346 enthaltenen Dateien stimmen per SHA-256
mit der installierten App überein. Bundle- und Executable-Signaturen,
Architektur, Systembibliotheksbindung sowie die Mikrofondeklarationen wurden
geprüft. Der vollständige Git-Verlauf wurde zusätzlich als Git-Bundle verifiziert.

ZIP-SHA-256: `31602ee0c05d7e90a855a4af5f542aef34b7bfcc8de5446dc249593ef99d7864`.
Das Paket ist lokal signiert und **nicht Apple-notarisiert**. Der
[manuelle Erststart](../usage.md#macos-blockiert-das-erste-öffnen) bleibt erforderlich.
Die Paketprüfung bedeutet keine Gatekeeper-Zulassung oder erfolgreiche Remote-CI.
