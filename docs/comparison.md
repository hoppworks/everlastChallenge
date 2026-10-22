# Vergleich mit Voicely: Ergebnis und Aussagegrenzen

## Aktueller Kandidat 0.3.0

Vier einzelne lokale Fallpaare zeigen ein gemischtes Bild. Gemessen wurde vom
tatsächlichen nativen Stoppereignis jeder App bis zum vollständig beobachteten
Text in einem unabhängigen Zielfeld. Die Bereiche begrenzen die Beobachtung;
sie sind keine statistischen Konfidenzintervalle.

| Deutscher Testfall | Everlast | Voicely | Inhaltsprüfung |
|---|---:|---:|---|
| Kurze Terminkorrektur | 446–493 ms | 859–894 ms | Everlast erhält die geprüften Fakten; Vergleichsausgabe verliert Satzinhalt |
| Verneinung | 482–526 ms | 955–990 ms | Everlast erhält die geprüften Fakten; keine allgemeine Qualitätsaussage |
| Zitat | 582–627 ms | 997–1049 ms | Beide verfehlen die vollständige Qualitätsanforderung |
| Langer Plan, etwa 60 s Audio | 3779–3847 ms | 2872–2922 ms | Beide mit Fehlern; Everlast erhält mehr Ausgangsinhalt |

Die Intervalle sind nach außen auf ganze Millisekunden gerundet.
[Einzelwerte und Nachweis-Hashes](benchmarks/native-comparison-0.3.0.json) binden
die Beobachtungen an Quellrevision und Audiodateien. Die kürzeren drei Fälle
sind beim Kandidaten schneller, der lange Fall etwa **0,86–0,97 Sekunden langsamer**.
Ein allgemeiner Geschwindigkeitsvorsprung oder gleichwertige Textqualität ist
damit nicht nachgewiesen. Insbesondere lässt sich daraus nicht ableiten, dass
nur die nachgelagerte Textbereinigung langsamer sei.

Beide Apps liefen lokal. Die Vergleichsapp protokollierte in allen vier Fällen
eine deterministische Bereinigung ohne LLM-Aufruf. Gleiche synthetische deutsche
Audiodateien wurden seriell über eingebaute Lautsprecher und Mikrofon zugeführt,
bei 50 % Ausgangslautstärke. Das ist keine bitidentische Mikrofoneingabe.

Es gibt nur eine Beobachtung pro Seite und Fall. Die bekannten Fälle wurden
nach Reparaturen an der Messsteuerung zusammengeführt; verworfene oder
abgebrochene Versuche bleiben dokumentierte Ausschlüsse, keine erfolgreichen
Wiederholungen. Die Stoppzeitpunkte lagen 15–51 ms auseinander, innerhalb der
vorab festgelegten 100-ms-Toleranz. Zusätzliche Verarbeitungszeit vor dem Stopp
kann trotzdem beeinflussen, wie viel Arbeit danach übrig bleibt. Die Messung
belegt weder einen unabhängigen Wiederholungstest noch eine allgemeine
Genauigkeitsrate, Erststartleistung oder Überlegenheit einzelner Backends.
