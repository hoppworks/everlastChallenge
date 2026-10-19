# Schneller autonomer Fehlerfall für langen Rauschnachlauf

Der offene Befund aus dem lokalen Maximaldauerlauf lässt sich jetzt ohne
Mikrofon als Abweichung von einer festen öffentlichen Referenz untersuchen.
Keine Produktparameter oder Modellwahl geändert.
[Daten, Befehle und Hashbindungen](long-noise-isolation-2026-10-19.json).

Der unveränderte öffentliche Satz `fleurs-de-dev-1517.wav` steht am Anfang.
Danach folgen entweder digitale Nullen oder deterministisches xorshift32-Rauschen,
Seed `0x45564C31`, PCM16-Spitzenamplitude 33 beziehungsweise 328.
Alle Dateien sind Mono/16 kHz/PCM16, Sprache unverändert. Der vorhandene
`transcribe_probe` wurde mit Standardfeatures gegen den aktuellen Stand gebaut.

| Gesamtdauer / Nachlauf | Referenzprüfung | Ausgabegröße |
| --- | --- | --- |
| 300 s / digitale Stille | bestanden | 134 UTF-8-Bytes |
| 300 s / Rauschen, Amplitude 33 | fehlgeschlagen | 251 UTF-8-Bytes |
| 300 s / Rauschen, Amplitude 328 | fehlgeschlagen | 386 UTF-8-Bytes |
| 30 s / Rauschen, Amplitude 33 | bestanden | 134 UTF-8-Bytes |
| 60 s / Rauschen, Amplitude 33 | fehlgeschlagen, dreimal in frischem Prozess | jeweils 147 UTF-8-Bytes |
| 120 s / Rauschen, Amplitude 33 | fehlgeschlagen | 173 UTF-8-Bytes |

Ein autonomer Fehlerdurchlauf dauert ungefähr vier Sekunden einschließlich
Modellstart; die WAV wird schneller als Echtzeit verarbeitet:

```sh
target/release/examples/transcribe_probe artifacts/models/ggml-large-v3-turbo-q5_0.bin artifacts/long-noise-isolation-v1/noise-low-60s.wav --language de --repeat 1 --reference artifacts/long-noise-isolation-v1/expected.txt
```

Erwarteter derzeitiger Befund: Exit 1, `147 bytes`, `reference_match=false`.
Das ist ein bewusst roter Diagnosefall, kein fehlgeschlagener Softwarebuild.
60 Sekunden sind die kürzeste untersuchte fehlerhafte Dauer, keine exakt
bestimmte Fehlergrenze. Keine vollständigen Transkripte protokolliert; die
Größendifferenz beweist weder die konkreten zusätzlichen Wörter noch die
Identität der Ursache mit der akustischen Mikrofonaufnahme.

## Eingrenzung und nächste Fixgrenze

Vor der weiteren Untersuchung wurden drei prüfbare Möglichkeiten priorisiert:
Rauschen bleibt durch die Energie-Randbestimmung im Decoderinput; längerer
Kontext begünstigt die Abweichung; zustandsabhängige Wiederverwendung verursacht
sie. Die stillen und kürzeren Kontrollen sowie frische Prozesse trennen diese
Fälle teilweise: Rauschen und Dauer beeinflussen den beobachteten Fehler;
Wiederverwendung eines alten Prozesses ist dafür nicht notwendig.

Die letzten 20 ms des kurzen Fehlerfalls haben RMS `0.000570873`, oberhalb
der aktuellen Endschwelle `0.000025` in `speech_range`. Nach dessen Regel bleibt
dieser Nachlauf im Energieintervall. Die semantische VAD gibt aktuell nur die
gesamte Aufnahme frei; sie schneidet diesen Nachlauf nicht separat ab.
Das erklärt die Exposition gegenüber Rauschen, noch keinen sicheren Fix.

Die Energieschwelle einfach anzuheben würde die vorhandene Reserve für leise
Sprache gefährden. Ein Fix muss den roten Dateifall und leise beziehungsweise
späte echte Sprache gemeinsam prüfen, ohne Textphrasen herauszufiltern.
Die bestehende qualifizierte Referenz und sämtliche formalen Paketgrenzen
bleiben bis zu einem solchen geprüften Fix unverändert. Stand 4/14.
