# P05: kurzer Vergleich relativer und absoluter Warteaufrufe

Stand:17.10.2026. Codee129031, Freeze986ae42, Releasebinary00b8e84a….
EinzigerOriginalprozess16354/Handle42563 terminalExit0, keineparallelenBuilds/
Tests/Modelle.18:45:29.388600–18:46:34.054402UTC; Gesamtprozessdauer enthält
auch abschließende JSON-Veröffentlichung und ist keine Warte- oder Diktatlatenz.

Vorab feste Reihenfolge: relatives std::thread::sleep, danach öffentliche
MachWaiter-API; je30s mit1ms-Anforderungen und identischem Publisher-QoS.
Keine Audiozeitachse, keine20ms-Publikation, kein Spin und kein Modell.

| Mode | Vollständig erhaltene Calls | Größtes gemessenes Intervall | Größter Overshoot | Über10ms Overshoot |
|---|---:|---:|---:|---:|
|relativ|19.905|1.926.750ns|926.750ns|0|
|Mach absolut je Aufruf|20.066|2.163.375ns|1.163.375ns|0|

Beide Modes completed, alle39.971Calls erfolgreich und ohne frühe Rückkehr.
Dauer relativ30.001.085.834ns, Mach30.000.292.792ns. Beiderseits öffentlicher
QoS-Eintritt und explizites Ende bestätigt, keine API-/Cleanupfehler. Alle
Einzelintervalle bleiben in der privaten Ergebnisdatei erhalten, keine Ausreißer
entfernt; die streng-größer-10ms-Schwelle war vorab festgelegt.

Kein Mode reproduziert das alte12,736750ms-Schlafintervall. Daraus folgt weder
eine OS-Ursache noch ein Vorteil oder eine Reparatur durch den API-Wechsel.
Die feste Reihenfolge und kurzen Beobachtungsfenster erlauben keine allgemeine
Rangfolge. Die Mach-Deadline wird je Aufruf aus aktueller Machzeit plus Dauer
berechnet; dies ist kein an der Aufnahmezeitachse verankerter Replay-Publisher.
Der vorhandene Replay-Default ist unverändert. Kein erneuter großer Korpuslauf
allein auf Grundlage dieses ausreißerfreien Kurzversuchs.

Der [inhaltfreie Bericht](deadline-wait-v1.json) bindet alle drei privaten
0600-Rohdateien, Freeze/Parameter und beide Modes mit vollständigen Summaries
und kanonischen Hashes sämtlicher Calls. Root hat alle39.971 Indizes,
Anforderungen, Vorher-/Nachherzeiten, Intervall-/Overshootrechnungen,
Modegrenzen und Summaries nachgerechnet. Keine L-/P05-/P06-Qualifikation.
Weitere Hypothesen und Primärquellen stehen in
[replay-sleep-diagnosis-v1.md](replay-sleep-diagnosis-v1.md).
