# Natürlicher Raumgeräuschfall für zusätzliche Kalibrierung

Recherche und Abruf: 2026-10-17. Ausschließlich zusätzliche **Kalibrierung**; der
eingefrorene Acceptance-Korpus bleibt unverändert. Keine Modelle, Transkription
oder Mikrofonaufnahme wurden hierfür gestartet. Die Quellenprüfung verwendet
Primärquellen und wurde durch einen lokalen Codex-Rechercheagenten durchgeführt.

## Gewählte Quelle und Label

**„Empty Office Room Tone“ von richwise**, Freesound-Sound-ID **456207**,
veröffentlicht am **2019-01-09**. Der Herausgeber beschreibt die Aufnahme seines
Büros vor Ankunft der anderen Personen: leises Lüfterrauschen und ein sehr leiser
wiederkehrender Piepton um 3,7 kHz; Aufnahmegerät Zoom H5 mit XYH5-Kapseln. Damit
ist eine reale Büroaufnahme und kein synthetisches Rauschen beschrieben.
[Herausgeberseite und Beschreibung](https://freesound.org/people/richwise/sounds/456207/).

**Labelbeweis und Grenze:** Die Einordnung als Raumgeräusch ohne Sprache beruht
auf dieser Herausgeberbeschreibung. Sie ist keine vollständige zeitliche
Sprachannotation. Es erfolgte hier keine Hörprüfung; Sprachfreiheit ist deshalb
**publisher-labeled, nicht unabhängig durch Hören verifiziert**. Auch ein
Energie-Gate oder ein leeres ASR-Ergebnis würde diese Lücke nicht schließen.
Der periodische Piepton gehört zum natürlichen Aufnahmeinhalt und darf nicht
als reines breitbandiges Lüfterrauschen beschrieben werden.

Vor jeglicher Inferenz ist der Kalibrierungsbereich auf die **ersten vollständigen
5 Sekunden, Intervall [0, 5 s)**, festgelegt. Der Herausgeber nennt keinen
Startton oder Sprachvorlauf. Die Auswahl erfolgt unabhängig von Decoderergebnissen;
keine nachträgliche Suche nach einem besonders günstigen Segment. Derselbe
sprachneutrale Ausschnitt kann je einen DE-/EN-Decoder-Kalibrierungsfall bilden,
ist dadurch aber weiterhin nur **eine unabhängige Naturaufnahme**. Vorläufige
Referenz: leer, mit obigem explizitem Labelvorbehalt. Dieser Recherchebaustein
extrahiert noch keinen Ausschnitt und ändert kein Manifest.

## Lizenz und Attribution

Die Herausgeberseite verlinkt für genau diesen Sound **CC0 1.0 Universal**;
dies gilt für die veröffentlichte Aufnahme. Die Lizenz erlaubt Kopieren,
Bearbeiten und Weitergeben, auch kommerziell, ohne vorherige Erlaubnis.
[Herausgeber-Lizenzangabe](https://freesound.org/people/richwise/sounds/456207/),
[CC0 1.0](https://creativecommons.org/publicdomain/zero/1.0/),
[rechtlicher Lizenztext](https://creativecommons.org/publicdomain/zero/1.0/legalcode).

Nachweis zur Herkunft, auch wenn CC0 keine Namensnennung verlangt:
**„Empty Office Room Tone“ — richwise, Freesound 456207, 2019-01-09,
CC0 1.0; offizielles HQ-MP3-Preview, später ggf. auf [0, 5 s) gekürzt und
nach PCM16/Mono/16 kHz konvertiert.** Keine Unterstützung dieses Projekts durch
den Urheber implizieren. Eine spätere Konvertierung muss eigene Ausgabehashes,
Framegrenzen und Werkzeugversion erfassen.

## Exakte Datei und reproduzierbarer Abruf

Das Original ist laut Seite eine 7:52.052 lange WAV-Datei mit Stereo/48 kHz/
32 Bit und 172,9 MB. Ihr Download erfordert Anmeldung und überschreitet das
50-MB-Budget. **Das Original wurde nicht geladen.** Verwendet wird stattdessen
die vom offiziellen Seiten-HTML im Attribut `data-static-file-url` verlinkte,
anonym erreichbare **HQ-MP3-Vorschau**. Sie ist eine verlustbehaftete Ableitung
der Naturaufnahme; die Original-WAV-Spezifikation darf nicht auf die MP3-Datei
übertragen werden.
[Seite mit Preview-Verknüpfung](https://freesound.org/people/richwise/sounds/456207/),
[direkte offizielle Preview-Datei](https://cdn.freesound.org/previews/456/456207_1481531-hq.mp3).

- Lokaler ignorierter Pfad: `artifacts/corpus/source/freesound-456207-hq.mp3`.
- Stabile Herausgeber-ID: `456207`, Preview-Asset-ID `456207_1481531-hq`;
  keine zusätzliche semantische Release-Version ausgewiesen. Der Inhalts-Hash
  pinnt daher die tatsächlich abgerufene Fassung.
- HTTP HEAD: 200, `Content-Type: audio/mpeg`, `Content-Length: 10002720`.
- `Last-Modified: Wed, 09 Jan 2019 19:23:44 GMT`;
  `ETag: "5c364a40-98a120"` (HTTP-Metadaten, kein kryptografischer Pin).
- Vollständiger GET: HTTP 200, **10.002.720 Bytes**, Dateimodus **0600**.
- **SHA256:** `0f2131bd208b604498f32f0151e98ca7440a603d1309a25879c04a232ecba047`.
- `ffprobe` des heruntergeladenen Assets: **MP3, Stereo, 48.000 Hz,
  472,052 s**, gemeldete Dateibitrate 169.518 bit/s. Dies ist eine reine
  Codec-/Containerprüfung, keine Hörprüfung oder Spracherkennung.

Der Abruf verwendet keine Cookies, Konten, API-Schlüssel oder Uploads, bricht
oberhalb von 50.000.000 Bytes ab und publiziert erst nach vollständigem Lesen
über eine private 0600-Tempdatei. Der nachfolgende Reproduktionsabruf muss gegen
den hier dokumentierten vollständigen SHA256 prüfen und bei Abweichung stoppen;
eine geänderte CDN-Datei ist keine stillschweigend austauschbare Version.

Reproduktionsabruf im Repository (maximal 50 MB, keine Überschreibung; kein
Modell- oder Audiowiedergabeschritt):

```python
import hashlib, os, tempfile, urllib.request
from pathlib import Path

url = "https://cdn.freesound.org/previews/456/456207_1481531-hq.mp3"
pin = "0f2131bd208b604498f32f0151e98ca7440a603d1309a25879c04a232ecba047"
final = Path("artifacts/corpus/source/freesound-456207-hq.mp3")
final.parent.mkdir(parents=True, exist_ok=True)
temporary = None
try:
    with urllib.request.urlopen(url, timeout=30) as response:
        if response.status != 200:
            raise ValueError("unexpected HTTP status")
        with tempfile.NamedTemporaryFile(dir=final.parent, delete=False) as out:
            temporary = out.name
            os.fchmod(out.fileno(), 0o600)
            digest, count = hashlib.sha256(), 0
            while True:
                block = response.read(min(1024 * 1024, 50_000_001 - count))
                if not block:
                    break
                count += len(block)
                if count > 50_000_000:
                    raise ValueError("download exceeds budget")
                digest.update(block)
                out.write(block)
            if count != 10_002_720 or digest.hexdigest() != pin:
                raise ValueError("source version changed")
            out.flush()
            os.fsync(out.fileno())
    os.link(temporary, final)  # fails if final already exists; no replacement
finally:
    if temporary is not None:
        os.unlink(temporary)
```

Vorhandene Datei ohne erneuten Download prüfen:

```sh
shasum -a 256 artifacts/corpus/source/freesound-456207-hq.mp3
wc -c < artifacts/corpus/source/freesound-456207-hq.mp3
/opt/homebrew/bin/ffprobe -v error -show_entries format=duration,size,bit_rate:stream=codec_name,sample_rate,channels,duration -of json artifacts/corpus/source/freesound-456207-hq.mp3
```

## Aussagekraft

Geeignet als zusätzlicher natürlicher Negativfall für die Untersuchung von
Energy-Gate und Whisper-Halluzinationen. Noch kein gemessenes Decoderergebnis,
keine Schwellenfreigabe und keine Erfüllung der vollständigen P05-Abnahme.
Die Quelle deckt nur ein Büro, ein Aufnahmegerät und eine MP3-Übertragung ab;
Bluetooth, reale Nutzermikrofone und andere Räume bleiben offen. Keine
synthetische Ersatzaufnahme und kein Beleg für alle Arten von Raumgeräusch.
