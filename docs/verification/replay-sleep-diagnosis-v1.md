# P05: begrenzte Diagnose des Schlafintervall-Ausreißers

Ausgangspunkt ist der erhaltene zentrale v3-Lauf: ein1ms-Schlafintervall misst
12,736750ms, danach ist die Publikation10,234042ms zu spät. Der eigentliche
Publish dauert2083ns; kein finaler Spin wurde in diesem Wait betreten.
[Phasenevidenz und Grenzen](paced-six-reference-v3.md).

Die Aussage betrifft das gemessene Intervall um den Aufruf. Es kann auch
Descheduling unmittelbar vor/nach dem eigentlichen Kernelwarten enthalten;
die Rohdaten beweisen keine bestimmte Scheduling- oder Coalescing-Ursache.

## Vor dem Versuch festgelegte Hypothesen

1. Der relative Schlafpfad einschließlich seiner Wiederaufnahme trägt zum
   langen Intervall bei. Vorhersage: ein isolierter absoluter Mach-Pfad ändert
   gemessene Rückkehrintervalle bei sonst gleichen Requests/QoS.
2. Der Thread wird trotz öffentlichem QoS verspätet wieder ausgeführt.
   Vorhersage: auch ein absoluter Wait kann entsprechende Ausreißer zeigen.
3. Eine größere Unterbrechung direkt um den Wait liegt im Intervall.
   Vorhersage: ein API-Wechsel allein verhindert dieses Muster nicht zwingend.

Die [offizielle Rust1.93.0-Quelle](https://raw.githubusercontent.com/rust-lang/rust/1.93.0/library/std/src/sys/thread/unix.rs)
führt diesen Unix-/Apple-Schlafpfad über relatives libc::nanosleep aus.
Apples archivierte [TN2169](https://developer.apple.com/library/archive/technotes/tn2169/_index.html)
beschreibt mach_wait_until als damals gemessenen Weg mit geringerem Overhead;
sie erklärt zugleich, dass Timer letztlich denselben Kernelmechanismus nutzen.
Das ist eine begründete Mechanismusalternative, kein garantierter Fix und
keine Behauptung über die konkrete Ursache dieses einen Ausreißers.

THREAD_TIME_CONSTRAINT_POLICY ist eine separate größere Alternative mit
Rechenbudget und eigener Thread-Lebensdauer-/Restorefrage; sie wird in diesem
Versuch nicht gesetzt. Öffentliche Grundlage:
[Apple Scheduling-Dokumentation](https://developer.apple.com/library/archive/documentation/Darwin/Conceptual/KernelProgramming/scheduler/scheduler.html).

## Korrektheits- und Feedbackgrenze

Der sichere native Adapter prüft die einmal abgefragte Timebase, rundet auf
volle absolute Ticks auf, prüft Dauer-/Deadlineüberlauf und gibt OS-Fehler
explizit zurück. Eine frühe erfolgreiche Rückkehr wird gegen dieselbe absolute
Deadline geprüft; es gibt keinen stillen relativen Fallback. Deterministische
Backendtests erfassen diese Verträge einschließlich Fehlern und Nullzeit.

Der kurze Entwicklervergleich plant genau zwei30s-Blöcke vor: relatives
std::thread::sleep und absolutes Mach-Warten, jeweils1ms-Anforderungen mit
demselben öffentlichen Publisher-QoS. Sämtliche Vorher-/Nachherintervalle und
Overshoots bleiben erhalten, kein I/O im Messloop. Beide Datensätze werden nach
Ende des QoS-Scope privat veröffentlicht; Fehler und nicht ausgeführte Modes
bleiben sichtbar. Schwelle ist10ms Overshoot, keine nachträgliche Auswahl.

Das ist ein enger reproduzierbarer Aufrufweg für die konkrete lange
Schlafintervall-Symptomatik. Ein seltener OS-Ausreißer ist im Kurzlauf nicht
deterministisch erzwingbar. Ausreißerfreiheit wäre deshalb keine Bestätigung
der Ursache oder des vollständigen Echtzeitaufbaus. Es gibt hier weder
Audioeingang noch20ms-Publikationen oder250µs-Finalspin; diese unveränderten
Pipelinegrenzen brauchen später ihre eigene vollständige Prüfung.
