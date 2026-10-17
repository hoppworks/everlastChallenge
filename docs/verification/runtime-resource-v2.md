# P05:100 Lifecyclezyklen mit korrigiertem Produktionsdecoder

Stand:17.10.2026. Code736242d, Freeze538c07c, Release-Resourcebinary30f41f9a….
no_timestamps=false, übrige Runtimeparameter unverändert. Vorherige Einzelinferenz
bestätigte die vorhandene private UTF-8-Baseline bytegenau; Wiederholbarkeit,
keine unabhängig gehörte Transkriptwahrheit. Kein App-/DMG-Neubau.

Ursprünglicher Launcher4017 terminalExit0; Hostprotokoll vor Kindstart
17:33:50.832815 UTC, Completion17:44:59.037345 UTC. Tatsächlicher time-Elternlauf
668,16s; keine Diktatlatenz. Launcher-/effektiver Exit0, keine Lade-, Diagnose-,
Berichts-, Cleanup- oder Metrikfehler. Fünf Warmups und alle100 Messzyklen statusok.

| Prüfung | Tatsächlicher Nachweis |
|---|---:|
| Messzyklen Stop / Capture-Cancel / Decoder-Entry-Cancel | 34 / 33 / 33 |
| Ganze Decoder-Eingaben exakt wie Fixture | 72 / 72 |
| Stop-Texte exakt wie Baseline, einschließlich Warmups | 39 / 39 |
| Abgebrochene Sitzungen ohne übernommenen Text | 66 / 66 |
| Freigabe durch nachfolgenden Start bestätigt | 104 / 104 |
| Publikationen | 29553 |
| Maximale Publikationsabweichung | 3406083ns |
| RSS-/vmmap-Checkpoints | 101 / 2 |

Alle105 Zeilen enden mit bestätigtem Controller-/Wrapper-Idle. Letzter Lauf ist
StopComplete; anschließend echter Runtime-Join bestätigt (3689µs beobachtet).
Danach Decoder nicht aktiv/armed,72 Aufrufe gestartet und beendet, kein
Samplemismatch oder Decoderfehler. Alle Einspeisezusammenfassungen innerhalb
der vorherigen10ms-Toleranz. Das Ressourcenwerkzeug hält Zusammenfassungen,
keine vollständigen L-Rohdatensätze oder Stop-/Textzeitpunkte fest.

| Residenter Prozessspeicher | Byte |
|---|---:|
| Nach fünf Warmups | 732102656 |
| Minimum aller101 Punkte | 667140096 |
| Maximum aller101 Punkte | 734806016 |
| Nach Messzyklus20 | 669597696 |
| Nach Messzyklus100 | 669794304 |
| Letzter Wert minus Warmupbaseline | −62308352 |
| Zyklus100 minus Zyklus20 | +196608 |

Kein über den gesamten Durchlauf fortgesetzter Anstieg. Der frühe Rückgang
und die späteren kleinen Zunahmen bleiben vollständig ausgewiesen; keine
Ursachenzuordnung zu Metal, Allocator oder Anwendung aus RSS allein. Der
Messprozess hält auch begrenzte Zeilen/Checkpoints, eine zusätzliche konstante
Originalfixture sowie262222B WAV,524288B normalisierte PCM,56B Baseline und
13100B Parametertext. RSS ist kein Live-Heapmaß. Kein erfundener Schwellwert
oder universeller Leakausschluss; resourceQualified bleibt ausdrücklich false.

Decoder-Entry-Cancel hält den Wrapper vor dem echten Engineaufruf an, widerruft
die Sitzung und lässt danach den nativen Aufruf enden. Das belegt Verwerfen,
Besitzfreigabe und Neustart, keine Kernelunterbrechung. Initialisierung erfolgt
im Entwicklerhauptthread vor Übergabe an den seriellen Worker. Eingebettete
Paced-/QoS-Parameter sind Provenienz; dieses Ressourcenprogramm führt weder
das zentrale L-Protokoll noch dessen Publisher-QoS-Override aus.

Der [inhaltfreie Bericht](runtime-resource-v2.json) bindet11 private600-Dateien,
30 Quellpins gegen immutable736242d, alle105 Zeilen und101 RSS-Punkte samt
Hashes der beiden privaten VM-Zusammenfassungen. Keine Nutzerdiktate ausgegeben.
Keine P05-/P06-, Mikrofon-, Hotkey-, native Übergabe- oder installierte Abnahme.
