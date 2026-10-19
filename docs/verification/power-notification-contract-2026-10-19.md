# Native Prozessbenachrichtigungen für Sleep/Wake

Der neue `power_notification_contract` registriert den produktiven
`PowerObserver` am echten prozesslokalen NSWorkspace-NotificationCenter.
Die drei öffentlichen Benachrichtigungen liefern exakt WillSleep, DidWake und
WillPowerOff auf dem Hauptthread. Nach zweimaligem remove sowie anschließendem
Drop treffen keine weiteren Ereignisse beim Empfänger ein. Der Test öffnet
kein Fenster und fordert weder Betriebssystemschlaf noch Herunterfahren an.

Eine kontrollierte Mutation von WillSleep zu DidWake schlug mit Exit 101 an
der exakten Ereignisfolge fehl; der Produktquelltext wurde bytegleich
wiederhergestellt. Anschließend bestanden der Test, gezieltes Clippy,
Scheduler-Tests, Guardrails und Vertragsprüfung. Unabhängiger Review ohne
Befunde. [Strukturierter Beleg](power-notification-contract-2026-10-19.json).

Grenzen: Hauptthread als Auslöser, keine isolierte Aussage über Registrierung
aus Workerthreads. Stille nach remove belegt die logische Abschaltung; Gate
und native Tokenentfernung werden nicht unabhängig voneinander isoliert.
Drop ohne vorheriges remove wird hier nicht separat geprüft. Echter
Sleep/Wake während Aufnahme und P03 bleiben offen.
