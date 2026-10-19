# Zusammengeführter autonomer Softwarestand

[Befund- und Quellbindung](single-press-integrated-software-2026-10-19.json).
`npm run test:autonomous:full` bestanden: sieben Gruppen,463Rust-,215UI-,
92bestandeneNode- und204Python-Tests. Ein ausdrücklich optionaler nativer
JS-Adaptertest wird im Standardlauf übersprungen; sein echter Helperlauf war
separat mit9/9bestanden. Drei Rust-Fixture-/Messfälle sind im normalen
Vertragslauf ignoriert; der neue echte P08-Dateifall wurde separat tatsächlich
ausgeführt. Kein Hardware-, GUI-Automations- oder neuer ASR-Korpusnachweis
wird aus dieser Suite abgeleitet.

Das gemeinsame CLI-Kommando bindet aktuelle private Konfiguration und
Vorbereitungsbeleg an Run-ID, Produkt-/Empfänger-PIDs, installierten Binaryhash,
normalen Buildlog und Default-Fingerprint. Die wirklichen Codesign-/Plist-
Prüfungen lesen ausschließlich das eigene installierte Bundle. Preflight
benutzt dieselbe Bereitschaftsprüfung wie der Runner und erzeugt keine
Startaufforderung, keinen Ledger-Verbrauch, Audio-/Helperstart oder
Produktbefehl. Die vollständige Run-Verdrahtung ist per kontrollierten
Factory-/Runnerverträgen geprüft; `--run` wurde noch nicht tatsächlich gestartet.
Neun Einstiegstests und unabhängiger Review bestanden.

Fenster geschlossen und Empfänger vorbereitet bleiben tatsächlich extern
beobachtete CUA-Fakten. Die Datei- und Hashprüfung authentifiziert deren
Bindung, ersetzt diese Beobachtung jedoch nicht. Ebenso belegt ein
Default-Fingerprint die Buildfeatures, nicht aus eigener Kraft die komplette
Herkunft des Executables; tatsächlicher eigener Build, Quellen und Signatur
sind zusätzlich gebunden.

Die erlaubte UI-Abfrage der eindeutig ausgewählten installierten App meldete
nun einen gesperrten Mac. Kein Entsperrversuch über andere Werkzeuge und keine
Installation oder Prozessbeendigung als Ersatz. Die installierte App bleibt
unverändert. Neuer Kandidat und eigener Empfänger sind vorbereitet; echte
Installation, frische Rechte-/Fokusprüfung und der einzige G-Lauf stehen aus.
Der reale dauerhafte Gestenvermerk existiert weiterhin nicht.
