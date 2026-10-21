## Die sieben Befunde

| Befund | Korrektur | Gezielte Evidenz |
|---|---|---|
| Sichtbarer Text und kopierter Text konnten auseinanderlaufen | `copyTranscript` sendet das sichtbare Payload; Rust prüft vor Clipboard-Dispatch die 256-KiB-Grenze. Kein Runtimezugriff für Copy | Angezeigter Text bleibt Copy-/Clear-Grundlage; leerer/überlanger UTF-8-Text abgewiesen; IPC-Aufruf mit exakt diesem Payload |
| Ungeeignetes Standard-Mikrofonformat verhinderte nutzbare Alternativen | Bestehenden kompatiblen Default behalten; sonst angebotene Formate deterministisch wählen | 96-kHz-Default mit 48-kHz-Alternative; reihenfolgeunabhängige Wahl; ungültige Kanäle/Raten; fehlender Default und fehlgeschlagene Geräteabfrage |
| Frischer Entwicklungsrechner unvollständig beschrieben | Rustup-Bootstrap, CMake und Xcode-/Metal-Komponente mit geprüften Versionen ergänzt; `npm run doctor` | Qualifizierter Host, fehlende Werkzeuge, falsche Versionen und installationsfreie rustup-Aufrufargumente getestet |
| Aktuelle Einstiege mischten historische Zustände | Aktueller Codeweg und aktuelle KI-Dokumentation eindeutig; alte Installations-/Prüfstände historisch gekennzeichnet | Dokumente gegen die geänderten Pfade gelesen; keine historische Messung verändert |
| Cleanupfehler verdeckte Assertion und übersprang zweite Fixture | Unabhängig registriertes Cleanup; nicht bestätigte Beendigung ebenfalls Testfehler | Kontrollierter PermissionError erhält Assertion plus Cleanupfehler; `False` beim zweiten Cleanup wird rot; gesamte Python-Suite jetzt grün |
| Terminaler Dienstfehler bot unmögliche Wiederholung | `running=false` hat Vorrang vor Setup/Settings; letzter Text bleibt kopierbar; Owner- und Joinursachen werden erhalten | Letzte Zustände Idle/Recording, gleichzeitig offene Settings/Recovery, gültiger Download-Retry bei laufendem Dienst; spezifische Fehler am Client und beim Ownerabschluss |
| Überholte Aktionsfehler blieben nach Erfolg sichtbar | Anfrageidentität begrenzt die Wirkung von Quittungen; transiente Transport- und dauerhafte Registrierungsfehler getrennt | Passender Erfolg löscht alten Fehler; verspäteter Erfolg/Fehler ändert neuere Meldung nicht; Registrierung bleibt trotz anderem Erfolg sichtbar |

Die Formatwahl findet vor Streamstart außerhalb des Audiocallbacks statt. Sie
erweitert den DSP nicht. CPAL kann dabei die nominale Hardware-Abtastrate
systemweit ändern. Externe Interfaces sind durch die kontrollierten Tests nicht
als Hardware qualifiziert.

## Unabhängige Gegenprüfung

Ein unabhängiger Reviewer prüfte das zusammenhängende Delta auf Standards und
Anforderungserfüllung. Vier Nachträge wurden korrigiert und gezielt nachgeprüft:
Fehlerverlust bereits im AppSupervisor, transiente Hotkeyfehler fälschlich als
dauerhaft, automatisch installierende Rust-Proxies im Doctor und ignoriertes
Cleanup-Ergebnis. Danach keine weiteren konkreten Befunde im begrenzten
Nachreview. Das ist keine allgemeine Fehlerfreiheits- oder Sicherheitsgarantie.

## Zusätzlicher Dependency-Befund

Der frische `cargo deny check` meldete `RUSTSEC-2026-0381` und
`RUSTSEC-2026-0382` für die bisher gebundenen TLS-Zertifikatsabhängigkeiten.
`rustls-webpki` wurde gezielt auf 0.103.16 und `webpki-roots` auf 1.0.10
aktualisiert, einschließlich der versionsgebundenen Lizenzregel. Andere
Lockfile-Auflösungen wurden erhalten. Der anschließende Lauf besteht Advisories,
Bans, Lizenzen und Quellen; erlaubte Mehrfachversionen bleiben Warnungen.

## Ausgeführte Prüfungen

Rohlogs: dem lokalen Prüfarchiv (nicht mitgeliefert) (lokal, nicht versioniert).

| Prüfung | Ergebnis |
|---|---|
| `npm run check` | bestanden, inklusive Rust-Format/Clippy, generierte Verträge und Abhängigkeitshygiene |
| `npm run test:portable` | 224 Frontend- und 21 Guardrailtests bestanden |
| Workspace-Library/Binary mit `measurement,test-control` | 447 bestanden, 3 modell-/audioabhängige Tests ausdrücklich ignoriert |
| Core-Integration | 31 bestanden |
| `npm run test:quality` | 226 bestanden |
| Node-Werkzeuge | 156 bestanden, 1 optionaler nativer Test übersprungen (vollständiger finaler Lauf) |
| `npm run test:guardrails` | positive/negative Regeln und Coverage-Grenzfixtures bestanden |
| Secretprüfung, `npm audit --audit-level=high` | bestanden; npm ohne Sicherheitsfunde |
| `cargo deny check` nach gezielten Updates | bestanden; ursprünglicher roter Lauf erhalten |
| Coverage | Core: 97,48 % Zeilen; Frontend: 99,01 % Zeilen / 95,47 % Zweige; alle bestehenden Hostgrenzen bestanden, Desktop 61,20 % |
| Frischer Checkout / Release | neuer Clone, Setup und Release aus neuem Target-Verzeichnis bestanden; ZIP-Rundlauf, lokale Signatur und Installation geprüft |

Die absichtlichen roten Copy-, Terminal-, Fehlerursachen-, Cleanup- und
Mikrofonformatproben liegen neben den grünen Gegenproben. Der ursprüngliche
Python-Gesamtlauf des Reviews bleibt unter dem lokalen Prüfarchiv (nicht mitgeliefert)
erhalten. Kein grüner Wiederholungslauf wird als damaliger Erfolg ausgegeben.

## Wiederverwendung und Grenzen

Modell, Decoder, VAD-/DSP-Algorithmus und Sitzungskern wurden nicht geändert.
Historische Qualitäts-/Langzeitnachweise behalten ihre ursprüngliche Bindung;
sie sind kein neuer Test des finalen Pakets. Audioformatwahl und UI-/Fehlerpfade
haben neue gezielte Softwarebelege. Die Registry-/FFI-Implementierung der
physischen Tastenkürzel wurde nicht geändert.

## Reales Paket und Kurzabnahme

Das neue ZIP liegt unter dem lokalen Prüfarchiv (nicht mitgeliefert).
SHA-256: `a4d7786a59d9ef659216d02ffab782ce4d9181b6f5839b73bdb089999e8f863d`.
Der installierte ausführbare Stand ist bytegleich zum ZIP-Rundlauf:
`3a6fccc60374c0a8f933af60c4c0b205151bd4340bc311008afd9a94dd2f3c54`.
Die vorherige Installation bleibt als `previous-installed.app` im lokalen
Prüfverzeichnis erhalten. Die bestehenden Einstellungen wurden beibehalten;
aktives Modell: `mlx_reference_encoder`, Sprache Deutsch, Standardmikrofon.

Zusätzlich wurde die neue native Copy-IPC im normalen Release ausgeführt und das
Ergebnis im eigenen leeren nativen Testfeld gelesen: identischer Text, 20 UTF-8-
Bytes und passender SHA-256. Dieser Beleg ist ausdrücklich manuelles Copy/Paste,
nicht ein zweiter automatischer Übergabenachweis.
