# R9: tatsächlicher Browserdownload und native Entpackung

Das unveränderte R9-ZIP wurde über eine eigene lokale Downloadseite im echten
Brave-Browser heruntergeladen und im nativen Save-Dialog nach Downloads
gespeichert. Der kurzlebige Server band ausschließlich 127.0.0.1 und lieferte
nur die bekannte ZIP-Datei; keine Verzeichnisfreigabe, kein öffentlicher
Upload und keine sonstigen Downloads-Dateien wurden bereitgestellt.

Die gespeicherte Datei `Everlast-Dictation-212d5e7-r9-browser.zip` besitzt
6.820.064 Bytes und SHA256
`ff3329ac27c86f497c5aeb93d5b66d7f9f28630e7af914f0ab9c7c86e11a2933`.
Sie ist bytegleich mit dem unabhängig geprüften R9-Archiv. Das vorhandene
com.apple.quarantine nennt Brave; WhereFroms verweist auf die lokale Seite.
Kein Quarantäneattribut wurde manuell gesetzt, entfernt oder abgeschwächt.

Finder öffnete exakt diese ZIP mit Archive Utility. Die extrahierten 468
regulären Dateien samt Modi sind identisch mit dem R9-Staging. Die App erbte
das Quarantäneattribut. Strikte Signatur und Mikrofondeklarationen bestehen;
Executable-SHA256 bleibt
`c79d838d2c999a907a850217d131b6c1b5b6d0bc6881505cb906b195210eae2b`.

Die rein lesende Gatekeeper-Bewertung mit `spctl --assess --type execute`
ergab Exit 3/rejected. Das ist keine erfolgreiche Gatekeeper-Öffnung. Kein
Override, keine Sicherheits-/Vertrauensänderung und kein Start der entpackten
Kopie wurden vorgenommen; die installierte normale App bleibt unverändert.
Der lokale HTTP-Server ist beendet. Der eigens geöffnete Finder-Tab wurde
geschlossen; andere Finder-Inhalte blieben unberührt.

## Abnahmegrenze

[Strukturierte Bindung](r9-browser-download-2026-10-19.json). Die anfängliche
Browser-Steuerung lief beim noch offenen Save-Dialog in ein Timeout. Es wurde
kein zweiter Download ausgelöst; derselbe Dialog wurde über die native UI
abgeschlossen und die tatsächliche Datei anschließend geprüft.

Der unabhängige Readback bestätigt fünf Artefaktbindungen, heruntergeladene
Datei-/Quarantänemetadaten, alle 468 extrahierten Dateien samt Modi, Signatur,
Mikrofondeklarationen und das unveränderte installierte Binary. Server-PID
und lokaler Port sind nachweislich nicht mehr aktiv. Keine Befunde; die
Gatekeeper- und weiteren Abschlussgrenzen bleiben bestehen.
