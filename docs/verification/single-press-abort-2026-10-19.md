# Gemeinsamer Lauf vor beobachtetem Start abgebrochen

Der vorbereitete normale R11-Lauf `c8f703d5cdac105461852d20def543af` erreichte `waiting-for-start`. Produktidentität, Modell, Rechte und tatsächlicher Empfängerfokus waren zuvor geprüft. Nach der einzigen Startaufforderung beendete der unabhängige Wächter den Produktprozess mit `observationInvalid`; seine Aufräumung wurde bestätigt. Der Runner meldete anschließend `stale-observation`.

Die separate CLI-Reproduktion arbeitet ausschließlich mit einem eigenen harmlosen Kindprozess und einer privaten, atomar ersetzten Statusdatei. Der bisherige Wächter scheiterte nach 143 ms trotz eines zwei Millisekunden alten gültigen Snapshots. Zwei Leserennen sind im Quelltext nachvollziehbar: Inodewechsel zwischen `lstat` und `open`, beziehungsweise ein nach `rename` nicht mehr verlinkter bereits geöffneter Dateideskriptor. Die genaue Syscall-Ursache des ursprünglichen Laufs wurde nicht protokolliert; die Reproduktion belegt die Fehlerklasse, nicht rückwirkend diesen einzelnen Syscall.

Der dauerhafte Gestenbeleg bleibt unverändert verbraucht. Kein weiterer Tastendruckauftrag, kein stiller Neustart dieses Laufs und kein Zurücksetzen des Belegs. Der fehlgeschlagene Lauf bleibt als solcher erhalten. Vorhandene unabhängige gültige Belege zu 181 Hoststarts, kurzer tatsächlicher Übergabe, Korpusqualität und lokalem Maximalstopp werden dadurch nicht verworfen. Der globale gemeinsame Release-Nachweis bleibt offen.

[Strukturierte Fakten und Beleg-Hashes](single-press-abort-2026-10-19.json).
