# Eigener nativer Testbeobachter: tatsächlicher Lauf am 19.10.2026

Lokal signierte App aus dem geänderten `native_text_recipient`-Example gestartet; keine zusätzliche AX-/Mikrofonfreigabe und keine Mikrofonaufnahme. App-PID95363 wurde nach regulärem Quit als beendet geprüft. [Maschinenlesbarer Beleg](native-recipient-observer-2026-10-19.json).

Der automatische250msBericht entstand tatsächlich:210periodische Beobachtungen und ein gestoppter Endzustand. Berichtsverzeichnis0700, Snapshot0600. Über CUA ausschließlich den öffentlichen FLEURS-Satz1517 im eigenen Fenster gesetzt: einmal ergibt exakte Übereinstimmung/Anzahl1, doppelt ergibt keine exakte Übereinstimmung/Anzahl2. Danach das eigene Feld geleert und Anzahl0 bestätigt. Die Berichtsdaten enthalten keinen eingegebenen Rohtext.

Bei der Texteingabe war der eigene FirstResponder „multiline“, aber AppActive und WindowKey beidefalse. Der neue CLI-Bereitschaftstest wies genau diesen frischen Zustand zurück, Exit1 vor Tests oder Audio. Das belegt, warum die per-App-AX-Fokusangabe allein für einen globalen Startfeldnachweis unzureichend war. Es isoliert noch nicht die Ursache des früheren Everlast-Startzielfehlers.

Beim regulären Beenden meldete der letzte Snapshot AppActive/WindowKeytrue, aber runningfalse/statusstopped. Daraus wird keine Startbereitschaft abgeleitet; der Prozess war anschließend abwesend. Keine positive globale Hotkey-/Everlast-Übergabe oder Behebung des Notes-Problems behauptet.
