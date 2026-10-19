# R4: tatsächliche lokale Einrichtung und nativer Funktionstest

Beobachtet am 19.10.2026 auf dem dokumentierten Mac (M5, macOS 27.0.1).
Dieser Nachtrag folgt auf die unveränderte [Paketvorprüfung](testversion-package-r4.md).
[Strukturierter Beleg](r4-installed-smoke-2026-10-19.json).

## Installationsgrenze

Produktquelle `ee1a6a4f78fca3b45026a8de835e997c914b43ec`, ausführbare Datei
SHA-256 `efea6ec2df880186788ff775b9beea61be8c53cd61c9485c932ca6f043bc83f5`.
Die lokale Entwicklungs-App wurde unter `/Applications/Everlast Dictation.app`
installiert; Strict Codesignverify und erneuter Binärhash stimmen.
Alte App und bisheriger App-Datenbestand wurden separat erhalten. Die neue App
startete mit frischen eigenen App-Daten. Keine alten Modellgewichte übernommen.

Der getrennte Browserdownloadversuch des eigenen R4-ZIP über einen ausschließlich
an Loopback gebundenen Server wurde von Brave mit „Your organization blocked
this file because it didn't meet a security policy“ gesperrt. Keine Umgehung,
kein anderer Browser, keine Änderung an Quarantäne oder Sicherheitsregeln.
Der lokale Server ist beendet. Die anschließende lokale Entwicklungsinstallation
ersetzt ausdrücklich nicht P11s Browser-/Quarantäne-/Installationsabnahme.
Es liegt kein verwertbarer Screenshot der Browsermeldung vor.

## Tatsächlich beobachtet

## Erkannte Abweichung und Grenzen

Erwartet: „Forschung im Bereich KI umfasst die Herstellung von Maschinen zur
Automatisierung von Aufgaben, die intelligentes Verhalten erfordern.“

Tatsächlich: derselbe vollständige Satz, zusätzlich „Vielen Dank.“. Damit ist
keine fehlerfreie Erkennungsqualität dieses akustischen Tests behauptet. Der
Zusatz war auch im früheren R3-Akustiktest beobachtet; Aufnahme über Lautsprecher
und Mikrofon ist kein bitidentischer Dateibackend- oder Korpusvergleich.
Die Ursache des Zusatzes ist mit diesem Nachweis nicht isoliert.
