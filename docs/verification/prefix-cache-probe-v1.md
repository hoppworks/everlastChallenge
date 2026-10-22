# Entwicklerdiagnose für den vorbereiteten Präfix-Cache

Historische prospektive Messprobe. Normale Engine- und App-Starts wählen diesen
Decoder nicht. Ihr Build belegt weder Cachefunktion noch Qualität, Latenz oder
Ressourcenverhalten.

## Aufruf und Eingabevertrag

`prefix_cache_probe MODEL MANIFEST NEW_OUTPUT` wählt das gepinnte Referenzmodell
und dessen VAD-Nachbardatei. Das begrenzte Korpusschema enthält genau zwei oder
vier Fälle als aufeinanderfolgende Präfix-/Gesamtpaare. Jedes Paar hat eine
Sprache, nicht leere PCM16-Mono-16-kHz-Daten und ein Präfix, das nicht länger
als die Gesamtdatei ist. Nach dem gemeinsamen WAV-Leser muss jedes f32-Sample
des Präfixes bitgleich dem Gesamtsample sein. Keine Normalisierung, Verbindung,
Neuabtastung, Transkription oder abgeleitete Teilreferenz kommt hinzu.
Zusätzliche Referenzfelder im Manifest dienen nicht als inhaltliche Wahrheit;
die Präfixbeziehung ist eine unabhängig geprüfte Audioeigenschaft.

Jedes Paar besitzt einen frischen PrefixCacheDecoder und seriellen
InferenceService. Präfix und Gesamtaudio teilen Sitzung, Generation und
WorkToken; danach wird dasselbe Gesamtaudio mit neuer Sitzung und neuem Token
übergeben. Auftrags-IDs steigen global. Der mittlere Auftrag darf den Cache
treffen oder verfehlen. Der Auftrag mit neuer Sitzung muss echtes natives
Decodieren melden, keine wiederverwendete Ausgabe.

Das Modell wird auf dem Hauptthread des Diagnoseprozesses geladen und danach
in den tatsächlichen InferenceService-Worker verschoben. Das unterscheidet
sich von der asynchronen Produktinitialisierung und belegt keine gleiche
Kaltstartlatenz. Getaktete Eingabe, Mikrofon, native Übergabe und Echtzeit-L
werden nicht geprüft.

## Nachweisvertrag

Eine private, atomar und ohne Überschreiben veröffentlichte JSON-Datei enthält
alle drei/sechs vorgesehenen Zeilen, auch Fehler oder nicht verfügbare Werte.
Exportiert werden Text-SHA256 und UTF-8-Länge, nie Rohtext oder PCM. Aktuelle
Auftrags-/Sitzungs-/Generationskennung und ursprünglicher Cacheauftrag stammen
aus Workerbestätigungen. Der Worker liefert Threadkennung und PCM-Fingerabdruck;
vorbereitete Frames/Hashes stammen aus dem Decoder. Ein erfolgreicher Abschluss
verlangt beobachteten Cleanup-Join.

Gebunden werden Quellen, Modell/VAD, Binary, Manifest, Eingaben und feste
Parameter einschließlich `ObservationSink` in `transcription/observation.rs`
und Deploymentkonfiguration `.cargo/config.toml`. Vor einem nativen Lauf sind
ein eigener eingefrorener Messstand und unabhängiger Review erforderlich.
Dieser Bericht bestätigt keine tatsächliche Messung.

## Modellfreie Prüfung

TDD-Befehl für das reine Messbeispiel:

`cargo test -p everlast-dictation --example prefix_cache_probe --features measurement --locked`

- CLI-Gerüst: Exit 101, ein Test mit „not implemented“; anschließend einer bestanden.
- Bestätigungs-/Präfixgerüste: Exit 101, zwei fehlgeschlagen; anschließend vier bestanden.
- Serieller Worker: zunächst fehlgeschlagene Cleanup-/Zeilenprüfung, danach zehn bestanden.
- Erste Gesamtprüfung: 13 bestanden (0.03s).
- Unabhängiger Review fand einen falschen abschließenden Beleg: Ein späteres
  Completed ohne Bestätigung wurde übersprungen und gab einen älteren
  Präfixbeleg zurück. Der Test `final_receipt.is_none()` scheiterte mit Exit 101.
  Danach wurde zuerst das letzte tatsächliche Completed und erst dann seine
  optionale Bestätigung ausgewählt. Kein Rückgriff auf ältere Belege bleibt.
- Abschließende Gesamtprüfung: **14 bestanden**, keine Fehler/ignorierten Tests (0.02s).
- `cargo clippy -p everlast-dictation --example prefix_cache_probe --features measurement --locked -- -D warnings`:
  Exit 0. Eigene Rustquellen wurden formatiert, Fremdquellen nicht verändert.

Kontrollierte Tests verwenden den echten Worker mit synthetischem Decoder und
Belegquelle. Sie prüfen aktuelle Kennungen, fremden Workerthread, Bitfingerabdruck,
drei erhaltene Zeilen, Fehlerweitergabe, keine Textserialisierung und echten
Join. Sie belegen keine nativen Whisper-/Cachetreffer oder GPU-Kernel.
Hash-, Format-, Pfadgrenzen und Ausgabe ohne Überschreiben sind separat geprüft.
Berichte sind auf 1 MiB, vorab geladene Eingaben auf vier begrenzte Vektoren
mit insgesamt 76.800.000 PCM-Nutzbytes begrenzt; das ist kein Produktpufferbudget.

120 Sekunden Ergebnis-/Bereitschaftsbudget und separate 120 Sekunden für
Cleanup sind keine harte native Unterbrechung. Unbestätigtes Cleanup verhindert
weitere Modellladungen. Der abschließende Beleg ist das letzte tatsächliche
Completed vor Besitzer-Join, keine neue Cacheabfrage nach Shutdown.
Die Decoderzeit enthält den Diagnosewrapper, also weder reine Cachekosten
noch die Runtime-Zeit vom Stoppen bis zum Text.

In diesem Versuch wurden kein Modellauftrag, Appstart, Rechtewechsel,
Schedulingwechsel, Bundle-/Releasebuild oder tatsächlicher Diagnoselauf
vorgenommen. Ein nativer Lauf benötigt neue Bindungen der dann aktuellen
Quelle, Binary, Parameter, Modelle, des Manifests und der Eingaben.
