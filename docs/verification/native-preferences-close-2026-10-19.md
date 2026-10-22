# Schließvertrag eigener nativer Einstellungen · 19.10.2026

Der gezielte AppKit-Vertrag reproduzierte **kein** Verbergen oder Verlieren des
separat besessenen Hauptfensters beim Schließen der Einstellungen. Das ist keine
Aussage über den Lebenszyklus des installierten Tauri-Fensters oder die
Fenstererkennung durch CUA.

## Ausgeführte Grenze

`crates/macos-integration/tests/native_preferences_close_contract.rs` läuft auf
dem tatsächlichen Hauptthread mit frischer `NSApplication`, eigener gehaltener
`NSWindow` und der echten `NativePreferences`-Komponente. Der kontrollierte
Zustand enthält keine Modell-/Audiooperation. Nach Anzeigen des Hauptfensters
folgen drei Zyklen aus Einstellungen anzeigen und nativem Schließen per
`performClick`. Nach jedem Zyklus sind die Einstellungen unsichtbar, während
das Hauptfenster sichtbar und in `NSApplication.windows` enthalten bleibt.
Auch `NativePreferences::close()` und `detach()` erhalten es. Abschließend
schließt der Test sein eigenes Hauptfenster.

Beobachtung in allen drei Zyklen:

```text
preferencesVisible=false; mainVisible=true; mainInOwnWindows=true
```

Ausgeführter Befehl:

```sh
cargo test --locked --offline -p macos-integration --test native_preferences_close_contract
```

Ausführung 38169 endete mit Exit 0. Das gespeicherte Log enthält erfolgreichen
Build und drei Beobachtungen. Die gezielte strikte Clippy-Prüfung endete
ebenfalls mit Exit 0; sie ist kein zweiter unabhängiger nativer Lauf.

## Gebundene Nachweise

| Datei | SHA-256 |
|---|---|
| `crates/macos-integration/tests/native_preferences_close_contract.rs` | `09c2b8e3160bc362c997e8dc226d44db0a55559598d4a29e925dcc26bfedfbe3` |
| `artifacts/offline-continuation-v1/preferences-close-contract.log` | `c5bfae3cac1ab7f27aeb7ed18acc5ffbbc294a5799d7c7b05b656cb163835689` |

Der Test ist als eigener nativer Test mit `harness = false` registriert.
Die Hashes identifizieren gelesene Test-/Logdateien, keine installierte App.

## Abgrenzung

- Eigene AppKit-Fenster ersetzen weder Tauri/Wry-Hauptfenster noch die
  Zusammenarbeit der Delegates im installierten Produkt.
- Der Klick erfolgt im eigenen Prozess. CUA, fremde AX-Abfragen, Aktivierung,
  Tastatureinspeisung, Mikrofon, Berechtigungsdialoge und Modell fehlen.
- CUA-Zeitüberschreitungen, Wiedererreichbarkeit, Fenstermenü und alle möglichen
  Schließen-/Verbergen-Wege werden nicht geprüft.
- Der Tauri-Schließhandler ist am Tauri-Hauptfenster registriert; daraus folgt
  nicht, welches Fenster eine konkrete CUA-Aktion ausgewählt hat.

Die ursprüngliche Unerreichbarkeit über CUA bleibt eine gesondert zu
reproduzierende Integrationsbeobachtung. Dieser Vertrag grenzt die Hypothese
ein, widerlegt die Beobachtung aber nicht und belegt keine vollständige Abnahme.
