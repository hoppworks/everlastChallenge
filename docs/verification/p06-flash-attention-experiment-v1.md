# P06 – isolierter Flash-Attention-Vergleich

Stand:18.10.2026. Prospekt vor nativen Kandidatenausgaben. **NOT RUN /
NOT QUALIFIED.** Keine Aktivierung, Leistungszusage oder finale Modellwahl.
Die [Quellenrecherche](flash-attention-feasibility-v1.md) begründet die
technische Variante; tatsächliche Ausführung und Ergebnisse stehen aus.

## Genau eine Konfigurationsänderung

Reference und Kandidat laden jeweils einen neuen Kontext aus derselben
gehashten `large-v3-turbo-q5_0`-Datei im bestehenden whisper.cpp-/Metal-Backend.
Die einzige semantische Parameteränderung ist
`WhisperContextParameters.flash_attn: false → true`. GPU-Gerät, Buildfeatures,
Threads, FullParams, Sprache, Zustandsneubau je Job, Energie-/VAD-/Paddingpfad
und Textzusammenführung bleiben identisch.

Beide Varianten behalten vollständigen Modellkontext1500 und `audio_ctx=0`.
Keine Segmentierung, Vorabtranskription, Änderung der Sprachschwellen oder
Modellgewichte. Die verworfene1024-Policy ist nicht Bestandteil dieses
Versuchs. Kein Backendwechsel und keine interne Kontextmutation. Beide Seiten
verwenden denselben Messwrapper und dieselbe Initialisierungs-/Workerownership.
Der Produktionsdefault bleibt während des Versuchs unverändert.

Flash verändert tatsächliche Graphen, KV-Layouts, Padding und Zwischenrechnung.
Es wird weder Bitgleichheit noch ein Speicher-/Geschwindigkeitsgewinn behauptet.
Ein gesetzter Bool allein ist kein Nachweis vollständiger GPU-Ausführung.
Fehler/Fallback und beobachtete Ausführungsgrenzen werden sichtbar protokolliert;
keine GPU-/Kernelbehauptung aus bloßer erfolgreicher Parametervalidierung.

## Erst unveränderte Dateiqualität

Die [feste100er-Auswahl](../../tests/fixtures/corpus/p06-context-quality-selection-v1.json)
und ihr [vollständiges Manifest](../../tests/fixtures/corpus/p06-context-quality-manifest-v1.json)
bleiben unverändert. Reihenfolge Reference→Flash→Flash→Reference, je ein
frischer Prozess, je100 Fälle in derselben Reihenfolge. Keine Wiederholungs-
oder Teilmengenwahl nach Ausgaben, keine neue Parameterwertsuche. Die
Vergleiche1/2 und4/3 werden einzeln beurteilt; ein verfehltes Paar wird nicht
durch Mittelung geheilt. Beide Seiten haben denselben Beobachtungsaufwand.

Neue gleichcode-/gleichwrapper-Referenz als Nenner verwenden. Historische
Ausgaben dienen nur der Drift- und Inhaltskontrolle; bei Referenzdrift
untersuchen, keinen bequemeren historischen Nenner wählen. Pro ursprünglicher
Sprach-/Klassengruppe gilt exakt `200*(E_candidate-E_reference)<=N` und clean
absolut höchstens10% WER. Die ursprünglichen90 behalten ihre Nenner; Supplement5,
vier bestehende Kalibrierrollen und controlled300 bleiben getrennte Kontrollen.

Alle100 tatsächlichen Texte je Block auswerten, einschließlich der langen
Kontrollen, Empty-/Raumfälle, Kurzäußerungen, Nachlauf und A22. Ganze geänderte
Referenz-/Baseline-/Kandidatenpaare und ihre Anker manuell differenziell prüfen.
Alte Urteile nur bei voller Referenz-/Status-/Hypothesenidentität übernehmen.
Keine neue verlorene Negation, falsche Zahl oder Segmentauslassung/-dopplung;
vorhandene Namens-/Wortfehler weiter offenlegen, kein neues pauschales
Nullfehler-Gate. Referenzen und Originalaudios nicht aus ASR-Texten korrigieren.

CLI-/Feature-/Parameterfehler müssen vor Modellinitialisierung scheitern.
Ausführung bindet tatsächliche Session-/Job-/Bereichs-/Sprach-/Frameidentität,
vorbereitete Framezahl, gewählten Audiokontext, tatsächlichen Decodeversuch,
ausgewählte Flash-Konfiguration und Aufräumen nach Join. Fehlende oder späte
Belege bleiben erhalten; ein Messfehler sperrt weitere Blöcke. Hypothesen in
privaten0600-Rohdateien, öffentliche Berichte enthalten Zahlen/Hashbindungen.

## Danach erst Leistung und Ressourcen

Dateizeiten sind kein Stop-bis-Text-L. Erst bei bestandener Qualität neue
gepaarte Echtzeitvergleiche durch denselben Runtime-/Ring-/DSP-/Worker-/
TC-Publisherpfad. Genau ein Warmup je zentralem Konfigurationslauf und
mindestens30 gemessene Wiederholungen je sechs zentralen Fällen; übrige feste
Fälle je ein Warmup und mindestens zehn gemessene Wiederholungen. Varianten-
reihenfolge, Jittergrenzen, vollständige Parameter und tatsächliche Zeitpunkte
vorher festhalten. Ältere L-Werte ersetzen die neue gepaarte Referenz nicht.

Aktivierung nur bei mindestens10% reproduzierbarer L-Verbesserung und sämtlichen
absoluten Median-/p95-, Qualitäts-, Ressourcen-, Abbruch- und Maximaldauergates.
Neue Graph-/State-/Prozesspeaks und100 Start/Stop/Cancel-Folgen prüfen; alte
`flash_attn=false`-Peaks allein qualifizieren `true` nicht. Die tatsächliche
lange Inferenz, Cancel und neuer Start müssen zur Flagvariante passen.

Vor dem ersten nativen Modellstart werden stabile Source, Binary/Build,
vollständige Parameter, Modelle, Eingaben, genaue Kommandos und neue Ausgabe-
ziele zusätzlich unveränderlich eingefroren. Während Modellläufen keine
konkurrierenden Builds, Tests, Modelle oder Agentenarbeiten. Alle Fehler und
unverfügbaren Läufe behalten; keine stillen Retries oder abgesenkten Gates.
