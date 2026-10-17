# P04/P05: angeforderte Audio- und DSP-Puffer

Stand:17.10.2026, geprüfte Anwendungspfade66d918a. Ergänzung der ausdrücklich
unvollständigen59.265.536B aus vad-resource.md; keine vollständige Heapgrenze.
Numerische Kapazitäten, feste Librarypläne und Prozessverbrauch bleiben getrennt.

## Besitz und eigene Puffer

| Benannte angeforderte Nutzkapazität | Formel | Bytes |
|---|---|---:|
| Zwei Journale |2×300×16000×4|38.400.000|
| Größter Ring |48000×8×4|1.536.000|
| Reader-Scratch |2048×8×4|65.536|
| Warmer VAD-Scratch |4800000×4|19.200.000|
| Mögliche Kurzaufnahme-Paddingkopie |16000×4|64.000|
| Bisher benannte Summe ||59.265.536|

Controller verschiebt das Journal in einen Arc; die Inferenz verwendet einen
Slice, keine weitere komplette PCM-Kopie. Maximal ein alter aktiver Job und ein
aktuelles Journal: Cancel entfernt wartende alte Audiodaten synchron, bevor neue
Aufnahme zugelassen wird. Ring und Reader-Scratch sind getrennte Puffer. Eigene
Vec-Reservierung ist eine angeforderte Kapazität, keine physische Heapmessung.

## Fehlende DSP-Payload ergänzt

pending reserviert1024f32=4096B auch beim16kHz-Bypass. Rubato0.16.2 und
RealFFT3.5.0 halten zusätzliche numerische Arrays. Für Eingangsrate r gilt:

```text
g = gcd(r,16000)
k = ceil(512/(r/g))
I = k*r/g; O = k*16000/g
Q = floor((I-1+1024)/I)*O
numerische DSP-Payload = 28I + 20O + 4Q + 8216
                       + 8*(ceil(I/2)+ceil(O/2)-2)
```

Das umfasst pending, AudioProcessor-Ausgabe, Rubato-Sample-/Spektralarrays und
RealFFT-Twiddles; FFT-Scratch und innere FFT-Planobjekte sind eigene Posten.
16kHz umgeht Rubato/FFT vollständig, pending bleibt reserviert.

| Rate |I|O|Q|Numerische DSP-Payload|Externer FFT-Scratch|
|---:|---:|---:|---:|---:|---:|
|8000|512|1024|2048|57.352B|0B|
|16000|Bypass|—|—|4096B|0B|
|24000|513|342|684|35.564B|0B|
|32000|512|256|512|32.776B|0B|
|44100|882|320|640|46.664B|0B|
|48000|513|171|342|30.096B|0B|

Die Formel und alle sechs Zeilen wurden unabhängig aus gepinnten Quellen
nachgerechnet. Eine einmalige Entwicklermessung erzeugte dieselben öffentlichen
RealFftPlanner<f32>-Pläne forward(2I)/inverse(2O) aus dem vorhandenen Release-
rlib. get_scratch_len() und make_scratch_vec().len() stimmten für beide Richtungen
aller fünf Resamplerformate überein: jeweils0 externe Complex-Scratchwerte.
Das bedeutet ausdrücklich nicht null innere FFT-Planallokationen. Diese Abfrage
verarbeitet kein Audiosignal; die Verarbeitung der Formate ist separat getestet.

Konservativ kombiniert ergeben die unabhängig größten Komponenten
**59.322.888B numerische Audio-/DSP-Payload**. Beim Resampling kommt zusätzlich
channel_mask mit einem Bool-Byte hinzu: **59.322.889B benannte Payload**. Diese
Maxima müssen nicht zugleich in einem Eingangsformat auftreten.

Für den tatsächlichen16kHz-Mono-Replaypfad: Ring64.000B, Reader8192B,
DSP-pending4096B. Mit denselben konservativen Journal-/VAD-/Paddingposten
ergibt das **57.740.288B numerische Payload**, ohne Resampler/FFT.

## Separate, begrenzte Kosten und Messgrenzen

Innere FFT-Planobjekte hängen an den festen I/O-Dimensionen und genau einem
aktiven Resampler; keine aufnahmelängenabhängige Planvermehrung. Ihre vollständige
interne Bytezahl wird von der öffentlichen API nicht offengelegt. Container,
Arc-/Channelobjekte, Allocatorrundung, Stacks und Konstruktorzwischenwerte sind
nicht in der Nutzkapazitätssumme enthalten. Allein sinc/filter_t bringen beim
Resamplerbau vorübergehend weitere12I Bytes plus Container; der Konstruktor kann
zusätzliche FFT-Arbeit verwenden. CPAL/OS sowie Whisper-/VAD-Modell-, Mel-,
Graph- und Backendallokationen sind separat als Prozessverbrauch zu messen.

Die kleine Plannerquery endete Exit0; time-l maß2.162.688B Peak-RSS und
1.360.232B Peak-Footprint. Das sind ausschließlich Werte des Queryprozesses,
kein Maximum des vollständigen Resamplers oder der App. Der reale100-Zyklen-
Lauf runtime-resource-v2 berichtet separat788.152.320B Peak-RSS und798.918.048B
Peak-Footprint. Der echte300s-Gesamtprozesspeak bleibt noch nachzuweisen.

Der [Bericht](audio-buffer-budget-v1.json) bindet Anwendungs-/Dependencyquellen,
Release-rlib, Compilerbefehl, Probequelltext/-binary und Zahlen-/time-Rohdateien.
Keine Modelle oder Aufnahmen ausgeführt. Kein vollständiger P04-/P05-Abschluss
und keine universelle Leakausschlussbehauptung aus einer Puffersumme.
