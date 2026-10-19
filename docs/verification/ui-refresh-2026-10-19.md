# Native UI-Überarbeitung, 19.10.2026

Ausgangsbasis: d680f35; abgeschlossener UI-Änderungsumfang im zugehörigen Commit.
Nutzerauftrag: größeres nutzbares Standardfenster, Progressive Disclosure,
erreichbare Einstellungen; Deutsch mit englischen macOS-Bezeichnungen.

## Änderung

- Standardfenster 760×720, Minimum 560×600; native Kopfleiste 64 statt 104 pt.
- Hauptansicht mit kompaktem Berechtigungshinweis und flexiblem Transkriptfeld.
- Einstellungen ersetzen die Hauptansicht: Aufnahme, Berechtigungen, Modell.
  Tastenkürzel und technische Modelldetails werden erst aufgeklappt.
- Settings-Navigation bleibt beim Modellladen und während einer Aufnahme offen;
  zustandsabhängige Sperren für tatsächliche Änderungen bleiben bestehen.
- Englische Pfade zu Microphone und Accessibility; erneute AX-Anfrage bei denied
  ausdrücklich verfügbar. Keine Berechtigung automatisch vergeben.

## Belege und Grenzen

215 Frontendtests bestanden; zwei neue Navigationsregressionen zuvor rot und
anschließend grün. `npm run check` bestanden. Native Geometrie-Vertrag prüft
560/760/1000 px; Desktopvertrag prüft Settings beim Laden. Unabhängige kombinierte
Reviews: application prüfte Frontend, settings prüfte native Änderungen, jeweils
ohne Befund. Impeccable-Detektor einmal nach finalen Änderungen: `[]`.

Releasebuild erfolgreich, Bundleprüfung und strikte Codesign-Prüfung bestanden.
Sichtbare Prüfung der installierten App per CUA: Hauptansicht während Loading
und Ready, Settings → Berechtigungen sowie Aufnahme, Schließen zurück zur
Hauptansicht. Im Standardfenster vollständig sichtbar, keine Seiten-Scrollleiste.
Settings bei Loading tatsächlich bedienbar. Minimumfenster nur geometrisch und
quellenbasiert geprüft; VoiceOver und vollständige Systemdarstellungen offen.

Die installierte aktuelle Kopie liegt unter `/Applications/Everlast Dictation.app`.
Vorherige beide Installationen wurden unverändert unter
`artifacts/testversion-retired-bundles/ui-refresh-2026-10-19/` archiviert;
Appdaten und TCC-Datenbank unverändert. Die ZIP-Fassung 7b3dee7 bleibt ein
historisches anderes Bundle und enthält diese UI-Überarbeitung nicht.

Aktueller sichtbarer Rechtezustand nach Installation: Microphone notDetermined,
Accessibility denied. Nutzer um eigene Freigabe gebeten. Keine Aufnahme oder
Zielprogramm-Einfügung in dieser UI-Prüfung durchgeführt; keine P02-/P07-/P09-
Vollabnahme daraus abgeleitet. P06-Laufzeitpolitik unverändert Referenz.

Logs: `artifacts/full-completion-audit/ui-{navigation-red,navigation-green,tests-full,check,release-build,bundle-verify}.log`
und `ui-design-detect.json`.

## Nachtrag: Berechtigungsdiagnose nach Nutzer-Rückmeldung

Mikrofon wird inzwischen in der laufenden App erkannt (Bereit zum Diktieren).
System Settings zeigt Everlast eingeschaltet unter „Device Control and Data
Access“, während die App AXIsProcessTrusted=false erhält. Einzige laufende Kopie:
`/Applications/Everlast Dictation.app`, PID 29519 zum Beobachtungszeitpunkt.

Eine ausschließlich lesende, auf exakt die eigene Bundle-ID eingeschränkte
Abfrage der systemweiten TCC-Datenbank ergab für kTCCServiceAccessibility
`auth_value=2`, aber Code-Requirement-CDHash
`3df69d982add5e6b8ba0f58440142ed466d3bcd3`. Dieser stimmt exakt mit der
archivierten alten `/Applications`-Kopie überein. Die aktuelle installierte App
hat CDHash `1c8b42152529c06c1e10a2dcba858561bf9c3854`; ihre binäre SHA256 ist
`e5e010fb470f030985ff3a3a8520b28a2f0a3ac1f5c6986c0d0b965cd5493637`.
Damit ist die alte Signaturzuordnung belegt; kein bloßer UI-Refresh-Verdacht.
Benutzerspezifische TCC-Datenbank nicht lesbar, kein Umgehungsversuch. Keine
Datenbankmutation, Rechteänderung oder Signaturänderung vorgenommen.

Unabhängige native Quellenprüfung durch settings: API/ABI korrekt, unmittelbare
Abfrage im Appprozess, Aktualisierung bei Start/Fokus/Anfrage korrekt vorhanden.
Nutzer um gezieltes Entfernen des alten Eintrags und Hinzufügen des exakten
aktuellen Pfads gebeten. Abschluss dieser Vergabe noch nicht nachgewiesen.
