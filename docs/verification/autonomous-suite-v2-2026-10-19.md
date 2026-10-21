## Anlass und neue Testgrenze

Zwei reale Läufe im unveränderten Entwicklerbuild `f592fcd` starteten und stoppten ohne physisches Kürzel. Beide erkannten den festen öffentlichen Satz exakt, hielten ihn aber wegen nicht bestätigter Editierbarkeit des Startfelds zurück. Im zweiten Lauf blieb der eigene Empfänger über alle 18 protokollierten Beobachtungen vorn. Das beweist keinen erfolgreichen Einfügevorgang. Die Rohbelege liegen unter `artifacts/test-control-v1/autonomous-live-20261019/`.

## Technischer Umbau

- Ein normaler Lauf prüft Rust-Libraries und Transport-Binary, UI-Verträge und sämtliche Node-Testwerkzeuge mit bis zu drei gleichzeitigen Gruppen.
- Das Vollprofil ergänzt statische Checks, Python-Qualitätswerkzeuge, Core-Integrationsverträge und eine ausdrückliche Liste unsichtbarer nativer Verträge. Der sichtbare Fenster-Lifecycle ist nur über `test:os:windows` erreichbar.
- Cargo-Gruppen laufen exklusiv. Nach einem Fehler starten keine weiteren Gruppen; schon aktive Gruppen enden kontrolliert. Null passende Rust-Tests, Timeout und fehlgeschlagene Prozessbereinigung werden nicht als Erfolg gewertet.
- Der normale Testbefehl akzeptiert keinen Fokusbericht mehr. Ein solcher Bericht gehört ausschließlich zum getrennten `test:os:readiness`.
- Vier neue Szenarien verbinden den tatsächlichen Runtime-Actor mit dem tatsächlichen NativeService-Owner. Nur Decoder und OS-Effekte sind kontrolliert. Getestet werden einmaliges Einfügen mit Cleanup, fehlender Startprozess, später unbrauchbares Ziel und Abbruch während Dekodierung.

## Wirksamkeit und unabhängige Prüfung

Die sieben gezielten Runtime-Verbundtests bestanden. Ein kontrollierter Mutationsversuch entfernte zeitweilig den echten `backend.send`-Aufruf: Der Erfolgsfall schlug dann fehl, obwohl ein oberflächliches Runtime-Ergebnis weiterhin `Sent` melden konnte. Anschließend wurde der Originalcode wiederhergestellt und alle sieben Tests bestanden erneut. Rohlogs: `artifacts/autonomous-suite-v2/joined-restored.log` und `mutation-skip-native-send.log`.

Der Parallelitätstest war vor der Scheduler-Änderung rot und ist danach grün. Er verlangt gleichzeitig laufende unabhängige Kinder und weist getrennt nach, dass zwei Cargo-Gruppen nicht überlappen. Fehlerdrain, Signalabbruch und Kindprozessbereinigung werden eigenständig geprüft.

Der abschließende Vollprofil-Lauf bestand in 51,568 Sekunden: 431 Rust-, 215 UI-, 41 Node- und 204 Python-Qualitätswerkzeugtests sowie statische Prüfungen. Er lief ohne Nutzeraktion, Aufnahme oder sichtbare Fenster. Lauf und Dateihashes stehen im [JSON-Beleg](autonomous-suite-v2-2026-10-19.json). Ein grüner Softwarelauf ersetzt weder reale OS-Abnahme noch ASR-Qualitäts-/Leistungsnachweise und schließt nicht automatisch alle Arbeitspakete.
