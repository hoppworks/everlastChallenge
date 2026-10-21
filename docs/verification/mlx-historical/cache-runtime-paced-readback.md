# Unabhängiger Readback: sechsarmiger Cache-/Speed-Vergleich

**Alle unabhängigen aktuellen Pace-Prüfungen bestanden.** Beide Paarreihenfolgen sind vollständig; keine offenen Findings in diesem Readback. `allIndependentChecksPassed`, `allChecksPassed` und `bothPairOrdersComplete` gelten für den unten beschriebenen begrenzten Prüfbereich. `qualified` bleibt **false** für das Gesamtprodukt. Keine neuen Modelle, GPU-Jobs, Builds oder Messungen.

[Vollständige Berechnungen, Rohwertlisten und Bindungen](cache-runtime-paced-readback.json) · ausgeführter unabhängiger Prüfcode (historischer oder lokaler Beleg, nicht mitgeliefert)

Der neue feste Lauf ist CPP → Cache256 → altes unbounded FP16 → altes unbounded FP16 → Cache256 → CPP. Alle sechs Arme enthalten exakt181 vollständige Slots (ein markierter Warmup plus30 Messungen je sechs Fällen). Keine Teilmenge, kein Messfehler und kein langsameres Cache-Ergebnis wurde entfernt. Dies ist der neue sechsarmige Nachweis; frühere fehlgeschlagene Arme werden nicht umetikettiert oder repariert.

| Arm | L (s) | Load (ms) | maximale RSS (Byte) | Peak-Footprint (Byte) |
|---|---:|---:|---:|---:|
| 01-cpp | 1.060281708250 | 6090 | 804667392 | 812484144 |
| 02-cache256 | 0.355929656083 | 6796 | 1389445120 | 3179645736 |
| 03-unbounded | 0.356195923667 | 6890 | 1478000640 | 3672001560 |
| 04-unbounded | 0.355005791583 | 6728 | 1535508480 | 3677195216 |
| 05-cache256 | 0.357764222250 | 6831 | 1389117440 | 3179104776 |
| 06-cpp | 1.057249871500 | 1312 | 801554432 | 809354752 |

Load sind beobachtete Prozessladezeiten mit vorhandenen OS-Caches; kein Coldbootbeweis. Die beiden OS-Speicherwerte stammen jeweils aus der tatsächlichen Schlussausgabe von `/usr/bin/time -l` und sind ganze Prozessmaxima.

| Vergleich, zeitliche Reihenfolge | Referenz-L (s) | Cache-L (s) | L-Zeitänderung Cache |
|---|---:|---:|---:|
| CPP01 → Cache02 | 1.060281708250 | 0.355929656083 | -66.430652032 % |
| Cache05 → CPP06 | 1.057249871500 | 0.357764222250 | -66.160863965 % |
| Cache02 → unbounded03 | 0.356195923667 | 0.355929656083 | -0.074753125 % |
| unbounded04 → Cache05 | 0.355005791583 | 0.357764222250 | +0.777010047 % |

Beide Cache-vs-CPP-Ordnungen verbessern **alle sechs Mediane und alle sechs p95**; jeder Cache-Fall erfüllt Median ≤1s und p95 ≤1,5s. Die CPP-Baselines überschreiten ihrerseits den1s-Medianwert; dies wird beibehalten. Gegen altes FP16 sind Verschlechterungen zulässig und unten sichtbar. Keine statistische Gleichheit und kein allgemeiner Null-Slowdown ist nachgewiesen.

Alle Fallstatistiken in **Millisekunden**; p95 ist der29. sortierte Wert von30. Die Spanne ist Max−Min. Die JSON hält sämtliche30 Werte je Fall und Arm in ursprünglicher Reihenfolge.

| Arm | Fall | Median | p95 | Min | Max | Spanne |
|---|---|---:|---:|---:|---:|---:|
| 01-cpp | fleurs-de-test-1736 | 1025.255459 | 1032.206209 | 998.857416 | 1032.990875 | 34.133459 |
| 01-cpp | fleurs-de-test-1663 | 1088.593312 | 1100.395000 | 1075.124542 | 1102.526083 | 27.401541 |
| 01-cpp | fleurs-de-test-1668 | 1102.107792 | 1111.172750 | 1076.492417 | 1212.980500 | 136.488083 |
| 01-cpp | fleurs-en-test-1679 | 1020.075812 | 1024.493625 | 990.701958 | 1027.898708 | 37.196750 |
| 01-cpp | fleurs-en-test-1676 | 1041.700312 | 1050.916666 | 1013.875416 | 1054.514208 | 40.638792 |
| 01-cpp | fleurs-en-test-1667 | 1083.957562 | 1093.351291 | 1074.907250 | 1094.071875 | 19.164625 |
| 02-cache256 | fleurs-de-test-1736 | 323.082667 | 337.325333 | 304.850250 | 337.674167 | 32.823917 |
| 02-cache256 | fleurs-de-test-1663 | 388.592146 | 401.696709 | 371.119667 | 410.380958 | 39.261291 |
| 02-cache256 | fleurs-de-test-1668 | 396.732562 | 409.262166 | 385.927583 | 412.745334 | 26.817751 |
| 02-cache256 | fleurs-en-test-1679 | 311.264604 | 322.890333 | 297.006250 | 323.592958 | 26.586708 |
| 02-cache256 | fleurs-en-test-1676 | 337.838812 | 350.881042 | 314.450375 | 351.034208 | 36.583833 |
| 02-cache256 | fleurs-en-test-1667 | 378.067145 | 412.638709 | 349.004708 | 498.631417 | 149.626709 |
| 03-unbounded | fleurs-de-test-1736 | 331.831834 | 351.022417 | 314.423792 | 382.912916 | 68.489124 |
| 03-unbounded | fleurs-de-test-1663 | 381.377250 | 400.527500 | 356.445334 | 449.082083 | 92.636749 |
| 03-unbounded | fleurs-de-test-1668 | 395.386625 | 410.173584 | 382.502667 | 410.533250 | 28.030583 |
| 03-unbounded | fleurs-en-test-1679 | 316.227688 | 327.269875 | 293.197000 | 337.358250 | 44.161250 |
| 03-unbounded | fleurs-en-test-1676 | 336.221896 | 349.977334 | 325.595875 | 355.705625 | 30.109750 |
| 03-unbounded | fleurs-en-test-1667 | 376.130250 | 393.064750 | 362.305292 | 400.738750 | 38.433458 |
| 04-unbounded | fleurs-de-test-1736 | 326.090750 | 340.401083 | 306.039250 | 343.501166 | 37.461916 |
| 04-unbounded | fleurs-de-test-1663 | 383.292125 | 398.053584 | 358.150625 | 399.487959 | 41.337334 |
| 04-unbounded | fleurs-de-test-1668 | 393.941937 | 405.935000 | 384.950625 | 409.600000 | 24.649375 |
| 04-unbounded | fleurs-en-test-1679 | 313.929374 | 321.382292 | 299.096458 | 322.361792 | 23.265334 |
| 04-unbounded | fleurs-en-test-1676 | 336.357646 | 350.998875 | 315.658292 | 385.419709 | 69.761417 |
| 04-unbounded | fleurs-en-test-1667 | 376.422916 | 385.089250 | 359.980459 | 387.637416 | 27.656957 |
| 05-cache256 | fleurs-de-test-1736 | 325.757271 | 342.869292 | 311.966000 | 362.500291 | 50.534291 |
| 05-cache256 | fleurs-de-test-1663 | 385.366751 | 402.575250 | 361.238167 | 417.361458 | 56.123291 |
| 05-cache256 | fleurs-de-test-1668 | 403.425292 | 426.949375 | 383.454417 | 449.441875 | 65.987458 |
| 05-cache256 | fleurs-en-test-1679 | 317.219770 | 326.699000 | 300.975791 | 327.280625 | 26.304834 |
| 05-cache256 | fleurs-en-test-1676 | 337.967125 | 353.455166 | 308.331041 | 356.272417 | 47.941376 |
| 05-cache256 | fleurs-en-test-1667 | 376.849125 | 386.721916 | 363.878041 | 387.664666 | 23.786625 |
| 06-cpp | fleurs-de-test-1736 | 1024.965208 | 1035.283000 | 989.345958 | 1037.591750 | 48.245792 |
| 06-cpp | fleurs-de-test-1663 | 1087.350812 | 1099.674084 | 1067.471458 | 1100.847000 | 33.375542 |
| 06-cpp | fleurs-de-test-1668 | 1100.786959 | 1111.035708 | 1070.968666 | 1113.282500 | 42.313834 |
| 06-cpp | fleurs-en-test-1679 | 1014.245646 | 1023.314667 | 990.743375 | 1024.147000 | 33.403625 |
| 06-cpp | fleurs-en-test-1676 | 1038.561334 | 1051.912666 | 1012.705959 | 1053.349375 | 40.643416 |
| 06-cpp | fleurs-en-test-1667 | 1077.589271 | 1088.077333 | 1070.778958 | 1095.143709 | 24.364751 |

Einzelfalländerungen Cache gegenüber altem FP16; **positiv bedeutet langsamer**. Kein Gesamtmittel verdeckt diese Richtungen.

| Fall | Cache02 vs03 Median | p95 | Cache05 vs04 Median | p95 |
|---|---:|---:|---:|---:|
| fleurs-de-test-1736 | -2.636627 % | -3.902054 % | -0.102266 % | +0.725088 % |
| fleurs-de-test-1663 | +1.891800 % | +0.291917 % | +0.541265 % | +1.135944 % |
| fleurs-de-test-1668 | +0.340410 % | -0.222203 % | +2.407297 % | +5.176783 % |
| fleurs-en-test-1679 | -1.569465 % | -1.338205 % | +1.048133 % | +1.654325 % |
| fleurs-en-test-1676 | +0.480907 % | +0.258219 % | +0.478502 % | +0.699800 % |
| fleurs-en-test-1667 | +0.514953 % | +4.979831 % | +0.113226 % | +0.423971 % |

1086 Rohobjekte geprüft (1080 gemessen +6 Warmup), 541500 einzelne Veröffentlichungen, 5430 TC-Snapshots, alle Controller-/Capture-Uhren, vollständige Feed-/Stop-Deadlines, Textterminalzeiten und RTF. Für jede Veröffentlichung gelten die unveränderten20ms-Chunks und10ms-Budgets. Die Timingdefinition und alle gemeinsamen CPP-Decoding-/Vorverarbeitungsparameter sind über alle Profile unverändert.

966 vollständige Objekte sind identisch zum aktuellen ursprünglichen CPP100-Oracle; 120 sind ausschließlich exakt SHA-gebundene bereits genehmigte ganze Paare. Alle180 vollständigen sechsfalligen Qualitätsrunden wurden mit unverändertem Unicode13-/WER-Oracle nachgerechnet; kein positiver Gruppen-WER-Delta und keine neue ungeprüfte Inhaltsänderung. Jede MLX-Ausgabe entspricht außerdem exakt dem aktuellen akzeptierten Quality100-FP16-Objekt.

Der tatsächliche Backend-Nachweis bestätigt CPP ausschließlich in01/06; in02/03/04/05 jeweils einen FP16-Load mit gebundenem F32-Checkpoint/F32-Brücke zum unveränderten CPP-Decoder und181 externe/0 native Encoderfenster. Nur02/05 enthalten genau einen installierten268435456-Byte-Worker-Cache-Receipt. Das ist ein wiederverwendbarer Cachebudget-Schnitt, keine Gesamt-RAM-Grenze.

Alle7093 geschützten Dateizeugen wurden vor/nach dem Readback geprüft; alle zugelassenen kleinen Inhalte erneut gehasht. Beide ursprünglichen Build-/Sourcearchive, eigenen Binaryzuordnungen, Parameterdokumente, Modell-/Kerneldefinitionen, Fixtures und Kontrollerkopien bleiben getrennt gebunden. Nur die sechs vorab zugelassenen Produktionsdateien für Cachepolicy unterscheiden sich zwischen altem und neuem Build. Große Modelle/Binaries/Kernel wurden nicht erneut gehasht; sie behalten die in der tatsächlichen Vorbereitung verifizierten Hash-/Dateizeugen. Aktuelles Quality100, frische12 und ihre vollständigen unabhängigen Acceptance-Readbacks sind unverändert gebunden und lagen vor Messstart vor.

2269 Powerbeobachtungen: AC und byteidentische Policy in allen Before-/Sample-/Post-Reap-Rollen. Maximale beobachtete Samplinglücke 5.149485s. Der nächste Arm startet erst nach bestätigtem Reap des vorherigen. Alle sechs Owner sind Exit0/reaped/terminationConfirmed, ohne ForcedCleanup, Watchdog, Cancel oder Power-/Cleanupfehler. Alle sechs PGIDs wurden im Readback erneut mit Signal0 geprüft und sind ESRCH-abwesend. Das gebundene Controller-`finally` wartet auf `caffeinate`; `caffeinateReaped=true` bleibt dokumentiert. Dessen PID wird nicht separat exportiert. Diskrete Samples beweisen keine kontinuierlich konstanten thermischen/physikalischen Lastbedingungen; CPUzeiten beweisen keinen Energie- oder Joulegewinn.

Die RAM-Abwägung bezieht sich zusätzlich exakt auf den bereits unabhängig geprüften [Runtime-RAM-Readback](cache-runtime-readback.md), dessen Rohbindungen hier erneut bestätigt wurden: warmcheckpoint-RSS −552943616 Byte (−39,108871%), warmes Cachemedian −1070603348 Byte; whole-process Peak-Footprint −298451304 Byte (rund8,6%). Das sind andere Phasen/Workloads als die sechs Pace-Arme. In diesen neuen Pace-Armen liegen die alten FP16-Peak-Footprints bei3,672/3,677GB und die Cache-Peaks bei3,180/3,179GB. Diese ganzen Prozessmaxima sind keine isolierten Warmphasensamples; überlappende Einsparungen werden nicht addiert.

Der historische RAM-v1-`limits`-Satz ist weiterhin sachlich falsch und als P2-Wording-Finding dokumentiert. Seine198 tatsächlichen Execution-/Speicherprüfungen gelten, sein `allChecksPassed=false` wird nicht umgeschrieben. Der neue V2-Build korrigiert die Beschreibung; daraus wird keine nachträgliche Rohdatenkorrektur oder zusätzliche RAM-Messung.

Der begrenzte Nachweis trägt die RAM-/Qualitäts-/Pace-Abwägung dieses Kandidaten. Rest94, resource100, Bundle/Offline/Default und die Gesamtproduktabnahme bleiben separat offen; keine volle Produktqualifikation.
