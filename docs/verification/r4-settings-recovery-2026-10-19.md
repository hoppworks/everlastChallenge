# R4: echte Einstellungswiederherstellung und korrigierte Statusprojektion

Am 19.10.2026 im tatsächlich installierten R4 (`ee1a6a4`) durchgeführt.
[Strukturierter Beleg](r4-settings-recovery-2026-10-19.json).
Lokale Rohbelege: `artifacts/r4-recovery-acceptance-v1/`; gesicherte
Nutzereinstellungen verbleiben ausschließlich dort, nicht in Git.

## Tatsächlicher installierter Ablauf

1. App regulär beendet und Prozessabwesenheit geprüft. Aktuelle Einstellungen
   bytegleich gesichert; Größe/Dateirechte und Modellmetadaten festgehalten.
2. Ausschließlich die eigene Einstellungsdatei atomar durch eine absichtlich
   ungültige synthetische JSON-Fixture ersetzt. App unverändert gestartet.
3. Sichtbarer Wiederherstellungszustand, keine Aufnahme-/Modelldownloadaktion.
   „Einstellungen zurücksetzen …“ öffnet einen echten nativen Bestätigungsdialog.
4. „Abbrechen“ hält den Wiederherstellungszustand; Fixture bleibt bytegleich.
5. Nach ausdrücklich erneutem Anfordern „Zurücksetzen“ bestätigt. Sichtbar
   wieder bereit; gültige JSON-Einstellungen mit Modus `0600`. Beide vorhandenen
   Modelle in Größe, mtime und Inode unverändert, kein erneuter Download.
6. App erneut regulär beendet, Originaleinstellungen bytegleich und mit ihren
   ursprünglichen Dateirechten zurückgesichert. Dieselbe installierte App
   gestartet: Modellprüfung, anschließend bereit mit leerem Text.

Keine Mikrofonaufnahme, keine TCC-/Sicherheitsänderung, keine Neusignierung und
keine Änderung an den separat erhaltenen Daten vor R4. Dies ergänzt die reale
P08-Recovery-Abnahme; es ist kein neuer Nachweis für A25s Signaturwechsel,
Offlinebetrieb oder vollständigen P08-Abschluss.

## Im echten Fenster gefundener Fehler

Bei ungültiger Einstellungsdatei zeigte der Header „Modell konnte nicht
eingerichtet werden“, während der Inhalt die richtige Ursache nannte. Die
Ursache erschien doppelt. Zusätzlich wurden Mikrofon-/Accessibility-Hinweise
ausgegeben, obwohl dieser Startpfad die Rechte noch nicht abgefragt hatte.

Die gemeinsame Headerprojektion behandelte den aus Recovery resultierenden
Setup-Fehler als Modellproblem. Der Workspace zeigte gleichzeitig Recovery-
Ursache und denselben allgemeinen Setupfehler. Das ist kein Beleg für verlorene
OS-Freigaben oder defekte Modellgewichte.

## Enger Quellfix, noch nicht in der installierten R4-App

- Nach dem bereits beendeten Zustand hat Settings-Recovery Vorrang im Header
  und damit auch im Menüleistenfeedback. Status: „Einstellungen können nicht
  geladen werden“, keine Start-/Stop-/Settingsaktion im Header.
- Workspace unterdrückt die identische Ursache als zweite Fehlermeldung und
  die verfrühten Berechtigungshinweise. Ein abweichender Fehler beim Speichern
  der Standardwerte bleibt sichtbar.
- Keine Änderung an Capture, Decoder, Modellwahl, Zielprüfung, Speicherformat
  oder Resetbestätigung. Die vorhandene installierte R4-App bleibt unverändert.

Der neue Regressionstest an den öffentlichen Snapshot→Native-State-Grenzen
war zuerst tatsächlich rot: erwarteter Einstellungsstatus gegen tatsächlichen
Modellstatus (Exit101). Nach der Korrektur grün. Er prüft zusätzlich fehlende
Aktionen, gemeinsame Statusmeldung, einmalige Ursache, unterdrückte Rechtehilfe,
erhaltenen abweichenden Resetfehler und Vorrang des beendeten Zustands.

`npm run check` bestanden (Format, Workspace-Clippy, TypeScript, Oxlint,
Guardrails, Vertragsprüfung). Alle Hostbibliothekstests: 224 bestanden,
0 fehlgeschlagen, 2 ausdrücklich ignorierte Modelltests. Ein kombinierter
unabhängiger Korrektheits-/Standards-/Anforderungsreview ohne Befund. Keine
Wiederholung unveränderter ASR-Korpus- oder mehrstündiger Leistungsreihen.
