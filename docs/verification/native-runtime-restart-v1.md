# P03/P05: echte Langinferenz abbrechen und kurze Sitzung neu starten

Stand:17.10.2026. Testcode66d918a, Freeze2e578f9; Release-Testbinary5a9fcda8….
Originalhandle68803 terminalExit0, Probe-PID14676 beendet.
18:34:36.501721–18:35:00.564552UTC. Genau ein ausgewählter ignored-Test
lief tatsächlich:1pass/0fail/0ignored,23,71s Harnessdauer. Keine Diktatlatenz.
Keine parallelen Builds/Tests/Modelle; keine Wiederholung.

Der normale Runtime-Client bestätigte Recording UND Finalizing der neuen
Sitzung2, während der alte serielle Worker noch vor seinem echten Engineaufruf
gehalten war. Erst danach wurde er freigegeben. Genau zwei Decoderaufrufe:
4.800.000 Frames (300s-Control) und131.072 Frames (8,192s Privatfixture).
Beide echten Engineaufrufe mussten erfolgreich enden. Die widerrufene Sitzung1
konnte keinen Text liefern; ausschließlich Deliver2 mit dem aktuellen WorkToken
und anschließendem aktuellen Snapshot wurde angenommen. Der kurze Text stimmte
bytegenau mit der vorher gepinnten56Byte-ASR-Baseline überein, ohne Wortlautausgabe.
Expliziter Runtime-Join bestätigte freigegebenen Audio- und Decoderbesitzer.

Vorherige kontrollierte Restarttests, Workspace-Alltargets und striktes Clippy
in Default/measurement sowie Format/Diff/Guardrails/Contracts/Secrets waren grün;
unabhängiger kohärenter Review ohne Befund. Der eigene Runfreeze bindet14Quellen,
Binary undsechsEingaben einschließlich WAVHeadern/Framezahlen/Referenzhash,
vor dem Modellstart unabhängig gegen die immutable Quellrevision geprüft.
Der [inhaltfreie Bericht](native-runtime-restart-v1.json) bindet beide privaten
0600-Rohdateien und die tatsächlich bestandenen Vertragsassertionen.

Grenzen: kontrollierte Audio-/Native-Endpunkte, Modellinitialisierung im
Testthread vor Übergabe an den seriellen Worker. Die Haltebarriere liegt vor dem
inneren Decode; keine Aussage über native Kernelunterbrechung oder Zeitpunkt
eines Cancel mitten im Kernel. Kein300s-Echtzeiteingang, Mikrofon, reales Ziel,
RSS-/Leistungsnachweis oder Inhaltsqualifikation des langen Texts. Die kurze
Baseline beweist Wiederholbarkeit. Kein P05-/P06-/P03-Gesamtabschluss.
