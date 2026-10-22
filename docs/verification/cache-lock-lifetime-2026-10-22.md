# Lebensdauer der Modellcache-Sperre

Im lokalen autonomen Lauf vom 22.10.2026 scheiterte
`real_lock_contention_and_drop_release_preserve_existing_lock_bytes` mit
`ModelError::Busy` beim erneuten Erwerb nach Ende des bisherigen Besitzers.
357 Hosttests bestanden, einer scheiterte, drei waren ausdrücklich ignoriert.
Isoliert bestand der ursprüngliche Test; bloßes Wiederholen wäre daher kein
Nachweis einer Korrektur gewesen.

## Deterministische Reproduktion

Der neue Test
`dropping_guard_releases_lock_while_inherited_child_handle_stays_open`
übergibt eine Kopie des tatsächlich gesperrten Dateideskriptors als stdin an
einen eigenen Kindprozess. Dieser hält die Dateibeschreibung offen, während der
Elternprozess seinen `CacheGuard` beendet. Vor der Korrektur scheiterte der
anschließende Sperrerwerb reproduzierbar mit `Busy`.

Dies entspricht der [dokumentierten Rust-Dateisperrensemantik](https://doc.rust-lang.org/std/fs/struct.File.html#method.lock):
Schließen eines Deskriptors gibt die Sperre erst frei, wenn auch seine Kopien
und geerbten Deskriptoren geschlossen sind. Ein ausdrückliches `unlock` löst sie
unabhängig davon. Der ursprüngliche parallele Lauf protokollierte die Vererbung
nicht; seine konkrete Prozessabfolge wird daraus nicht nachträglich behauptet.

## Korrektur und Prüfung

`CacheGuard` gibt die Sperre beim Ende ausdrücklich frei. Der Guard wird sofort
nach erfolgreichem Erwerb angelegt, sodass auch anschließender Abbruch oder ein
Fehler beim Setzen der Rechte diesen Freigabepfad verwenden. Ein Fehler beim
Entsperren wird gemeldet; der Dateideskriptor wird anschließend weiterhin
geschlossen. Die dauerhafte Sperrdatei wird nicht gelöscht.

Der Test prüft außerdem: Wenn der alte Kindprozess später endet, bleibt die
Sperre eines inzwischen neuen Besitzers erhalten. Kein Warten, Wiederholen
oder Aufweichen der ursprünglichen Busy-Prüfung wurde eingebaut.

Alle zehn gezielten Cachetests bestanden nach der Korrektur:

```sh
cargo test -p everlast-dictation --lib --features measurement,test-control model::cache::tests --locked
```

Die Korrektur ändert keine Modellgewichte oder Decoderparameter. Sie betrifft
die Lebensdauer des kooperativen Caches, nicht den Schutz gegen einen
böswilligen Schreiber mit denselben Benutzerrechten.
