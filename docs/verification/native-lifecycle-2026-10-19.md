## Kandidat und Verfahren

Installiert: `/Applications/Everlast Dictation.app`, Produktquellstand 634d6c9,
erneut direkt gehashte Executable-SHA256
`45f4e463076c88e36adcc7ae95f810329686b32a69be374e0440d0ed7c0fd4ca`.
Aktuell erfasst: macOS 27.0.1, Apple M5, 34.359.738.368 Bytes RAM, arm64.
Einstellung Systemstandard, Deutsch. Keine Änderung von Mikrofon, Kürzel,
Modell, Systempräferenzen oder Rechten. Keine Inferenz-/Qualitätsmessung parallel.

Sämtliche GUI-Aktionen liefen über CUA an der echten installierten App.
Ein separater read-only Beobachter fragte mit der öffentlichen CoreAudio-API
den Standard-Eingang und `kAudioDevicePropertyDeviceIsRunningSomewhere` ab.
Die lokale SDK-Dokumentation definiert diesen Wert als geräteweite IO-Aktivität
in mindestens einem Prozess. Der Beobachter erzeugt weder AudioUnit noch IOProc,
liest keine Audiosamples und benötigt keine zusätzliche Mikrofonfreigabe.
Zusätzlich wurden Existenz des ausgewählten Produkt-PID, abgetastetes RSS per
`ps` und Dateideskriptorzahl per `proc_pidinfo(PROC_PIDLISTFDS)` erfasst.
CoreAudio wurde nominell alle 0,1 Sekunden abgefragt; Ressourcen etwa einmal
pro Sekunde. Deren letzter Wert wurde in den dazwischenliegenden Zeilen
übernommen. Die ersten neun Zeilen der 100er-Prüfung enthalten noch keine
RSS-/FD-Werte; 2.477 weitere Zeilen enthalten gültige, teilweise übernommene
Werte. Es sind nicht 2.486 unabhängige Ressourcenabfragen.

Hardwareaktivität ist kein Transkript- oder interner Join-Trace. Ein global
aktives Gerät wäre bei weiterer gleichzeitiger Nutzung nicht eindeutig Everlast
zuzuordnen. Hier war derselbe Standard-Eingang vor den jeweiligen Versuchen
inaktiv, wurde im Zusammenhang mit den sichtbaren Aufnahmen aktiv und war
danach wieder inaktiv. Device-ID 70 blieb innerhalb beider Versuche konstant.

## A19: regulär beenden während Aufnahme

1. Ausgangszustand: Prozess 42622 vorhanden, Mikrofoneingang inaktiv.
2. Im Hauptfenster **Aufnehmen** geklickt; **Aufnahme läuft** direkt beobachtet.
   Der separate Beobachter erfasste tatsächlich aktive Eingangs-IO.
3. Währenddessen im nativen Appmenü **Everlast Dictation beenden** angeklickt.
   Keine Prozessbeendigung per Force Quit oder Signal durchgeführt.
4. Beobachter meldete anschließend Prozess nicht mehr vorhanden und Eingang
   wieder inaktiv, über zehn aufeinanderfolgende Samples. Insgesamt 350 Samples,
   davon 199 mit gleichzeitig vorhandenem Prozess und aktivem Eingang.
5. Nach bestätigtem Prozessende regulär neu gestartet. Modellprüfung und danach
   **Bereit zum Diktieren**, leerer letzter Text und deaktiviertes **Kopieren**
   direkt beobachtet. Keine automatische Wiederaufnahme.

Beim nachfolgenden Nachlesen kein neuer produktbezogener Crashbericht im
Beobachtungszeitraum gefunden. Der native Menüpfad fordert den vorhandenen
kontrollierten Tauri-/Worker-Shutdown an; dessen Besitzer-/Joinverträge sind
zusätzlich durch die bestehenden kontrollierten Tests abgedeckt. Eine separate
interne Join-Telemetrie dieses Releasebundles wurde nicht erhoben.

Lokaler Rohbeleg: `artifacts/a19-native-quit-v1/observations.jsonl`, SHA256
`d8f47527a270afd3605b142f57b7a6fdf0b360e5214a15061702945a26a83101`.
`summary.json`, SHA256
`4daa93731faa7cf546976b5808c2b554e72ef103be9bbeaf3f116e4ffacb99d2`.
Der Beobachter beendete sich nach bestätigtem Prozessende selbst mit Exit 0.

**Noch offen für A19/P03:** echter Sleep/Wake während Aufnahme. Simulierte
Workspace-/Workerereignisse ersetzen diesen Betriebssystemfall nicht.

## P04/A11/A12: 100 kurze Aufnahme-/Abbruchfolgen

Nach dem Neustart, gleicher unveränderter Produktbinärdatei, Prozess 43672:

- 100-mal sichtbares **Aufnehmen → Aufnahme läuft → Abbrechen → Diktat
  abgebrochen** ausgeführt. Vor jeder Aktion aktuelle AX-Zustände gelesen;
  keine blind weiterlaufenden Klicks oder automatischen Neustarts.
- Nach jeder Folge **Kopieren** deaktiviert und kein Transkript im Ergebnisfeld.
  Die nächste Folge begann erst nach beobachtetem Abbruchabschluss. Alle 100
  fortlaufenden Fälle sind in den CUA-Ausgaben bestätigt, kein Fall blieb offen.
- Der getrennte CoreAudio-Beobachter erfasste ebenfalls genau **100 getrennte
  aktive Eingangsphasen**, davor und danach jeweils inaktiv. 2.486 Samples,
  kein beobachtetes Verschwinden des ausgewählten Produkt-PID.
- Gültige abgetastete Dateideskriptorzahl: Minimum, Maximum, Anfang und Ende jeweils
  **17**. Keine Akkumulation in dieser beobachteten FD-Klasse.
- Abgetastetes RSS: erster gültiger Wert **853.536 KiB**, Maximum **856.944 KiB**, letzter gültiger Wert
  **856.624 KiB**. Enddifferenz **3.088 KiB**, ungefähr **3,0 MiB**. Kein Nachweis
  eines exakten Speicherpeaks, sämtlicher Ressourcenklassen oder einer
  unbegrenzt langen Leckfreiheit.
- Beim Nachlesen kein neuer produktbezogener Crashbericht im Zeitraum gefunden.
  Am Ende kein aktiver Mikrofoneingang; App regulär weiter vorhanden und bereit
  für einen neuen Start, letzter Ausgang **Diktat abgebrochen**.

Der read-only Beobachter wurde nach dem letzten abgeschlossenen Fall über eine
lokale Stopmarke beendet; Exit 0 und `inputStillRunning: false` bestätigt.
Es wurden keine Mikrofonmitschnitte oder Transkripte gespeichert. Das sind
Aufnahme-/Abbruchfolgen, keine 100 vollständigen Stopp-/Inferenzdiktate und kein
automatischer Übergabenachweis. Starting-/Finalizing-Abbruch wurde in diesem
Lauf nicht real nativ ausgelöst.

Lokaler Rohbeleg: `artifacts/native-short-cycles-v1/observations.jsonl`, SHA256
`b0d91c4846b4dd3cffd052a1f017208973005b4a465fdb2c3e0a95893de5d210`.
`summary-v2.json` ergänzt die Stichproben-/Cachegrenzen der Ressourcenwerte;
SHA256 `a0506af485f91026d037e5859ad93ba839760590ba5c7bd6716c42bf23e28482`.
Die ursprüngliche `summary.json` und alle Rohdaten bleiben erhalten.
Beobachterquellen und aus den Rohdaten ermittelte aktiven Phasen liegen daneben.
Das `endUnixTime` einer Phase bezeichnet den ersten nachfolgenden inaktiven
Sample, keinen exakten physischen Hardware-Stoppzeitpunkt.

## Verbleibende Abschlussgrenzen

Der unabhängige P03-Quellaudit fand keine weitere fehlende kontrollierte
Kernfunktion. Übergänge, Starting-Stop, Repeat, Prioritätskanäle, Cancel-/Neustart,
veraltete Facts, Maximum, Drain/Join, Panik und kontrolliertes Sleep/Wake sind
gezielt abgedeckt. Historische native Restartbelege sind wegen inzwischen
geänderter Quellen kein pauschaler aktueller Produktnachweis.

Unabhängiger Readback reproduzierte Zeilenzahlen, aktive Phasen, gültige
RSS-/FD-Werte, ABI-/SDK-Zuordnung sowie Artefakt- und installierten Binärhash.
Eine zunächst zu pauschale Formulierung zu den Ressourcenwerten wurde wie oben
präzisiert. GUI-Aktionen und Crashberichtsuche bleiben direkte Root-Beobachtungen;
aus den Geräte-/Ressourcenlogs allein sind sie nicht erneut herleitbar.
