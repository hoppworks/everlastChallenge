# Begrenzte Beobachtung akzeptierter nativer Transkriptionssegmente

Entwicklerdiagnose für die erhaltenen Kurz-/Langaufnahmefehler. Keine zusätzliche
Produktkonfiguration, kein Textfilter und keine Optimierungsaktivierung.

Mit Cargo-Feature measurement ergänzt Engine::transcribe_observed den gemeinsamen
Vorbereitungs-/Decoderpfad um Eingangsframes, ausgewählten end-exklusiven
Energiebereich, ASR-Frames einschließlich kurzer Nullauffüllung und akzeptierte
native Segmente. Segmentfelder: Index, native Start-/Endzeit in Zentisekunden,
No-Speech-Wahrscheinlichkeit und unveränderter strikter UTF-8-Text. Normales
transcribe bleibt derselbe Pfad ohne diese zusätzlichen Getter/Beobachtung.

observed_corpus_probe lädt ASR/VAD über Engine::load auf genau einem seriellen
Besitzerthread und verwendet dasselbe gehashte WAV-Eingangsformat wie corpus_probe.
Höchstens vier Fälle, 4096 Segmente und 256 KiB roher Segmenttext je Fall.
Explizite private Neudateien mit Nichtüberschreibschutz; Fehler erhalten keinen
scheinbar gültigen leeren Text. Kein Tauri, Mikrofon oder laufender Controller.
Das Werkzeug ist daher eine Engine-Diagnose und keine zusätzliche L-Messung.

Nur akzeptierte Segmente nach full() sind sichtbar. Verwarfene oder fehlende
Fenster, tatsächliche Seek-/EOT-Entscheidungen, Fallbacktemperatur und dynamische
Prompt-Historien bleiben unbeobachtet. Native Zeiten bei no_timestamps=true
sind keine Wortausrichtung; mehrere Segmente können einen Fenster-NSP-Wert teilen.
no_context=true bedeutet im tatsächlichen Vendor keine garantierte Löschung der
Historie zwischen allen Fenstern desselben Full-Aufrufs. Keine Rückschlüsse auf
eine bereits beobachtete konkrete Ursache aus diesen Grenzen allein.

Kein Callback oder globaler Logger hinzugefügt. Ein zukünftiger Zugriff auf
verworfene Encoder-Versuche braucht eine gesonderte begründete Instrumentierung;
der sichere vorhandene Segmentgetter genügt dafür nicht. Jede tatsächliche
Diagnose bindet neue Quellen/Binary/Parameter vor Modellstart, alte Daten bleiben
an ihre ursprünglichen Revisionen gebunden.

## Verifikation des Implementierungspakets

Tatsächliches RED am Segmentbeobachtungstest (fehlender Record), anschließend
14 Transkriptions-/Beobachtungstests mit measurement und acht im Standardbuild
erfolgreich. Grenzen inklusive 4096 Records/256 KiB, ungültige Metadaten,
identische PCM-Vorbereitung/Padding und Gate-/Fehlerpfade geprüft. Die fünf
wiederverwendeten Beispiel-Eingangs-/Exporttests bestehen ebenfalls.
Workspace-Clippy für alle Targets mit measurement, Beispiel-Check, Format-,
Contract-, Guardrail- und Secretprüfung erfolgreich. Ein Secret-Scanner-
Fehlalarm auf zusammengeschriebene Protokollprosa wurde durch lesbare Formulierung
beseitigt, keine Regel ausgenommen. Unabhängiger kombinierter Review ohne
Befunde; gemeinsame Parameter einschließlich full()-Aufruf unverändert.
Eine tatsächliche Modellbeobachtung ist damit noch nicht durchgeführt.

Der erste tatsächliche Lauf scheiterte an einer nicht weiter differenzierten
Beobachtungsablehnung und einem inkompatiblen Ergebnisexport. Der Fehlerbericht
bleibt separat erhalten. Anschließend typisierte numerische Metadatenfehler
hinzugefügt, ohne Grenzen/Validatoren zu lockern; kompatible Dreifeldresultate
und vollständige Sidecar-Diagnose getrennt. Tatsächliches RED→GREEN, 15 Engine-
und sechs Beispieltests, strikte integrierte Prüfungen und unabhängiger Review.

Der zweite tatsächliche Lauf lokalisierte den Abbruch: native Rohstartzeit
−100730 Zentisekunden trotz unverändertem no_timestamps=true. Die Messschnittstelle
bewahrt deshalb jetzt auch negative oder umgekehrte signed Zeiten unverändert.
nonnegativeOrderedTimestamps prüft ausschließlich start >= 0 und end >= start;
selbst true garantiert keine tatsächliche Audio-/Wortausrichtung. Diese Werte
werden weder normalisiert noch zum Verwerfen des Texts benutzt. Index-/NSP-
Validierung und Segment-/Textgrenzen bleiben erhalten. Reales Regression-RED mit
den beobachteten Zahlen, danach 17 Tests erfolgreich. Kein Decoderverhalten geändert.

## Getrennte Zeitmarkenalternative für die Diagnose

Die zusätzliche measurement-only Methode transcribe_timestamped_observed und
explizite CLI-Option --native-timestamps setzen ausschließlich no_timestamps=false.
Produktionsmethode und bisherige Beobachtung behalten true. Vorbereitung, alle
anderen Decoderparameter, Segmentauswertung und Textverkettung bleiben gemeinsam.
Das Sidecar meldet die tatsächlich gewählte noTimestamps-Einstellung. Unbekannte
Optionen werden abgelehnt. Die Alternative aktiviert keine Produktoptimierung;
ihr tatsächlicher Modellvergleich braucht einen eigenen Vorabfreeze und erhält
alle positiven wie negativen Ergebnisse. Keine Wirkung ist vorweggenommen.

Für vollständige Qualitätskorpora unterstützt auch corpus_probe dieselbe explizite
measurement-only Option --native-timestamps. Der Standardaufruf bleibt beim
bisherigen InferenceService::load. Der Diagnoseaufruf lädt Engine auf dem seriellen
Entwickler-Hauptthread und übergibt sie mit lokalem Decoderadapter an denselben
Inferenzdienst; Jobs, Identitätsprüfung, Beobachtungstimeout und Shutdown bleiben
erhalten. Die gebundene native Beobachtung wird je Auftrag erzeugt und anschließend
verworfen, nur der unveränderte Text wird weitergegeben. Das Sidecar weist
Initialisierungsunterschied und zusätzliche Beobachtungsallokationen aus.
Daraus keine vergleichbaren Init-/Ressourcen- oder Echtzeitwerte ableiten.
Ohne measurement wird die Option bereits vor Datei-/Modellzugriff abgelehnt;
kein alternativer Produktpfad wird aktiviert. Die bestehenden Korpusgrenzen und
der private Dreifeldexport bleiben unverändert.
