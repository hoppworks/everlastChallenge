# Öffentliche, nicht vertrauenswürdige TLS-Prüfdaten

Zertifikat und PKCS#8-Schlüssel wurden lokal für Download-Regressionstests über
die Loopback-Schnittstelle erzeugt. Der Schlüssel ist absichtlich öffentliches
Testmaterial, kein Zugangsschlüssel. Nur der Testclient für die positive
Kontrolle vertraut diesem Zertifikat. Es wird weder im System als
vertrauenswürdig installiert noch mit der App ausgeliefert.

Das Zertifikat enthält die SAN-Einträge DNS localhost und IP 127.0.0.1 sowie die
Verwendung serverAuth. Die Tests zeigen zunächst, dass der normale Produktclient
es vor der Veröffentlichung von Bytes ablehnt. Anschließend vertraut ein reiner
Testclient genau diesem Zertifikat und bestätigt den erfolgreichen Download vom
selben Server. Das schützt davor, die Zertifikatsprüfung unbemerkt abzuschalten.
