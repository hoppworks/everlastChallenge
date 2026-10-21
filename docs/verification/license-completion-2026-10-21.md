# Lizenznachtrag — 21.10.2026

## Änderungen

- **FP16:** vollständiger MIT-Originaltext mit den drei ursprünglichen
  Copyrightangaben unverändert aufgenommen. Das vendorte `gguflib/fp16.c` verweist
  ausdrücklich auf dieses Upstreamprojekt. Der konkrete ursprüngliche
  Übernahmecommit ist dort nicht genannt und wird nicht nachträglich erfunden.
- **objc2:** vollständige MIT-, Apache-2.0- und Zlib-Texte aus dem
  [Upstream-Ergänzungscommit](https://github.com/madsmtm/objc2/commit/ee9a7ada2131f5944b8750428e265c15632f2a19)
  aufgenommen. Die bisherigen Lizenzdeklarationen und SDK-Hinweise bleiben erhalten.
  Die Ergänzung ist neuer als die verwendeten Crate-Revisionen; sie ist kein
  Dependencyupdate. Der Maintainer bestätigt in
  [Issue 826](https://github.com/madsmtm/objc2/issues/826) die deklarierten Lizenzen
  älterer Versionen, garantiert aber keine vollständige Liste früherer Mitwirkender.
- **realfft 3.5.0:** unveränderte README-Lizenzdeklaration und Cargo-Autorenmetadaten
  aus dem tatsächlich verwendeten Quellcommit hinzugefügt. Der vollständige
  gepinnte Git-Baum enthält keine LICENSE-/COPYING-Datei. Die vorhandene
  ausdrücklich standardisierte MIT-Ergänzung bleibt erhalten. Eine originale
  Copyrightnotice kann ohne Upstreamquelle nicht ergänzt werden.

Alle zusätzlichen Dateien sind mit URL, Commit, Größe und SHA-256 im
[Lizenznachtrag](../licenses/third-party/license-completion.json) gebunden.
Bestehende Inventare werden als historische Belege erhalten und verweisen auf
diesen Nachtrag. Fremde Copyrightangaben werden nicht umgeschrieben.

## Prüfung und Paket

Die Quelltexte und Abhängigkeitsversionen der Anwendung bleiben unverändert.
Der neue lokale Paketstand unter `artifacts/delivery-licenses-2026-10-21/`
enthält die ergänzten Lizenzressourcen und wurde neu lokal signiert. Die
ursprünglichen Runtimebelege gelten weiter für unveränderten Code; es wird
kein neuer Mikrofon-/Transkriptionslauf behauptet. Das Paketprotokoll bestätigt Lizenzhashes, Signatur und ZIP-Rückextraktion.
Nach Entfernung der Signatur sind die alten und neuen Executable-Dateien bytegleich;
die MLX-Metallib ist unverändert. Das Bundle enthält die vollständige bereinigte
Main-Historie. `npm run check:fast` und die Quell-/Historien-Geheimnisscans sind
erfolgreich. Die installierte App wurde nicht ersetzt.

## Verbleibende Grenze

## Ergänzende Herkunftsprüfung

Die [Quellenkette vom 21.10.2026](fp16-source-chain-2026-10-21.json) belegt:
Die implementierenden Tokens des vendorten FP16-Codes stimmen nach Entfernung
von Kommentaren und Whitespace mit dem ersten gguf-Import vom 27.12.2023 überein.
Der vollständige FP16-Lizenztext im letzten Upstreamstand vor diesem Import ist
bytegleich zum bereits beigefügten Text. Das präzisiert die Herkunft; der konkrete
übernommene FP16-Commit bleibt unbekannt.
