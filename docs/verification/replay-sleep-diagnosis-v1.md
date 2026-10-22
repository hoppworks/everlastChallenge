# P05: begrenzte Diagnose des Schlafintervall-Ausreißers

Ausgangspunkt ist der erhaltene zentrale v3-Lauf: ein1ms-Schlafintervall misst
12,736750ms, danach ist die Publikation10,234042ms zu spät. Der eigentliche
Publish dauert2083ns; kein finaler Spin wurde in diesem Wait betreten.
Phasenevidenz und Grenzen.

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

## Nächster begrenzter Versuch: Schedulingklasse des Audio-Publishers

Der [vollständige kurze Wartevergleich](deadline-wait-v1.md) reproduziert den
seltenen Ausreißer nicht. Er belegt keinen Vorteil eines API-Wechsels. Deshalb
bleibt im nächsten Versuch der vorhandene relative Schlafpfad unverändert.
Verglichen werden dieselben 480.000 PCM16-Monoframes bei 16 kHz, zuerst mit
USER_INTERACTIVE-QoS, danach mit THREAD_TIME_CONSTRAINT_POLICY. Je Mode gibt es
einen eigenen Publisherthread und genau einen 30-Sekunden-Durchlauf. Die
20-ms-Publikationen, 10-ms-Toleranz, maximal 1 ms Schlaf und 250 µs Finalspin
bleiben gleich; kein Modell und kein Mikrofon werden verwendet.

Die feste TC-Anforderung beträgt 20 ms Periode, 500 µs Rechenbudget und 1 ms
Constraint, preemptible. Der sichere Adapter rechnet die öffentlichen Tickfelder
mit geprüfter Aufrundung um. Er setzt die Policy ausschließlich im neu erzeugten
Childthread und liest sie zurück. Der normale Parent startet Capture/DSP erst
nach dessen Ready-Bestätigung; diese Worker sollen die Publisherpolicy nicht
erben. Das Threadende mit beobachtetem Join begrenzt die Policy-Lebensdauer;
ein Wiederherstellen einer unbekannten ursprünglichen QoS wird nicht versucht.

Gespeicherte TC-Felder allein beweisen keine aktive Schedulingklasse. Deshalb
werden zusätzlich der rohe THREAD_BASIC_INFO-Policywert und Thread-CPU-Zeiten
vor/nach Einrichtung, vor/nach Feed und unmittelbar nach jeder ungültigen
Publikation erfasst. Die Readbacks bestehen aus mehreren aufeinanderfolgenden
OS-Abfragen und sind keine atomare oder kontinuierliche Demotionsbeobachtung.
Ein Basicwert von 2 ist nur mit der angeforderten Klasse vereinbar, kein eigener
RT-Beweis; ein beobachteter Timesharewert, eine Defaultpolicy oder abweichende
TC-Felder macht den entsprechenden TC-Versuchsaufbau ungültig. Die Baseline
wird nach beendetem QoS nicht gegen eine TC-Anforderung geprüft.

Alle Publikationen, Fehler, Stop-Zeitpunkte, Policy-/CPU-Readbacks und
Aufräumergebnisse bleiben erhalten. Beobachtungsaufwand bei Ausreißern darf
folgende Publikationen beeinflussen und wird nicht aus der Zeitachse entfernt.
Stop und Portfreigabe erfolgen im Publisher vor der Nach-Feed-Abfrage; der
Parent prüft anschließend vollständige Samplegleichheit und Audioowner-Join.
Ein rein zeitlich ungültiger Mode sperrt den zweiten Mode bei bestätigtem
Aufräumen nicht; OS-, Policy-, Beobachtungs-, Audio- oder Joinfehler schon.

Releasebinary, kompilierte Parameter, WAV und vollständige Quellen werden vor
dem echten Versuch gebunden. Bis dahin sind nur kontrollierte Tests ausgeführt.
Auch zwei saubere Kurzläufe wären weder eine allgemeine RT-Garantie noch eine
Qualifikation von P05/P06 oder ein Stop-bis-Text-Messwert.
