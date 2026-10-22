# Zeitweilige Diagnose der installierten Tastenkürzel

Historische Untersuchung vom 21.10.2026. Die anfängliche Beobachtung war noch
keine Fehlerkorrektur oder bestandene Abnahme; Ergebnis und Abbau stehen unten.

Die installierte App bestand zunächst den berichteten Stopptest mit Option+D.
Escape wurde als nicht funktionierender Abbruch gemeldet; die Vordergrund-App
war unbekannt. Später wurde auch Option+D als nicht verfügbar beschrieben.
Eine gezielte Prozessstichprobe zeigte ansprechbaren Haupt-Eventloop und
Desktopbesitzer sowie nur einen installierten Produktprozess. Eine Ursache
war daraus nicht ableitbar.

Automatisierte Eingaben konnten bei fokussierter App Aufnahme und Escape-Abbruch
auslösen. Mit TextEdit im Vordergrund kam Escape nicht an; automatisiertes
Option+D löste ebenfalls nichts aus. Synthetische Eingaben sind damit kein
Beleg für physische globale Tastaturzustellung.

## Ausdrücklich aktivierte Protokollierung

Die Diagnose wurde am 21.10.2026 aktiviert. Vor dem Start des damaligen
Diagnosebuilds wurde die leere Datei
`~/Library/Application Support/de.hoppworks.everlast-dictation/hotkey-diagnostics.enabled`
angelegt. Jede Installation des Hotkeybesitzers erzeugte dort eine neue
`hotkey-diagnostics-<pid>-<time>.log` mit Modus 0600. Entfernen der Markierung
und Neustart deaktivierten die Protokollierung. Die Instrumentierung war
vorübergehend und wurde nach der Diagnose entfernt.

Erfasst wurden nur vergangene Mikrosekunden, `toggle`/`cancel` und feste
technische Stufen: Registrierung, nativer Tastendruck/-freigabe, angenommene/
ignorierte Ereignisse, Hauptthread-Warteschlange/-Callback, Widerruf und Fehler.
Audio, Transkript, beliebige Tasten, Fenstertitel und Zielprozessidentität
wurden nicht gespeichert.

Der native Callback führte keine Datei-I/O aus und wartete nicht auf die
Diagnosesperre. Die vorab angelegte Warteschlange fasste 128 Einträge;
die Gesamtausgabe war auf 2048 begrenzt. Bei verlorenen Ereignissen oder
Sättigung erschien zusätzlich `diagnostic / incomplete`. In einem solchen
Trace beweist eine fehlende Zeile kein fehlendes natives Ereignis. Der
Hintergrundbesitzer leerte die Warteschlange auch abschließend beim Shutdown.
Öffnungs-/Schreibfehler erschienen im bestehenden App-Fehlerpfad.

## Prüfumfang

Ein eigenständiger Rust-Test prüfte ausdrückliche Aktivierung, begrenzte Ausgabe,
Sättigungsmeldung und neue Logdatei bei erneuter Installation. Quellenregeln
und vollständiger Diagnosebuild waren vor der Installation erforderlich.
ASR-Korpus, Zeitkampagne und Modellvergleich wurden nicht wiederholt.
Zur Diagnose der globalen Zustellung waren frischer Trace und bekannter
physischer Tastendruck nötig.

## Installierter Kandidat und erste Beobachtung

Diagnosequelle: `e4ea631beb737adc82148ce051e5c7df99d40181`.
Executable-SHA-256:
`f8e27c8f7b825c4aed2e90d5e94cbbd817cc58e9bd48df6c9291ec7693d2c1d7`.
Die vorherige App blieb unter
`artifacts/hotkey-diagnosis-2026-10-21/before-diagnostics.app` erhalten.
Strikte Signaturprüfung, Diagnosetest, `check:fast` und Release-Clippy mit
`-D warnings` bestanden. Nach Korrekturen fand der unabhängige Review keinen
verbleibenden blockierenden Befund.

Der erste Trace zeigte erfolgreiche Toggle-Registrierung, dann Registrierung
und Scharfschaltung von Escape während einer per UI gestarteten Aufnahme,
anschließend Entfernung nach lokalem Escape-Abbruch. Synthetisches Option+D
und Escape erzeugten kein `native_pressed`; eine `incomplete`-Markierung fehlte.
Damit übte dieser Automationsweg den globalen Callback nicht aus. Physische
Tasten waren dadurch nicht geprüft. Die Aufnahme wurde verworfen; die App
blieb idle mit aktivierter Diagnose.

Lokale Belege liegen unter `artifacts/hotkey-diagnosis-2026-10-21/`.
Das vorherige ZIP wurde nicht als korrigierter Hotkey-Release umbenannt.

## Physisches Ergebnis und Abbau

Anschließend wurde bestätigt, dass Option+D weiterhin startet/stoppt. Der
physische Trace enthielt vollständige Toggle-Callbacks sowie Escape-Druck,
Annahme, Callback, Freigabe und Deregistrierung ohne `incomplete`.
Die sichtbare UI zeigte **Diktat abgebrochen**. Laut Rückmeldung fügte Escape
nichts in Notes ein. Das entspricht dem Abbruchvertrag: Zweites Option+D
beendet zur Erkennung/Übergabe, Escape verwirft.

Die erste Meldung war als fehlender Abbruch missverstanden worden. Diese
Sequenz belegt funktionierende globale Zustellung und Abbruch für den
beobachteten Fall, nicht alle Szenarien oder eine unabhängige Notes-Prüfung.
`physical-result.json` enthält Ergebnis und Loghash; der technische Rohtrace
bleibt im ignorierten lokalen Artefaktverzeichnis.

Markierungsdatei und Instrumentierung wurden entfernt. Der Laufzeitcode wurde
auf den Zustand vor der Diagnose zurückgesetzt. Diagnosecommits und Ergebnis
bleiben in der Historie erhalten.
