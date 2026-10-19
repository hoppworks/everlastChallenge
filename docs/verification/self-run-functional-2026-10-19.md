# Selbst ausgeführter Funktionstest, 19.10.2026

## Unverändertes Produkt und isoliertes Ziel

Installiert: `/Applications/Everlast Dictation.app`, Produktquellstand
634d6c9, Executable-SHA256
`45f4e463076c88e36adcc7ae95f810329686b32a69be374e0440d0ed7c0fd4ca`.
Beide Produktrechte zuvor direkt als erteilt beobachtet. Keine Produktänderung,
Neuinstallation oder zusätzliche Zugriffsvergabe für diesen Test.

Eigener Empfänger: `crates/macos-integration/examples/native_text_recipient.rs`,
Quell-SHA256
`7f8f2b0ce203d27391806bb251872b8246b91796fb32f4dc5b1b84b331bdba8a`.
Normales AppKit-Fenster mit editierbarem NSTextView und NSTextField sowie
Standard-Bearbeiten-Menü. Kein eigener fremder AX-Zugriff, keine Speicherung,
Netzwerk-, Aufnahme- oder direkte Clipboard-API. Der Text bleibt flüchtig im
eigenen Testfenster. Build, striktes Beispiel-Clippy und Formatierung bestanden.
Unabhängiger Review fand einen fehlenden `paste:`-Menübefehl; dieser wurde
ergänzt und die Korrektur gezielt bestätigt. Standard-Paste mit öffentlichem
Dummytext und anschließendes Leeren wurden tatsächlich geprüft.

Separates signiertes Empfängerbundle:
`artifacts/native-recipient-test-v2/Everlast Testziel.app`, Executable-SHA256
`c53ef8dcfa1a560489db2ea0e130757bf7418a309cf61081180c8140cff7d617`.
Die statische strikte Codesign-Prüfung bestand. Dies ist ein Testempfänger,
keine neue Produktfassung.

## Tatsächliche Beobachtungen

Zunächst war ein öffentliches Selenium-Testfeld mit reinem Dummytext beschreibbar;
der Text wurde anschließend entfernt und das Formular nicht abgesendet.
Die lokale Browserfixture konnte wegen der Browserrichtlinie für `file://`
nicht geöffnet werden. Kein lokaler HTTP-Server oder anderer Umweg wurde
benutzt. Der vollständige Aufnahmeversuch erfolgte ausschließlich mit dem
eigenen lokalen nativen Empfänger, nicht in der öffentlichen Webseite.

Zwei automatisierte ⌥D-Versuche erzeugten `∂` im eigenen Empfänger. Beim
zweiten Versuch war die frisch gestartete Produkt-App sichtbar bereit und
blieb bei diesem Versuch unverändert bereit. Daher ist kein durch die
Automation gestartetes Hotkeydiktat belegt. Escape wurde vorsorglich gesendet,
Audio gestoppt und das Zeichen aus dem eigenen Feld entfernt. Ob dieser
synthetische Eingabeweg denselben Systempfad wie eine physische Taste erreicht,
ist nicht belegt. Die frühere menschliche positive TextEdit-Rückmeldung wird
damit weder ersetzt noch als automatisierte Prüfung ausgegeben.

Anschließend wurde der sichtbare Produktbutton **Aufnehmen** benutzt. Quelle
war die vorhandene öffentliche Kalibrierungsfixture `fleurs-de-dev-1517`,
Original-SHA256 `b892cf9dc80c437f0a78c6f261d83dda196252e31a1e415032981472d3ea9137`.
Eine lokale WAV-Kopie erhielt zehn Sekunden stillen Vorlauf und zwei Sekunden
Nachlauf, die Sprachsamples blieben unverändert. QuickTime spielte die Datei
lokal ab; Everlast nahm über **Systemstandard**, Sprache Deutsch, auf.
Aufnahme läuft → Stoppen → Verarbeitung → Diktat ist fertig wurden direkt
beobachtet. Das ist ein akustischer Funktionstest, keine direkte PCM-Zuspielung
oder Leistungs-/Qualitätsqualifikation.

Referenz:

> Forschung im Bereich KI umfasst die Herstellung von Maschinen zur
> Automatisierung von Aufgaben, die intelligentes Verhalten erfordern.

Tatsächliches Transkript: genau dieser Satz, anschließend **„Vielen Dank.“**.
Die Ursache des Zusatzes ist nicht festgestellt; insbesondere ist ohne
gespeicherten Mikrofonmitschnitt weder eine reine Modellerfindung noch eine
Umgebungsäußerung bewiesen. Keine fehlerfreie Erkennungsqualität daraus ableiten.

Danach wurde **Kopieren** in Everlast angeklickt und über das normale ⌘V im
eigenen Testfenster eingefügt. Ein tatsächlicher AX-Vergleich bestätigte:
Produkt meldet fertig; sein Transkript und der Zieltext stimmen exakt überein;
der gesamte Text steht genau einmal im Ziel; kein zusätzliches `∂` im Endtext.
Dies prüft den expliziten Kopierweg mit agentengesteuertem Einfügen, **nicht**
Everlasts automatische Zielerfassung und den automatischen Versand.

## Grenzen und Belege

Lokale gebundene Metadaten: `artifacts/native-recipient-test-v2/evidence.json`;
genaue Beobachtungen und Ergebnisassertion in den CUA-Ausgaben. Der Empfänger
wurde anschließend regulär geschlossen; seine Inhalte werden nicht persistiert.
