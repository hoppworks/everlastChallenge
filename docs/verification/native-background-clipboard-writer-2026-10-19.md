# Native Hintergrundschreiber auf privatem Pasteboard · 2026-10-19

Der native Contract startet nach der echten `PreparedClipboard::install` einen separaten Prozess desselben Testprogramms. Dieser öffnet dasselbe explizit benannte private Pasteboard und ersetzt dessen Inhalt durch einen festen Sentinel. Der Elternprozess wartet auf den erfolgreichen Prozessabschluss. Anschließend liefert die echte `restore`-Implementierung `OwnershipChanged`; die native Rücklesung enthält exakt den Sentinel und der Change Count bleibt durch `restore` unverändert.

Der Helfer akzeptiert ausschließlich `org.everlast.clipboard-contract.<positive PID>.<32 lowercase hex>` und prüft vor dem Schreiben den bereits installierten festen Testtext. Er verwendet weder das allgemeine Clipboard noch Fenster, Mikrofon, Accessibility oder Tastensimulation. Die bisherige Unicode-Einfügung in das eigene unsichtbare Textfeld sowie alle vier nativen At-Limit-/One-Over-Limit-Paare bleiben enthalten.

## Nachweise

- RED: Ohne separaten Schreiber scheiterte der neue Contract tatsächlich mit `Restored` statt `OwnershipChanged` (Cargo 101).
- GREEN: `cargo test -p macos-integration --test native_clipboard_contract --locked` bestand einschließlich des echten Subprozesses.
- `cargo clippy -p macos-integration --test native_clipboard_contract --locked -- -D warnings` bestand.
- Quell-, Binary- und Rohlog-Hashes stehen im gleichnamigen JSON; Rohdaten liegen unter `artifacts/native-background-clipboard-writer-2026-10-19/`.

## Grenzen

Dies belegt einen tatsächlichen separaten Schreiber auf demselben privaten Pasteboard, keinen fremden Anwendungsprozess auf dem allgemeinen Clipboard. Physisches Cmd+V, fremde Textfelder und G werden dadurch nicht abgenommen. Der Schreiber hat eine Zwei-Sekunden-Frist; bei Timeout oder Wartefehler wird ausschließlich der eigene Kindprozess beendet und synchron eingesammelt. Dieser Fehlerpfad wurde hier nicht künstlich ausgelöst. Das normale Produkt wurde nicht geändert; historische Clipboard-Nachweise bleiben unverändert.
