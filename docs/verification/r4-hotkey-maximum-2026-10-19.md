# Reale Hotkeyaufnahme bis zur Fünf-Minuten-Grenze

## Ablauf und tatsächlich bestandener Teil

Die öffentliche Fleurs-Datei mit SHA256
`b892cf9dc80c437f0a78c6f261d83dda196252e31a1e415032981472d3ea9137`
wurde ohne Sprachänderung in eine 360-s-Fixture eingebettet:45 s digitale Stille,
10,56 s öffentliche Sprache, restliche Datei digitale Stille. SHA256 der
abgeleiteten WAV:
`bc2b7b11958baca48af5db22fa07f7573161b133300ab7f12278677eeabee9fd`.
Das sind keine gespeicherten Mikrofon-PCM-Daten.

Der read-only Beobachter erfasst vor Beginn inaktiven Standardeingang 70,
dann genau eine aktive Phase. Erster aktiver bis erster nachfolgender inaktiver
Sample: **299,982 s**. Aus den benachbarten Samples liegt die Dauer der
beobachteten Aktivität zwischen **299,878 und 300,084 s**. Eingangs-IO war danach
wieder inaktiv; Produktprozess über alle 4101 Samples vorhanden. Der tatsächliche
spätere Appstatus lautet **„5-Minuten-Grenze erreicht · Text wurde zurückgehalten“**.
Das belegt den echten automatischen Maximaldauerstopp im Hotkeylauf.

CoreAudio ist ein geräteweiter Aktivitätswert, kein interner Join-Trace. Hier
war derselbe Eingang davor und danach inaktiv und wurde nur im Zusammenhang
mit der angeforderten Aufnahme aktiv. Hardware nominell alle 0,1 s abgefragt,
RSS/FD nominell jede Sekunde; Ressourcenwerte zwischen Abfragen übernommen.
Die ersten zehn Zeilen enthalten noch keine Ressourcenwerte; 4091 Zeilen
enthalten gültige, teilweise übernommene Werte. Abgetastetes RSS: erster gültiger
Wert 168864 KiB, Maximum 346448 KiB, letzter gültiger Wert 295408 KiB.
Alle 4091 gültigen FD-Werte sind 17. Kein exakter Speicherpeak oder vollständiger
P06-Ressourcennachweis. Beobachter ausdrücklich beendet, Exit 0 und Eingang
inaktiv bestätigt. Rohdatei:
`artifacts/hotkey-maximum-preparation-v1/observations.jsonl`, SHA256
`eaa7a76367ff701840310974959f2188a954f626049a0010d1832226a3c1385b`.

## Nicht bestandene beziehungsweise noch offene Teile

Das Empfängerfeld blieb leer und ohne zusätzliches „∂“. Die App nennt
**„Startziel nicht erfasst“**: fokussiertes Element nicht als geeignet bestätigt.
Die Zielprüfung hat deshalb zurückgehalten. Welches globale Vordergrundfeld
beim tatsächlichen Start erfasst wurde, ist nicht unabhängig belegt. Die per
CUA gemeldete Fokusposition des eigenen Fensters beweist nicht automatisch,
dass dieses Fenster global vorne war. Daraus folgt noch kein nachgewiesener
Fehler des TargetReaders; Produktionsguard nicht abgeschwächt.

Der öffentliche Satz ist im Ergebnis enthalten, zusätzlich erschien weiterer
Text. Vollständiger Mikrofontext und Mikrofon-PCM wurden nicht gespeichert.
Die synthetische Quelldatei enthält im Vor-/Nachlauf nur digitale Stille; das
beweist nicht, dass die echte Mikrofonaufnahme störungsfrei oder identisch war.
Die Ursache der Zusätze bleibt zunächst zu isolieren, beispielsweise über
einen einzelnen exakten öffentlichen Dateifall. Keine fehlerfreie Qualitäts-
oder automatische Einfügeabnahme aus diesem Versuch.

Die UI-Beobachtung der laufenden App lieferte zwischenzeitlich ScreenCaptureKit
-3812; derselbe Prozess/Beobachter blieb nachweislich aktiv. Nicht neu gestartet,
kein Fenster zum Auslesen zwangsweise geöffnet. Der spätere Zugriff auf denselben
Apphandle funktionierte wieder. **Die 30-s-Vorwarnung wurde nicht direkt gesehen**;
eine vollständige Aussage zu Tray/Ton/keinem neu auftauchenden Fenster wird
nicht aus diesem Fehler abgeleitet. A26 bleibt daher insgesamt offen, obwohl
der tatsächliche 300-s-Stopp jetzt belegt ist. Gesamtpakete weiterhin 3/14.

Ein unabhängiger Readback bestätigt Rohzahlen, Zeitgrenzen, öffentliche
PCM-Einbettung, Fixturehashes und Abnahmegrenzen. Die anfänglichen zehn
Zeilen ohne Ressourcenwerte sind nach seinem Hinweis ausdrücklich getrennt.

## Begrenzte Dateidiagnose danach

Zwei sequenzielle Durchläufe über den realen Reference-Inferenzpfad:
öffentliche Originalaufnahme (10,56 s) und unveränderte öffentliche Sprache
mit digitaler Stille auf insgesamt 300 s (35 s Vorlauf). Beide liefern
134 UTF-8-Bytes und `reference_match=true` gegen dieselbe erwartete Referenz,
Exit 0. Keine Mikrofonaufnahme, kein neuer Appbuild oder ASR-Patch.
[Strukturierter Diagnosebeleg](r4-long-silence-isolation-2026-10-19.json).

Damit reproduziert langer digital stiller Kontext allein den Zusatztext nicht.
Die Ursache im akustischen Mikrofonlauf bleibt offen. Zwei Einzelfälle sind
keine allgemeine Qualitäts- oder Leistungsqualifikation; insbesondere sind
Start-/Cachebedingungen verschieden und die Laufzeiten kein belastbarer
Geschwindigkeitsvergleich.
