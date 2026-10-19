# Native Testfeld: Startfehler behoben

Zwei echte Starts über die UI-Steuerung endeten mit Exitcode 101. Die begrenzte Diagnose zeigte einen Panic in objc2: Reporter-Ivars mit 16-Byte-Ausrichtung werden nicht unterstützt. Die neue Berichtsstruktur enthält `u128`; `RefCell<Reporting>` verletzt damit die Grenze des Objective-C-Instanzlayouts.

`RefCell<Box<Reporting>>` hält nur pointer-ausgerichtete Daten in der Instanz. Rust richtet den Heapwert einschließlich `u128` aus und gibt ihn weiterhin mit dem Reporter frei. Der gezielte Regressionstest prüft die tatsächlich deklarierte Ivars-Struktur: vor dem Fix rot, danach grün. Alle 14 Beispieltests, Build und striktes Clippy bestanden. Unabhängiger Review: keine Befunde. Der temporäre Diagnose-Hook wurde vollständig entfernt.

Der erneute echte UI-Start zeigt jetzt das Testfenster und sein editierbares mehrzeiliges Feld. Der eigene passive Bericht bleibt aktiv und erfasst leere Textfelder. Das behebt den Startfehler, beweist aber noch keine Bereitschaft: Nach dem Hintergrundstart bleiben `applicationActive`, `frontmostIsSelf` und `windowKey` falsch. Lokaler AX-Fokus allein reicht weiterhin ausdrücklich nicht. Keine Startaufforderung, physische Geste oder Aufnahme wurde ausgelöst.

[Hashes, Regression und genaue Beobachtungen](native-recipient-alignment-2026-10-19.json).
