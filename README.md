# Everlast Dictation MLX

Isolierte Entwicklungsvariante für Apple Silicon und macOS 27. Dieses Checkout
liegt getrennt unter `everlastChallenge-mlx`. Es installiert oder ersetzt die
Original-App nicht. Appname, Bundle-ID, Datenverzeichnis und Entwicklungsport
sind von der laufenden Referenzfassung getrennt.

Der aktuelle Kandidat verwendet **MLX für den Whisper-Encoder und den bisherigen
whisper.cpp-Decoder für den vollständigen Text**. Die Modellkoeffizienten stammen
bytegenau aus der bisherigen Q5-Referenz und werden lokal in F32 konvertiert.
Sprache, VAD, Audioaufbereitung, Decoderparameter und Textübergabe bleiben auf dem
bestehenden Pfad. Aufnahmen mit mehr als 30 Sekunden vorbereitetem PCM verwenden
den bisherigen vollständigen Decoderpfad; spätere Whisper-Fenster ebenfalls.
Es gibt keine satz-, datei- oder korpusspezifische Umschaltung.

Die vollständigen MLX-Decoder wurden wegen neuer Inhaltsfehler verworfen. Sie
sind keine qualifizierte Alternative zum hybriden Kandidaten. Vorberechnung
während der Aufnahme ist derzeit deaktiviert.

## Stand der Prüfung

Standard dieser isolierten Variante ist der **F16-Encoder aus den verifizierten
F32-Koeffizienten**, mit F32-Featureübergabe an den vollständigen bisherigen
Decoder. Der wiederverwendbare MLX-Cache ist auf 256 MiB begrenzt; das ist kein
Limit für den gesamten Prozess. Vorberechnung bleibt aus. Eine ausdrücklich
bereits gespeicherte Modellwahl bleibt erhalten. Ein Build ohne Defaultfeatures
kann weiterhin den bisherigen Referenzpfad verwenden.

Der aktuelle Kandidat besteht den vollständigen 100er-Qualitätsvergleich und
zwölf vorab festgelegte Bestätigungsfälle aus acht neuen öffentlichen Aufnahmen
und vier leiseren Varianten. Zwei verbesserte englische Ausgaben wurden als
vollständige Textpaare einzeln und per SHA gebunden geprüft. Alle Sprach- und
Testklassengruppen erfüllen den Vertrag. Das ist ein endlicher Korpusnachweis,
keine universelle Fehlerfreiheitsgarantie. Deutsch bleibt die Produktsprache;
Englisch dient zusätzlich dem Vergleich.

Der kontrollierte Sechsarmvergleich umfasst beide Paarreihenfolgen gegen CPP
und gegen den bisherigen unbegrenzten F16-Cache: jeweils ein Warmup pro Prozess
und 30 Echtzeitmessungen je DE/EN-5/10/15-s-Fall. Alle 1.086 Ausgaben wurden
unabhängig geprüft; alle sechs Fallmediane und p95 verbessern sich gegen CPP.
Das zentrale L ist der ungewichtete Mittelwert dieser sechs Fallmediane.

| Messgröße | Bisheriger CPP-Pfad | Gewählter F16-/Cache256-Pfad |
| --- | ---: | ---: |
| Zentrales Stop-bis-Text-L, erste Paarreihenfolge | 1,060 s | 0,356 s |
| Zentrales Stop-bis-Text-L, Gegenreihenfolge | 1,057 s | 0,358 s |
| macOS-Spitzen-Speicherfußabdruck | 0,809–0,812 GB | 3,179–3,180 GB |

Das sind **66,2–66,4 % weniger Stop-bis-Text-Zeit**. Gegen den unbegrenzten
F16-Cache verändert sich L in diesen zwei Paaren um −0,07 % beziehungsweise
+0,78 %; einzelne p95 steigen bis 5,18 %. Dies ist ein angenommener gemessener
RAM-/Speed-Kompromiss, kein statistischer Gleichheitsnachweis. Die beobachtete
Modellbereitschaft im frischen Cache256-Prozess beträgt etwa 6,8 Sekunden und
ist von der warmen Textlatenz getrennt. Betriebssystemcaches wurden nicht
geleert; dies ist kein Start nach einem Rechnerneustart.

Ein separater identischer Phasenvergleich senkt den warmen RSS-Median gegenüber
dem unbegrenzten F16-Pfad von **1,414 auf 0,861 GB (−39,1 %)**. Der Spitzen-
Speicherfußabdruck sinkt dort von 3,475 auf 3,176 GB (−8,6 %). Aktive
Encoderallokationen bleiben etwa 1,274 GB; die Einsparung betrifft vor allem
wiederverwendbaren Cache. Nach Finish werden die eigenen Modelle freigegeben,
der Cache geleert und das vorherige Prozessbudget wiederhergestellt. RSS,
MLX-Allokationen und Speicherfußabdruck überlappen und werden nicht addiert.
GB meint dezimale Milliarden Byte.

Die aktuellen Ressourcenbelege umfassen fünf Warmups und 100 Sitzungsfolgen
mit Stop, Aufnahmeabbruch und Decoderabbruch: 72 echte Decoder-/PCM-Vergleiche,
39 exakt referenzgleiche Stop-Ergebnisse und 66 textlose Abbrüche, reguläres
Finish/Join. RSS nach Warmup 911.474.688 Byte, letzter Messpunkt vor Cleanup
804.667.392 Byte; dies belegt keine universelle Leckfreiheit. Das dortige
`loadMicros` misst nur Worker-Spawn/Setup vor asynchroner Modellbereitschaft.

Auch die übrigen 94 Fälle sind vollständig abgeschlossen: je ein Warmup und
zehn Messungen, insgesamt **1.034 Ausgaben und elf vollständige Qualitätsrunden**.
Alle Ausgaben entsprechen dem aktuellen Qualitätskandidaten; 1.023 sind exakt
CPP-identisch, elf verwenden dasselbe zuvor geprüfte englische Verbesserungspaar.
Alle 94 Prozessgruppen sind regulär beendet und unabhängig abwesend bestätigt.
Die zwei 300-Sekunden-Fälle sind deutsch und verwenden den bisherigen CPP-Pfad.
Beim erlaubten 30-Sekunden-Fall bleiben native Folgefenster erhalten. Der Restlauf
liefert kein neues zentrales L und keine zusätzliche kontinuierliche Energieprüfung.

Nachweise des gewählten Kandidaten:

- `artifacts/implementation/ram-budget-v1/cache-runtime-paced-readback.{md,json}`
- `artifacts/implementation/ram-budget-v1/cache-runtime-readback.{md,json}`
- `artifacts/implementation/ram-budget-v1/cache-runtime-quality100-readback.{md,json}`
- `artifacts/implementation/ram-budget-v1/cache-runtime-prospective-readback.{md,json}`
- `artifacts/implementation/ram-budget-v1/cache-runtime-resource-readback.{md,json}`
- `artifacts/implementation/ram-budget-v1/cache-runtime-remainder-readback.{md,json}`

Rohbelege unter `artifacts/mlx-qualification/` werden nicht eingecheckt. Frühere
F32-Vergleiche und fehlgeschlagene Versuche bleiben erhalten; ihre Ergebnisse
werden nicht nachträglich auf den gewählten Kandidaten übertragen. Tracker und
Ausführungsstand liegen unter `../everlastChallenge-mlx-plan/.scratch/mlx-speed/`.
Die älteren Berichte unter `docs/verification/` betreffen überwiegend das Original.
Aktuelle Projekt-/Bundle-/Offlineprüfungen werden unter
`artifacts/implementation/final-acceptance-v1/` separat festgehalten.

## Entwickeln und prüfen

Geprüfter Rechner: Apple M5, 32 GiB RAM, macOS 27.0.1, Xcode 27.1. Die lokale
`rust-toolchain.toml` pinnt Rust **1.99.1**. Die globalen Rust-Defaults werden
nicht umgestellt. Der native MLX-Build benötigt den passenden Apple-Metal-
Toolchain-Baustein; dieser wurde auf dem Prüfhost über Apples Toolchainverwaltung
bereitgestellt. Andere Macs oder macOS-Versionen wurden nicht gemessen.

- Abhängigkeiten: `npm run setup`.
- Statische Projektprüfung: `npm run check`.
- Bestehende Testsuite: `npm test`; vollständig mit Coverage und Abhängigkeitsprüfung: `npm run verify`.
- MLX-/Messverträge zusätzlich: `cargo test --workspace --locked --features mlx-encoder-f16,measurement,test-control` und die featuregebundenen Example-Tests.
- App-only-Release: `npm run release:mlx` oder `npm run release`.
- Bereits eingerichtete lokale Signieridentität: `npm run release:local`.
- Gebautes Paket prüfen: `npm run verify:bundle`.

Der Releasehelper kopiert die tatsächlich verwendete `mlx.metallib` neben das
App-Executable, signiert zuerst die kopierte Bibliothek und dann die App und
prüft Signatur, Mikrofonbeschreibung und Audio-Entitlement. Das Ergebnis liegt
unter `target/aarch64-apple-darwin/release/bundle/macos/Everlast Dictation MLX.app`.
Die Befehle installieren, veröffentlichen oder notarieren nichts und aktivieren
keinen Entwickler-Steuerkanal. Beim Verschieben müssen Signatur-xattrs erhalten
bleiben, auf diesem Mac beispielsweise mit `ditto`.

## Modelle und Laufzeit

Die MLX-Encoder-Variante benötigt sechs geprüfte Dateien mit zusammen
**3.813.494.037 Byte**: ursprüngliche Q5-Referenz, daraus erzeugte F32-Gewichte,
Konfiguration, Tokenizer, Tokenizerkonfiguration und Silero-VAD. Die Einrichtung
lädt fest gepinnte Eingaben erst nach ausdrücklichem Start und prüft Größe und
SHA-256. Die F32-Datei wird mit sicherem Rust lokal erzeugt; sie hat keinen
Downloadendpunkt. Ein Abbruch beseitigt ausschließlich die eigene Teildatei;
ein vorhandenes geprüftes Ziel bleibt erhalten.

Beide MLX-Varianten verwenden getrennte Modellverzeichnisse unter ihrer festen
Modell-ID. Das eigene AppData lautet
`~/Library/Application Support/de.hoppworks.everlast-dictation-mlx/`.
Die Original-App und deren Einstellungen/Modelle werden nicht übernommen oder
verändert. Die isolierte App registriert kein globales Diktier-Tastenkürzel.

Nach erfolgreicher Einrichtung arbeitet die Inferenz lokal. Initialisierung,
Inference und Freigabe aller MLX-Objekte erfolgen im selben langlebigen Worker.
Pro Auftrag wird frischer Decoderzustand verwendet. Abbrechen verwirft das
Ergebnis der widerrufenen Sitzung; es verspricht keine harte Unterbrechung eines
bereits laufenden Metal-Kernels. Texte werden nur nach vollständiger finaler
Prüfung weitergegeben.

Der zusätzliche Encoder hält neben der Referenz ein zweites ASR-Modell und einen
zweiten VAD-Kontext bereit; je Auftrag arbeitet ein VAD. Der Speicherbedarf ist
daher weiterhin höher als bei der bisherigen Q5-Referenz; gemessene Kosten und
Grenzen stehen oben.

## Grenzen und Abhängigkeiten

Native MLX-/Metal-Bindings und der kleine lokale Encoder-Bridgepatch sind
zusätzlicher Wartungsaufwand. Versionen, native Quellpins und Patchbelege stehen
in `Cargo.lock`, `config/mlx-build-inputs.json` und den jeweiligen
`vendor/*/PATCHES.md` bzw. `EVERLAST-PATCH.json`. Lizenz-/Herkunftsnachweise stehen
unter `docs/licenses/`; dort offengelegte Lücken sind keine Rechtsfreigabe.
