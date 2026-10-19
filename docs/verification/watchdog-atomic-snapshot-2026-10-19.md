# Watchdog: atomarer Snapshotwechsel

Die Fehlerkorrektur betrifft ausschließlich das Lesen der regelmäßig atomar ersetzten Produktdatei `snapshot.json`. Der bisherige Immutable-Reader konnte einen gültigen Wechsel zwischen `lstat` und `open` als Inodefehler behandeln oder einen nach dem Öffnen ersetzten alten Dateideskriptor mit `nlink=0` ablehnen.

## Reproduktion und Ergebnis

Der unveränderte Watchdog wurde mit einem eigenen `/bin/sleep`-Kind und einem privaten vollständigen JSON-Snapshot betrieben. Ein eigener Writer ersetzte die Datei im Stressmodus etwa alle 1 ms atomar. Der echte CLI-Lauf brach nach 143 ms/127 Writes mit `observationInvalid` ab; `startObserved=false`, Cleanup bestätigt, ausschließlich das eigene Kind per SIGTERM beendet. Der abschließend lesbare Snapshot war korrekt und 2 ms alt. Siehe `baseline-result.json` und `baseline-cli.log`.

Deterministische Dateisystemtests ergänzen den Lauf: Ein kontrolliertes Rename unmittelbar vor `open` und eines nach dem Read reproduzieren beide konkreten Racefenster. Der neue Reader versucht ausschließlich bei belegtem Austausch erneut zu lesen, maximal achtmal. Ein unverbundener alter Inode wird niemals als aktuelle Beobachtung akzeptiert. Die Datei bleibt auf 4096 Bytes beschränkt; Eigentümer, private Rechte, regulärer Dateityp und einzelne Verknüpfung bleiben vorgeschrieben. Falsche Schema-/PID-/Run-/Zeitwerte lösen keine Wiederholung aus. Die eingefrorene Verzeichnis- und Markeridentität wird vor und nach dem gesamten Snapshot-Read geprüft.

`private`, `read_private`, Prozessidentitätsprüfung, Signal- und Cleanupfunktionen sowie der Haupt-Loop bleiben gegenüber dem Baseline-Quelltext unverändert. Die strengen Leser für Config, Owner-Marker und Anfragen werden nicht gelockert.

Nach der Korrektur bestanden 11 fokussierte Rusttests und striktes Clippy. Drei echte CLI-Stressläufe bestanden mit jeweils ungefähr 6,1 Sekunden und 5323–5375 atomaren Updates; alle endeten regulär per Disarm, Exit 0, ohne `observationInvalid`. Die anschließend notwendigen SIGTERM-Signale betrafen nur die eigenen `/bin/sleep`-Kinder des Probes. `fixed-result.json` enthält die Rohbelege; Disarm bedeutet hier absichtlich nicht, dass der Watchdog das Produkt beendet hat.

## Grenzen

Dies ist eine Watchdog-/Dateisystemreproduktion, kein Diktier- oder Hotkeylauf. Kein Produkt wurde neu gestartet, keine Aufnahme gestartet, keine Benutzeroberfläche bedient und kein Gesten-Ledger verändert. Der ursprüngliche G-Abbruch wird getrennt vom Hauptagenten dokumentiert und bleibt unverändert. Die Reproduktion bestätigt einen Fehler derselben Fehlerklasse; der genaue damals fehlgeschlagene Syscall wurde nicht aufgezeichnet und wird hier nicht nachträglich behauptet.

`verification.json` bindet Quelltext, ursprüngliche und korrigierte Binaries, Commands/Logs und Rohberichte mit SHA-256. Die unabhängige lesende Prüfung durch `r8_artifact_review` ist abgeschlossen: keine handlungsbedürftigen Findings. Quell- und Beleg-Hashes sowie die sechs gegenüber der Baseline unveränderten Schutzfunktionen wurden bestätigt. Der Reviewer startete keine zusätzlichen Tests oder Prozesse.

Rohbelege liegen unter `artifacts/watchdog-atomic-snapshot-v1/`. [Strukturierter Nachweis](watchdog-atomic-snapshot-2026-10-19.json), [ursprünglicher Abbruch](single-press-abort-2026-10-19.md).
