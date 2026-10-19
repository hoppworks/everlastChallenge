# Startwartezeit und atomarer Signalwechsel

Der Runner toleriert vor dem tatsächlichen Start einen vorübergehenden Fokusverlust ausschließlich im Zustand Idle. Die bestehende Frist von 120 Sekunden bleibt unverändert. Die Empfängeranzeige wechselt währenddessen zu „Vorbereitung“ und bei wiederhergestelltem Fokus zur selben Startaufforderung. Es wird kein zusätzlicher Gestenbeleg erzeugt. Sobald Starting oder Recording beobachtet wird, gelten vor der öffentlichen Wiedergabe wieder die vollständigen Fokusprüfungen.

Nach einem Fehler mit verbrauchter Startaufforderung fordert der Runner den bestehenden, an Prozessidentität und Lauf gebundenen Watchdog-Abbruch auch für einen noch untätigen Produktprozess an. Dadurch bleibt nach einem gescheiterten Versuch kein unbewachter Prozess für einen verspäteten Tastendruck übrig. Ein Fehler beim Abschalten des Wächters korrigiert auch eine vorherige Erfolgsanzeige zu „fehlgeschlagen“.

Der Signal-Leser des Testempfängers wurde gegen dieselbe Klasse atomarer Dateiersetzungen abgesichert wie zuvor der Watchdog. Zwei deterministische Rename-Fenster waren vor der Änderung rot. Ausschließlich belegte Ersetzungen werden bis zu achtmal neu gelesen; bereits entkoppelte alte Daten werden nicht akzeptiert. Rechte, Eigentümer, Größe, Verzeichnis-/Markeridentität, Lauf, Phase und Aktualität bleiben geprüft.

39 gezielte Node-Tests und 13 native Empfängertests bestanden; ein vorhandener optionaler nativer Integrationstest blieb ausdrücklich übersprungen. Lint, Clippy und Empfängerbuild bestanden. Der unabhängige Quell- und Belegreview ergab keinen offenen Befund. Diese Regressionen verwenden kein Mikrofon und ersetzen keinen tatsächlichen G-Lauf. Das normale Produktbundle blieb unverändert; nur der Testempfänger wurde neu gebaut.

[Quellen, Binärbindung, Tests und Rohbelege](single-press-focus-wait-2026-10-19.json).
