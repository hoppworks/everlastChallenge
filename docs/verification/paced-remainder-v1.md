# Restkorpus v1: vollständiger struktureller Messnachweis

**94/94 Fälle und 1.034/1.034 Zeilen erfolgreich:** je ein Warmup und zehn Messungen, insgesamt **911.559 Publikationen** ohne frühe oder verspätete Publikation beim unveränderten 10-ms-Budget. 94 Runtime-Cleanups und alle 1.034 Publisher-Joins sind bestätigt. Keine erneuten Läufe und keine gefilterte Erfolgsmenge.

Quelle `b9799b673f7623f7e774b61a599527f6ab251bba`, Freeze-Commit `1f84d683f66b873f83a1956c1e503c445184d4b3`, Freeze-SHA256 `b09d4dd6beda9a522862ed73db4a1e334ed1d19e15e2a7b0c288dc1a3473c586`. Binary, Parameter, 37 Freeze-Source-Dateien (einschließlich aller 27 kompilierten Sourcepin-Einträge), Modell/VAD, Auswahl, alle 94 PCM16-Mono-16-kHz-WAVs und unabhängigen Referenzidentitäten wurden vollständig geprüft. Getrackte Pins wurden zusätzlich gegen unveränderliche Git-Revisionen geprüft. Alle gebundenen Rohdateien, der private Freeze und das Referenzmanifest bleiben unverändert und 0600 erhalten.

Die Originalprozesse liefen seriell von `2026-10-17T20:26:13.571959+00:00` bis `2026-10-18T01:56:02.926449+00:00`. Jeder Fall hat einen eigenen Prozess und einmal geladenen warmen Kontext für seine elf Sitzungen. 94 unterschiedliche Probe-PIDs und 94 Wrapper-PIDs sind getrennt gebunden. Alle Exitcodes sind 0; kein Watchdog-/Launchfehler. Die aufgezeichneten Hostbeobachtungen stimmen überein (Apple M5, 32 GiB, macOS 27.0.1/26A434, AC Power). Das ist keine fortlaufende Hintergrundlast-/Stromversorgungskontrolle.

Erwartete Outcomes waren vorab explizit festgelegt: **968 Local- und 66 Empty-Zeilen** (88 Local- und sechs Empty-Fälle). Empty behält t_text=None; seine Terminalzeit wird niemals als Textlatenz oder L ausgegeben. Alle 1.034 Läufe besitzen tatsächliche Engine-Dauer mit exakt der Fixture-Framezahl und konsistenter RTF. Auch eine frühe Energy-/VAD-Ablehnung innerhalb des Engine-Aufrufs gehört zu dieser Dauer.

Alle Framebereiche einschließlich letzter Teilchunks, absoluten Endframe-Deadlines, Before-/After-Publikationszeiten, Abweichungen und Stop-/Terminalgleichungen wurden unabhängig nachgerechnet. Größte Publikationsabweichung **87.000 ns**, größter Publikationskörper **86.666 ns**, größte Stop-Abweichung vom geplanten Ende **29.291 ns**. Alle Stops liegen nach der letzten vollständigen Publikation. Vollständige Frameabdeckung belegt keine zusätzlich gemessene Sample-für-Sample-Gleichheit jedes Durchlaufs.

Alle **4.136 Post-Setup-Snapshots** (configured, beforeFeed, afterFeed, after) melden exakt period/computation/constraint 480000/12000/24000 Ticks, preemptible=true, defaultPolicy=false und Basic.policy=2. Vor Setup war Basic.policy=1/defaultPolicy=true. Kein Policyfehler und kein Ausreißersnapshot. Das belegt die abgefragten Zeitpunkte, keine durchgehend wirksame RT-Klasse oder Durchsetzung jedes periodischen CPU-Budgets. Publisher-Feed-CPU (User+System): **413786981000 ns** inklusive Warmups; **376283001000 ns** nur für 940 Messungen.

Die folgenden Mediane und Spannweiten verwenden ausschließlich die zehn Messläufe pro Fall; jeder Warmup steht separat im JSON. RTF = tatsächliche Decoderaufrufdauer / übergebene 16-kHz-PCM-Dauer, einschließlich Engine-Energieprüfung/VAD/Vorbereitung/ASR. Sie ist weder reine ASR-Kernelzeit noch Stop→Text-Zeit. Bei sehr kurzen Clips kann der fixe Aufwand RTF über 1 erzeugen: Maximum aller Messläufe 2,095145580; deshalb wird keine pauschale RTF<1-Aussage über alle Längen gemacht.

| Fall | Erwartung | Dauer (s) | Stop→Terminal Median / Spanne (ms) | Engine-RTF Median |
|---|---|---:|---:|---:|
| fleurs-de-test-1660 | Local | 10.380000 | 1083.828395 / 19.397291 | 0.102291219 |
| fleurs-de-test-1661 | Local | 6.660000 | 1058.866542 / 13.876625 | 0.155652365 |
| fleurs-de-test-1662 | Local | 23.460000 | 1088.556562 / 16.575292 | 0.045472381 |
| fleurs-de-test-1664 | Local | 8.700000 | 1055.118084 / 17.486917 | 0.119243645 |
| fleurs-de-test-1665 | Local | 10.680000 | 1107.994229 / 15.213042 | 0.102050646 |
| fleurs-de-test-1666 | Local | 17.760000 | 1086.393646 / 28.163291 | 0.060015602 |
| fleurs-de-test-1667 | Local | 27.660000 | 1140.361854 / 16.264042 | 0.040425063 |
| fleurs-de-test-1669 | Local | 12.780000 | 1079.342875 / 40.411750 | 0.083076700 |
| fleurs-de-test-1670 | Local | 21.120000 | 1196.353353 / 30.405667 | 0.055556060 |
| fleurs-de-test-1671 | Local | 28.740000 | 1126.468250 / 30.915083 | 0.038615205 |
| fleurs-de-test-1672 | Local | 10.500000 | 1082.697042 / 21.460250 | 0.101293064 |
| fleurs-de-test-1673 | Local | 12.600000 | 1088.824729 / 34.716875 | 0.085180268 |
| fleurs-de-test-1674 | Local | 12.060000 | 1068.113709 / 15.827375 | 0.086717214 |
| fleurs-de-test-1675 | Local | 15.840000 | 1075.176897 / 44.178750 | 0.066718074 |
| fleurs-de-test-1676 | Local | 6.060000 | 1051.655125 / 18.576417 | 0.170410764 |
| fleurs-de-test-1677 | Local | 5.400000 | 1051.211708 / 10.618292 | 0.190972434 |
| fleurs-de-test-1678 | Local | 16.020000 | 1103.923292 / 34.273042 | 0.067665481 |
| fleurs-de-test-1679 | Local | 6.660000 | 1034.551187 / 26.174000 | 0.152328144 |
| fleurs-de-test-1680 | Local | 9.900000 | 1085.899480 / 31.461876 | 0.107968830 |
| fleurs-de-test-1682 | Local | 19.380000 | 1125.425729 / 24.425291 | 0.057059158 |
| fleurs-de-test-1683 | Local | 10.680000 | 1114.393959 / 24.160458 | 0.102119915 |
| fleurs-de-test-1684 | Local | 12.360000 | 1073.948208 / 23.181792 | 0.085373512 |
| fleurs-de-test-1685 | Local | 22.080000 | 1113.810604 / 85.621958 | 0.049578282 |
| fleurs-de-test-1686 | Local | 9.480000 | 1084.022000 / 43.467375 | 0.112779369 |
| fleurs-de-test-1687 | Local | 7.020000 | 1051.804333 / 23.123416 | 0.146215112 |
| fleurs-de-test-1688 | Local | 16.980000 | 1112.730917 / 208.056875 | 0.064398645 |
| fleurs-de-test-1689 | Local | 18.120000 | 1119.053291 / 62.337792 | 0.060531896 |
| fleurs-de-test-1690 | Local | 14.580000 | 1133.462542 / 29.808124 | 0.076527050 |
| fleurs-de-test-1691 | Local | 8.280000 | 1076.112124 / 18.772124 | 0.127434798 |
| fleurs-de-test-1692 | Local | 11.520000 | 1066.662583 / 15.085416 | 0.090951590 |
| fleurs-de-test-1693 | Local | 8.880000 | 1083.061375 / 17.048916 | 0.119424960 |
| fleurs-de-test-1694 | Local | 14.340000 | 1109.798438 / 40.798625 | 0.075968486 |
| fleurs-de-test-1695 | Local | 12.060000 | 1105.444062 / 24.751291 | 0.090089029 |
| fleurs-de-test-1696 | Local | 17.700000 | 1074.506687 / 14.748707 | 0.059444373 |
| fleurs-de-test-1697 | Local | 7.980000 | 1074.973187 / 68.873166 | 0.132033767 |
| fleurs-de-test-1698 | Local | 22.500000 | 1080.139500 / 19.878208 | 0.047165459 |
| fleurs-de-test-1699 | Local | 10.320000 | 1103.621312 / 43.366584 | 0.104976865 |
| fleurs-de-test-1700 | Local | 7.320000 | 1028.730292 / 18.176041 | 0.138419786 |
| fleurs-de-test-1660-quiet | Local | 10.380000 | 1082.534917 / 20.978584 | 0.102636232 |
| fleurs-de-test-1661-quiet | Local | 6.660000 | 1059.191812 / 17.639999 | 0.155597376 |
| fleurs-de-test-1662-quiet | Local | 23.460000 | 1090.141230 / 14.833667 | 0.045569775 |
| fleurs-de-test-1663-quiet | Local | 10.000000 | 1092.839937 / 24.492541 | 0.107112763 |
| fleurs-de-test-1664-quiet | Local | 8.700000 | 1057.844605 / 15.800000 | 0.119649337 |
| fleurs-en-test-1660 | Local | 10.680000 | 1059.457833 / 14.971500 | 0.097259149 |
| fleurs-en-test-1661 | Local | 6.360000 | 1040.105792 / 14.049542 | 0.159886845 |
| fleurs-en-test-1662 | Local | 8.380000 | 1048.966750 / 19.494042 | 0.122381290 |
| fleurs-en-test-1663 | Local | 12.120000 | 1073.943458 / 38.384791 | 0.086994551 |
| fleurs-en-test-1664 | Local | 8.360000 | 1037.996167 / 46.237792 | 0.121778197 |
| fleurs-en-test-1665 | Local | 8.280000 | 1063.035479 / 70.467541 | 0.126110228 |
| fleurs-en-test-1666 | Local | 8.340000 | 1048.151021 / 14.834458 | 0.123437463 |
| fleurs-en-test-1668 | Local | 13.320000 | 1078.146021 / 51.099333 | 0.079592255 |
| fleurs-en-test-1669 | Local | 7.200000 | 1034.222354 / 39.244708 | 0.140889193 |
| fleurs-en-test-1670 | Local | 29.300000 | 1156.862979 / 49.699334 | 0.038701881 |
| fleurs-en-test-1671 | Local | 10.020000 | 1049.559980 / 36.612500 | 0.103094372 |
| fleurs-en-test-1672 | Local | 10.160000 | 1048.264416 / 22.332083 | 0.101428619 |
| fleurs-en-test-1673 | Local | 10.080000 | 1069.391896 / 48.188083 | 0.104204756 |
| fleurs-en-test-1674 | Local | 5.640000 | 1025.158437 / 36.679750 | 0.178407764 |
| fleurs-en-test-1675 | Local | 8.760000 | 1039.096605 / 29.063876 | 0.116767364 |
| fleurs-en-test-1677 | Local | 5.940000 | 1027.449521 / 17.090084 | 0.169903830 |
| fleurs-en-test-1678 | Local | 12.780000 | 1074.035541 / 34.403875 | 0.082520067 |
| fleurs-en-test-1680 | Local | 10.200000 | 1069.765604 / 23.401333 | 0.102670795 |
| fleurs-en-test-1681 | Local | 19.980000 | 1087.518875 / 33.884416 | 0.053332938 |
| fleurs-en-test-1682 | Local | 15.640000 | 1097.423500 / 18.717458 | 0.068757928 |
| fleurs-en-test-1683 | Local | 11.100000 | 1076.897104 / 32.016292 | 0.095282701 |
| fleurs-en-test-1684 | Local | 10.960000 | 1058.508480 / 17.062874 | 0.095137890 |
| fleurs-en-test-1685 | Local | 13.920000 | 1074.218708 / 9.360541 | 0.075724532 |
| fleurs-en-test-1686 | Local | 12.120000 | 1065.628125 / 14.867042 | 0.086362992 |
| fleurs-en-test-1687 | Local | 11.520000 | 1049.565625 / 15.053750 | 0.089133442 |
| fleurs-en-test-1688 | Local | 17.440000 | 1096.010041 / 206.466125 | 0.061408535 |
| fleurs-en-test-1689 | Local | 20.760000 | 1086.118604 / 47.835209 | 0.051366993 |
| fleurs-en-test-1660-quiet | Local | 10.680000 | 1062.915375 / 15.534708 | 0.097510071 |
| fleurs-en-test-1661-quiet | Local | 6.360000 | 1038.719521 / 38.270709 | 0.159817558 |
| fleurs-en-test-1662-quiet | Local | 8.380000 | 1051.016229 / 37.640708 | 0.122798364 |
| fleurs-en-test-1663-quiet | Local | 12.120000 | 1074.981395 / 43.035875 | 0.086990393 |
| fleurs-en-test-1664-quiet | Local | 8.360000 | 1038.983500 / 19.099292 | 0.121561875 |
| synthetic-de-silence-5s | Empty | 5.000000 | 26.057063 / 9.231001 | 0.000055429 |
| synthetic-de-noise-5s | Empty | 5.000000 | 37.597354 / 22.181584 | 0.003899996 |
| synthetic-en-silence-5s | Empty | 5.000000 | 26.150229 / 7.541459 | 0.000034058 |
| synthetic-en-noise-5s | Empty | 5.000000 | 35.516083 / 19.188668 | 0.003969983 |
| constructed-de-masked-300s | Local | 300.000000 | 18038.823647 / 380.068250 | 0.060055358 |
| hd-de-acceptance-s04-t0-d8 | Local | 0.712875 | 988.623792 / 12.061458 | 1.360802093 |
| hd-de-acceptance-s05-t0-d9 | Local | 0.567000 | 976.456375 / 18.645917 | 1.688994011 |
| hd-en-acceptance-s04-t0-d8 | Local | 0.588000 | 992.228916 / 40.845499 | 1.652590456 |
| hd-en-acceptance-s05-t0-d9 | Local | 0.623500 | 976.807750 / 22.287250 | 1.539103983 |
| synthetic-de-ja | Local | 0.461375 | 983.126374 / 14.988751 | 2.086551477 |
| synthetic-de-nein | Local | 0.569312 | 985.499167 / 14.642750 | 1.690889231 |
| synthetic-de-thanks | Local | 1.834375 | 999.540875 / 32.618584 | 0.532833402 |
| fleurs-de-test-1732-thirty-second-v1 | Local | 30.000000 | 2084.717375 / 21.505917 | 0.068782296 |
| fleurs-en-test-1670-thirty-second-v1 | Local | 30.000000 | 1152.618375 / 20.005625 | 0.037792093 |
| calibration-de-trailing-silence | Local | 12.000000 | 1096.867146 / 44.527083 | 0.089734224 |
| calibration-de-room-5s | Empty | 5.000000 | 37.553938 / 9.572499 | 0.004037354 |
| calibration-en-trailing-silence | Local | 10.920000 | 1050.164000 / 30.147208 | 0.094341600 |
| calibration-en-room-5s | Empty | 5.000000 | 37.083666 / 17.756542 | 0.003986646 |
| diagnostic-de-controlled-300s | Local | 300.000000 | 15428.086562 / 249.636250 | 0.051350131 |

Die beiden konstruierten 300-s-Fälle mit jeweils 4.800.000 Frames sind vollständig in je elf Sitzungen verarbeitet worden. Sie sind ausdrücklich keine unveränderten natürlichen Mikrofonmonologe.

| 300-s-Fall | Stop→Text/Terminal Median / Spanne (s) | Engine-RTF Median / Maximum (10 Messungen) | Maximum RSS (Bytes) | Peak Memory Footprint (Bytes) |
|---|---:|---:|---:|---:|
| constructed-de-masked-300s | 18.038823646 / 0.380068250 | 0.060055358 / 0.061185932 | 946683904 | 929842784 |
| diagnostic-de-controlled-300s | 15.428086562 / 0.249636250 | 0.051350131 / 0.052079722 | 911294464 | 931808864 |

Die `/usr/bin/time -l`-Peaks umfassen jeweils den gesamten Fallprozess einschließlich Modellladen, einem Warmup, zehn Messungen und Cleanup. Höchstes Maximum RSS über alle 94 Prozesse: **946.683.904 Bytes**; höchster Peak Memory Footprint: **931.808.864 Bytes**. Die beiden Maxima stammen aus verschiedenen Prozessen. Das sind tatsächliche beobachtete Prozesspeaks, keine isolierten Sitzungsallokationen, Leakfreiheit oder automatische Ressourcenfreigabe.

Alle 94 Prozesspeaks, PIDs, Zeitstempel, CPU-/Policywerte, Engine-/Stop-/Terminalzeiten, Warmups, zehn Messzeilen je Fall, Rohdateihashes sowie pro Fall Median/p95/Spannweite stehen im [JSON-Bericht](paced-remainder-v1.json). Gerade Mediane mitteln die beiden zentralen Werte; p95 ist nearest-rank und bei zehn Messwerten damit das Maximum. Die per-phase Waitmaxima stammen aus den gespeicherten Summaries; interne Detailzeitstempel fehlen bei gültigen Publikationen. Die Sourcebindung sichert die Instrumentierung, ersetzt aber keine zweite unabhängige absolute Uhrspur.

Diese Restkorpusmessung ist kein zentraler Sechsfall-L-Nachweis und berechnet kein Gesamt-L. Inhaltliche Qualität/Anker, natürliche versus synthetische/konstruierte Quellen, Mikrofon-/Hotkey-/Empfängerabnahmen und endgültige Ressourcen-/Abbruchqualifikation bleiben getrennt. Kein P05-/P06-Abschluss und kein kausaler Vergleich mit früheren ungültigen Messversuchen wird behauptet.
