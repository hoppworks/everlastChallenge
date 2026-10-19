# A18: begrenzter echter Carbon-Registrierungskonflikt

Am 19.10.2026 wurde ausschließlich **Control+Command+Option+Shift+F20**
mit `RegisterEventHotKey` und `kEventHotKeyExclusive` kurz registriert.
Die erste Registrierung musste erfolgreich sein; bei bereits belegter Kombination
hätte die Probe ohne Entfernen fremder Registrierungen unentschieden beendet.
[Quell-, SDK-, Binary- und Rohbindungen](a18-carbon-collision-2026-10-19.json).

| Schritt | Tatsächlicher OSStatus |
|---|---:|
| Erste exklusive Registrierung | 0 |
| Gleiche Kombination erneut registrieren | -9878 (`eventHotKeyExistsErr`) |
| Nach dem Fehler nochmals auf vorhandenen Besitz prüfen | -9878 |
| Ursprüngliche eigene Registrierung entfernen | 0 |
| Kombination wieder registrieren | 0 |
| Neue eigene Registrierung entfernen | 0 |

Minimaler C-Prozess, PID 7196, Laufdauer 0.160 s,
Exit 0 und über seinen Handle beendet abgewartet. Beide erfolgreichen Referenzen
wurden explizit entfernt; Fehlerpfade besitzen zusätzlich begrenztes eigenes Cleanup.
Der vollständige Quelltext und die Rohwerte liegen unter
`artifacts/a18-carbon-collision-v1/`.

Keine Tastaturereignisse erzeugt, kein Eventhandler/Eventloop gestartet, keine
Fenster bedient, keine Einstellungen verändert und keine fremde Registrierung
entfernt. Die SDK-Deklarationen und Fehlerkonstanten sind gehasht gebunden.

Der Befund belegt **nur den realen Carbon-Konflikt und die Freigabe eigener
Registrierungen**. Er prüft weder die produktive Einstellungsänderung samt Rollback
noch den physischen Hotkey-/Layoutpfad oder eine installierte App. A18 und die
Gesamtpakete werden dadurch nicht vollständig abgeschlossen.
