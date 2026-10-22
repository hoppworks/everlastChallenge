# Bedienung und Fehlerfälle

Installation und die vier Schritte zum ersten Diktat stehen im
[README](../README.md#loslegen).

## Aufnahme und Einfügen

Das globale Tastenkürzel **⌥D** startet und stoppt die Aufnahme. Dabei wird das
Ziel vor einem möglichen Fokuswechsel erfasst. Bei unsicherem oder verändertem
Ziel bleibt der Text in der App; ein versandtes Paste-Ereignis ist noch keine
Bestätigung, dass die Zielanwendung ihn übernommen hat.

Der Aufnahmebutton oder **⌘R** im App-Fenster erzeugt ein lokales Diktat.
**Escape** verwirft eine aktive Aufnahme. Ein Abbruch verhindert die spätere
Textübergabe, unterbricht aber nicht zwingend einen bereits laufenden GPU-Kernel.
Nach fünf Minuten endet die Aufnahme automatisch.

„Kopieren“ übernimmt exakt den sichtbaren Text. Endet ein Hintergrunddienst,
bleibt der letzte Text kopierbar; die App erklärt den erforderlichen Neustart.
Ein gescheiterter Modelldownload kann ohne App-Neustart wiederholt werden.

## Oberfläche und Modell

Die Hauptansicht zeigt Status, Mikrofonpegel und Aufnahmeaktion. „Letzter Text“
und „Aufnahmedetails“ lassen sich aufklappen. Neue Ergebnisse öffnen den Text
auch bei zwei identischen Diktaten. Die Pegelbalken zeigen den tatsächlichen
Mikrofonpegel, kein Frequenzspektrum.

Einstellungen öffnen sich über das Zahnrad oder **⌘,**; der Zurück-Pfeil führt
zum Diktat. Das Fenster wächst für Text und Einstellungen. Hell-/Dunkelmodus
folgen der macOS-Einstellung.

Die App verwendet ausschließlich den MLX-Hybrid. **Einstellungen → Modell**
zeigt Status und Herkunft. Bekannte ältere Modellwahlen werden beim Start auf
den Hybrid umgestellt; Mikrofon, Sprache und Tastenkürzel bleiben erhalten.
Deutsch ist die geprüfte Produktsprache. Die [Leistungsübersicht](performance.md)
beschreibt Qualitäts- und Messgrenzen.

## Mikrofonformate

Ein unterstütztes Standardformat bleibt erhalten. Andernfalls wählt die App
ein angebotenes Format mit 1–8 Kanälen und 8/16/24/32/44,1/48 kHz: weniger
Kanäle zuerst, dann höhere Rate, dann Float-/Signed-Formate. Gibt es kein
passendes Format, verweist die Fehlermeldung auf das Audio-MIDI-Setup.

CPAL kann die nominale Rate des Audiogeräts systemweit umstellen. Dadurch
können andere Audioanwendungen betroffen sein. Eine Unterstützung jedes
externen Interfaces ist nicht nachgewiesen.
