# P05: Referenzqualifikation und vorläufige Modellwahl

Stand:18.10.2026, Produkt-/Messquellen `b9799b6`; abgeschlossener
Messnachweis im Archivcommit `3d4e6ab`. Entscheidung nach P05 und §13 des
[Implementierungsplans](../../IMPLEMENTATION_PLAN.md#measurement).

**`large-v3-turbo-q5_0` ist als vollständige Referenz qualifiziert und vorläufig
gewählt. `small-q5_1` ist wegen deutscher clean-WER über10% ausgeschlossen.**
Die finale Modell-/Parameterwahl und die absolute Leistungsabnahme gehören zu
P06. Es wird noch kein finales Modellmanifest geändert.

## Zusammengeführte Nachweise

| Pflicht | Entscheidung und Beleg |
|---|---|
| Fester Korpus, Herkunft und Auswertung | Unveränderte rechtegeklärte Audio-/Referenzbindungen, vorab festgelegte Klassen, Parameter und Unicode13-Auswerter; vollständiger aktueller [Restbericht](paced-remainder-v1.md) und [Qualitätsbericht](paced-remainder-v1-quality.md). Kalibrierung, Supplement und ursprüngliche Abnahme bleiben getrennt. |
| Clean-WER höchstens10% je Sprache | Reference: DE46/915=5,0273%, EN34/734=4,6322%. Die aktuelle kanonische Full90-Rekonstruktion verwendet genau eine Ausgabe je ursprünglichem Fall:84 Restfälle plus sechs Zentralfälle, identische Decoder-/Vorbereitungs-/Stateparameter. Keine Gewichtung durch unterschiedlich viele Wiederholungen. |
| Zahlen, Negationen und Segmentvollständigkeit der festen Anker | Alle158 ursprünglichen clean-Inventaranker sind mit historischen Ganzpaar-/Urteilhashes gebunden. Keine verlorene Negation, falsche Zahl oder neue Segmentauslassung/-dopplung in diesen Ankern. Bekannte Namens-/Wortfehler und eine Namensidentitätsunklarheit bleiben dokumentiert; keine erfundene Null-Namensfehlergrenze. |
| Kurze Sprache, Stille, Raumrauschen und A22 | Menschliche kurze Digitfixtures und die synthetischen Ja-/Nein-/A22-Fälle werden verarbeitet. Alle sechs festen Empty-/Room-Fälle bleiben in sämtlichen elf aktuellen Ausgaben leer. Beide Nachlauffälle liefern keinen zusätzlichen Endtext. Das ist eine Fixtureabnahme, keine beliebige Mikrofon-/Empfängergarantie. |
| Zehn warme Durchläufe der übrigen Fälle, einschließlich30s und300s | Alle94 Fälle mit einem Warmup plus zehn Messungen vollständig:1034 erfolgreiche Zeilen,911.559 Publikationen innerhalb des vorab festgelegten10ms-Budgets. Beide300s-Fälle besitzen vollständige Frame-/Jobabdeckung und tatsächliche Enginezeiten, RTF und Prozesspeaks. Einzelwerte, Mediane und Spannweiten im Restbericht. |
| Fünf Minuten ohne sichere Energiepause | Controlled300 hat4.800.000 Frames und15.000 vollständige20ms-Fenster; minimaler RMS0,0002655040258626123 über Start0,00005 und Ende0,000025. Alle21 unbeschnittenen Quellblöcke sind einmal erkennbar. Die [Rezeptur](long-control-v1.md), tatsächlichen PCM-Werte und bekannte lokale Inhaltsfehler sind gebunden. Ein natürlicher Monolog oder perfekte Worttreue sind keine zusätzlichen Planbedingungen. Der stark maskierte Originalfall bleibt unverändert als Geräuschgrenze erhalten. |
| Kalte Modellbereitschaft, getrennt von warmen Messungen | Drei neue Prozesse je Modell im [Initialisierungsbericht](model-initialization.md): Reference1269/1245/1251ms, Small549/548/539ms. Keine geleerten OS-Caches, keine App-/Hotkey- oder erste Sprachinferenzzeit behauptet. |
| Begrenzte Audio-/DSP-Ressourcen | [Pufferbudget](audio-buffer-budget-v1.md): maximal59.322.889Byte benannte konservative Audio-/DSP-Payload; tatsächlicher16kHz-Mono-Replay57.740.288Byte. Feste FFT-Pläne, Bibliotheks-/Allocator-/OS-/Backendkosten bleiben getrennt. Keine unbegrenzte PCM-Übergabekopie. |
| Tatsächlicher Prozessverbrauch und100 Sitzungsfolgen | [Resource-v2](runtime-resource-v2.md):34 Stop,33 Capture-Cancel,33 Decoder-Entry-Cancel,101 RSS-Punkte, alle Besitzer freigegeben. Kein über den ganzen Lauf fortgesetzter Anstieg; letzter Wert62.308.352Byte unter Warmupbaseline. Aktuelle Langprozesse ergänzen RSS bis946.683.904Byte und Footprint bis931.808.864Byte. Keine erfundene RSS-Schwelle und kein allgemeiner Leakausschluss. |
| Abbruch, alte Ergebnisse und neue Aufnahme | Resource-v2:66 abgebrochene Sitzungen ohne übernommenen Text. [Echter300s-Restart](native-runtime-restart-v1.md): neue Sitzung erreicht Recording und Finalizing vor Freigabe des alten Decoders, beide echten Engineaufrufe enden, ausschließlich aktuelles Ergebnis wird übernommen, Runtimejoin bestätigt. Die Barriere liegt vor dem inneren Decode; keine Kernelunterbrechung behauptet oder verlangt. |
| Reproduzierbare vollständige Echtzeitreferenz | [Central-v4](paced-six-reference-v4.md): ein Warmup plus180 Messungen, alle Zeilen und90.250 Publikationen gültig. L=1,059564354s. Alle sechs Mediane überschreiten1s; alle p95 erfüllen1,5s. Nach P05/§13 blockiert diese Leistungsgrenze P05 nicht; sie bleibt ausdrücklich für P06 offen. |

Die Ressourcenentscheidung folgt aus begrenzten Besitz-/Pufferverträgen,
den100 tatsächlichen Sitzungsfolgen, dem bestätigten Aufräumen und den nun
gemessenen Langprozesspeaks gemeinsam. Das vorsichtige `resourceQualified:false`
eines früheren Einzelberichts wird nicht rückwirkend verändert; es war keine
bereits getroffene Gesamtentscheidung und ersetzt diese Verknüpfung nicht.

## Übertragbarkeit älterer Belege

Die aktuellen Qualitäts-/Zeitreihen verwenden die produktive Timestamp-Policy
`no_timestamps=false`; keine nachträgliche Textdeduplizierung. Ältere Belege
werden ausschließlich für ihre tatsächlich unveränderte Messgrenze übernommen:

- Native-Restart: alle14 Quellpins gegen immutable `66d918a` geprüft und aktuell
  bytegleich. Pufferbudget: alle sieben Anwendungspins aktuell bytegleich.
- Resource-v2: alle30 Pins gegen immutable `736242d` gültig;20 aktuell identisch.
  Engine, SpeechGate, DSP, Controller, Control und Lockfile sind unverändert.
  Änderungen an Runtime/Inference ergänzen ausschließlich Messfeature-Timing
  und Trace; übrige Unterschiede betreffen Messwerkzeuge und Exporte.
  `runtime`, `preprocessing`, `state` und `models` des damaligen Baseline-
  Parameterdokuments sind exakt gleich den aktuellen TC-Parametern.
- Kalte Initialisierung: `Engine::load` hat damals und aktuell denselben
  Funktionshash; SpeechGate und Backend sind unverändert. Der Unterschied in
  `model.rs` klassifiziert TLS-Fehler und verändert keinen lokalen Ladepfad.
  Die spätere Decoderpolicyänderung liegt hinter der gemessenen Bereitschaft.

Die vorangegangenen unabhängigen Reviews und die erneute Prüfung dieser
Übertragbarkeit ersetzen keine erneute Abnahme nach zukünftigen relevanten
Produktänderungen. Diskrete TC-/CPU-Snapshots beweisen keine kontinuierliche
RT-Klasse. Frühere ungültige Zeitläufe bleiben erhalten und werden nicht als
bestandene Vergleichswerte verwendet.

## Modellwahl und verbleibende Arbeit

Small verfehlt im unveränderten vollständigen Korpus DEclean mit100/915=
10,9290%; ENclean42/734=5,7221%. Der
[Native-Timestamp-Abnahmebericht](native-timestamps-acceptance-v1.md) bindet
diese Ergebnisse. Die Konfiguration ist damit nicht P05-qualifiziert. Ihre
nicht ausgeführten vollständigen warmen Ressourcen-/Zeitreihen werden nicht
als vorhanden ausgegeben; eine bereits am Pflichtgate ausgeschlossene
Konfiguration benötigt keine weiteren Qualifikationsläufe für die Auswahl.

Unter den qualifizierten vollständigen Referenzen bleibt nur Reference.
Sein gleichgewichtetes clean-Q beträgt rund4,8297%. Damit ist die vorläufige
Wahl unabhängig von einer Rangfolge weiterer unqualifizierter Modelle.
P06 prüft Vorabtranskription und reduziertes `audio_ctx` zunächst einzeln und
gegebenenfalls kombiniert: mindestens10% L-Gewinn, höchstens0,5 Prozentpunkte
WER-Verschlechterung je Sprache und Klasse, keine neuen kritischen Anker-/
Segmentfehler sowie alle absoluten Pflichtgrenzen. Keine Optimierung ist hier
bereits aktiviert oder verworfen.
