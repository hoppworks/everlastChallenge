# Native Unicode- und Clipboard-Grenzprüfung, 2026-10-19

Bestanden: Die produktiven `Snapshot::capture`- und `PreparedClipboard`-Adapter übertragen eine feste Unicode-Fixture über ein eigenes privates NSPasteboard in ein unsichtbares eigenes NSTextView. Der native Eingabepfad `readSelectionFromPasteboard_type` übernimmt exakt 80 UTF-8-Bytes einschließlich Umlauten, Ziffern, Zeilenumbruch und Emoji genau einmal. Anschließend stellt die Lease den vorher gesicherten Seed wieder her.

SHA-256 der übernommenen Fixture: `55dcecf192298167c397764f94f84551b7bd6980287a3cf310b8eb1346a0fcfb`.

Vier echte native Grenzpaare prüfen jeweils Annahme am Limit und `ClipboardError::Limit` bei genau einem zusätzlichen Element/Byte: 16 Items, 128 Typen, 1024 UTF-8-Bytes je ASCII-UTI-Typname und 8 MiB Nutzdaten. Die Sicherung verändert den changeCount nicht. AppKit lehnt nicht-ASCII-UTIs schon vor der Sicherung ab; diese sind kein positiver Typnamenbeleg.

Ausgeführt wurde `cargo test -p macos-integration --test native_clipboard_contract`; Root wiederholte den Vertrag zusammen mit den beiden nativen Tastaturverträgen erfolgreich. Striktes zielgerichtetes Clippy bestand. Root und ein unabhängiger Reviewer haben die Quellen gelesen. Der Vertrag ist im vollständigen autonomen Testprofil registriert.

Der Nachweis ergänzt A21a/P07 auf Ebene N. Er belegt weder physisches ⌘V noch globale Hotkeys, AX-Zielerkennung oder den echten Empfang aus dem gemeinsamen Mikrofonlauf G. Allgemeine Zwischenablage, sichtbare Fenster, Aktivierung und Mikrofon wurden nicht benutzt. Ein optionaler früherer Helferprozessversuch endete mit EOF und zählt nicht als bestandener Nachweis. Materialisierungsfristen und Prozessisolation behalten ihre separaten Belege; die Nutzdatengrenze ist keine RSS-Obergrenze.

Quell-, Binär- und Log-Hashes sowie genaue Beobachtungen stehen im [zugehörigen Nachweis](native-private-clipboard-2026-10-19.json). Rohdaten liegen lokal unter `artifacts/native-clipboard-contract-v1/` und `artifacts/native-keyboard-clipboard-final.log`.
