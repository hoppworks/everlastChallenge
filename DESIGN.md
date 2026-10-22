---
name: Everlast Coding Challenge
description: Kompakte native Diktierpalette mit schrittweise eingeblendeten Details
colors:
  text: "light-dark(#252824, #f3f7f1)"
  background: "light-dark(#fbfcfa, #252d29)"
  surface: "light-dark(#ffffff, #354038)"
  muted: "light-dark(#62685f, #bac6bc)"
  line: "light-dark(#dee2d9, #505c53)"
  control-line: "light-dark(#a8b2a2, #74836e)"
  accent: "light-dark(#293d2e, #d5ebd8)"
  accent-text: "light-dark(#ffffff, #1f3b28)"
  soft: "light-dark(#e8eee5, #37463b)"
  signal: "light-dark(#3b6346, #b6d9bb)"
  danger: "light-dark(#ad3c30, #f6c6bf)"
  danger-text: "light-dark(#ffffff, #712e24)"
typography:
  body:
    fontFamily: '-apple-system, BlinkMacSystemFont, "Segoe UI", sans-serif'
    fontSize: "14px"
    lineHeight: 1.55
  status:
    fontFamily: '-apple-system, BlinkMacSystemFont, "Segoe UI", sans-serif'
    fontSize: "16px"
    fontWeight: 600
    lineHeight: 1.35
    letterSpacing: "-0.02em"
  label:
    fontFamily: '-apple-system, BlinkMacSystemFont, "Segoe UI", sans-serif'
    fontSize: "12px"
rounded:
  control: "7px"
  field: "6px"
  transcript: "8px"
  level: "11px"
spacing:
  tight: "8px"
  group: "16px"
  section: "24px"
components:
  button-primary:
    backgroundColor: "{colors.accent}"
    textColor: "{colors.accent-text}"
    rounded: "{rounded.control}"
    padding: "7px 12px"
  button-secondary:
    backgroundColor: "{colors.surface}"
    textColor: "{colors.text}"
    rounded: "{rounded.control}"
    padding: "7px 12px"
  button-stop:
    backgroundColor: "{colors.danger}"
    textColor: "{colors.danger-text}"
    rounded: "{rounded.control}"
    padding: "7px 12px"
  level:
    backgroundColor: "{colors.soft}"
    textColor: "{colors.signal}"
    rounded: "{rounded.level}"
    width: "44px"
    height: "44px"
---

# Designsystem: Everlast Coding Challenge

## Überblick

Die Oberfläche ist eine kompakte Diktierpalette. Mikrofonpegel, Status und
Hauptaktion stehen in einer vertikal zentrierten Zeile. Transkript,
Aufnahmedetails und erweiterte Einstellungen erscheinen bei Bedarf. Die
macOS-Systemschrift bleibt erhalten.

Dieses Dokument beschreibt `src/features/app/styles.css`, `AppView.tsx`,
`LevelVisual.tsx` und `src/features/settings/styles.css`. Es ersetzt keine
Prüfung von Aufnahme, Build oder nativer Funktion.

## Farben

Zurückhaltende Grün- und Neutraltöne prägen das Werkzeug. Die Akzentfarbe
kennzeichnet die Hauptaktion, die Signalfarbe Mikrofonpegel und Fokus.
Die Gefahrenfarbe markiert das Stoppen während einer Aufnahme. Flächen,
Linien und dezente Hintergründe trennen Bedienelemente und Abschnitte.

Die Werte im Metadatenblock erhalten die CSS-Paare aus `light-dark()` und
`color-scheme: light dark`. macOS bestimmt die aktive Darstellung. Bei
erhöhtem Kontrast werden Sekundärtext, Trennlinien und Buttonränder verstärkt.

## Typografie

Durchgehend Systemschrift verwenden. Statusüberschriften nutzen die Rolle
`status`, kompakte Angaben die Rolle `label`. Fließtext und Transkript
verwenden `body`; das Transkript hat einen Zeilenabstand von 1,65.
Zeitangaben nutzen gleich breite Ziffern. Keine dekorativen Schriften ergänzen.

## Layout

Der Inhalt füllt das native Fenster und scrollt bei Bedarf. Der Innenabstand
beträgt 12px 20px 16px. Die Hauptzeile hat 16px Abstand und eine feste
44px-Pegelanzeige. Status und Aktionen sind daran vertikal ausgerichtet,
auch bei einzeiligen Ergebnissen wie einem Abbruch. Statusüberschriften haben
keinen Außenabstand. Der Appname steht in der OS-Titelleiste; im Inhaltskopf
erscheint nur die Einstellungsaktion.

Bis 460px Breite bricht die Zeile um und stellt Aktionen unter den Status;
der seitliche Innenabstand sinkt auf 16px. Das Transkript öffnet sich unter
einer Trennlinie. Einstellungen haben einen eigenen Scrollbereich, einen
festen Navigationskopf und 24px Innenabstand, bis 400px Breite 16px.
Die maximale Breite beträgt 680px.

## Flächen und Formen

Die Oberfläche verwendet flache Farbflächen und Trennlinien ohne eigene
Schatten. Der native Fensterrahmen gehört zur OS-Darstellung. Dekorative
Desktopflächen, Fensterampeln oder Schatten aus Browserentwürfen gehören
nicht in den App-Inhalt.

Bedienelemente sind leicht abgerundet. Die stärkere Rundung der Pegelanzeige
macht sie als kompaktes Element erkennbar. Transkriptfelder haben eine eigene
Rundung; Abschnitte werden durch Linien statt durch verschachtelte Karten getrennt.

## Komponenten

- **Aktionen:** eine hervorgehobene Aufnahmeaktion; Stoppen verwendet die
  Gefahrenfarbe. Sekundäre Aktionen haben eine Fläche mit sichtbarem Rand.
  Hover dunkelt die Hauptaktion ab; deaktivierte Aktionen bleiben erkennbar.
- **Pegel:** sieben Balken werden aus einem begrenzten nativen RMS-Wert
  abgeleitet. Der Balkenbereich ist 26px groß, mit 2px breiten Balken und
  2px Abstand innerhalb der 44px-Anzeige. Er zeigt den Pegel, kein Spektrum
  und keine Zufallsanimation. Eine zugängliche Pegelanzeige stellt den Wert
  bereit; im Ruhezustand erscheint das Mikrofonsymbol.
- **Transkript:** ein aufklappbarer Abschnitt zeigt auswählbaren, nicht
  editierbaren Text mit Kopieren und Löschen. Neue Ergebnisse öffnen den
  Abschnitt; reine Telemetrieänderungen öffnen ihn nicht erneut.
- **Einstellungen:** Navigation zeigt einzelne Bereiche. Auswahllisten und
  Modifikatortasten verwenden Standard-Bedienelemente. Erweiterte Details
  bleiben zunächst zugeklappt. Ein Zurück-Pfeil links neben „Einstellungen“
  führt zum Diktat; rechts gibt es keinen zusätzlichen Schließen-Button.
- **Rückmeldung:** Berechtigungsbedarf und Fehler bleiben sichtbar.
  Fokusmarkierungen sind 3px stark und haben 3px Abstand. Fehler nutzen die
  zusammengehörigen warmen Vorder- und Hintergrundfarben. Statusmeldungen
  unterscheiden einen versandten Einfügebefehl vom beobachteten Textempfang.

## Gestaltungsregeln

- Diktieren bleibt die Hauptaufgabe; unterstützende Details erscheinen bei Bedarf.
- Hell-/Dunkelpaare und die Kontrasteinstellung des Systems beachten.
- Fehler, Berechtigungsbedarf und Tastaturfokus sichtbar halten.
- RMS-Balken nicht als Spektrum ausgeben oder erfundene Telemetrie animieren.
- Browser-Fixtures und Screenshots nicht als Beleg nativer Funktion ausgeben.
