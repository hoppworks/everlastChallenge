# Native Prüfung von Version 0.2.0

Der lokal signierte Releasebuild wurde aus
`36b453258b148f687ca1f95d89251af0fc51ffa0` mit dem aktualisierten npm-Lockfile
gebaut. `Info.plist` meldet 0.2.0; Bundle-Signatur und Mikrofonentitlement wurden
geprüft. Plattform: Apple Silicon, macOS 27.0.1. Eine Apple-Notarisierung liegt
nicht vor.

**Nachprüfung des Downloads:** Gatekeeper weist diese lokal signierte App
zurück. Der gültige Bundle-Signaturcheck bedeutet keine Gatekeeper-Zulassung.
[Diagnose und Prüfgrenze](gatekeeper-2026-10-22.md) sowie
[einmalige manuelle Freigabe](../usage.md#macos-blockiert-das-erste-öffnen)
beschreiben den tatsächlichen Installationsweg.

## Beobachtete Funktion

- Eine gespeicherte Modellwahl `reference` wurde beim Start auf
  `mlx_reference_encoder` migriert. Alle übrigen Einstellungswerte blieben
  unverändert. Der Modellbereich zeigte ausschließlich den bereiten Hybrid;
  es gab keine Modellauswahl.
- Im finalen App-Bundle wurden Einstellungen, Rücknavigation und die native
  Aufnahme geprüft. Die App wechselte von Bereit zu Aufnahme, dann nach Stop
  zur lokalen Verarbeitung und zurück zum fertigen Diktat. Der Pegel reagierte
  auf das Mikrofon. Der beim Test erzeugte Text wurde anschließend entfernt.
- Der Inhalt dieses Mikrofontests wurde nicht als Qualitätsreferenz bewertet.
  Die Eingabe war keine kontrollierte Audiofixture. Rohaufnahme und Text sind
  nicht Bestandteil des öffentlichen Nachweises.
- Die gestartete App-Kopie meldete Mikrofonzugriff, aber keine Freigabe für
  Bedienungshilfen. Automatisches Einfügen blieb entsprechend sichtbar gesperrt.
  Dieser Lauf belegt keinen physischen globalen Hotkey und keinen externen
  Textempfang. Frühere Empfangsbelege bleiben an ihre damaligen Builds gebunden.

## Modelltransport und Erkennung

Ein frisches Verzeichnis erhielt das VAD-Modell über den produktiven
HTTPS-Downloadpfad; Größen- und Hashprüfung waren erfolgreich. Es wurde kein
vorhandener Modellcache als Download ausgegeben.

Drei öffentliche deutsche FLEURS-Fixtures (1736, 1663, 1668; 5/10/15 Sekunden)
wurden mit dem tatsächlichen Hybridpfad verarbeitet. Alle drei Aufträge endeten
mit `ok`. Diese endlichen Modellproben sind kein Nachweis fehlerfreier
Erkennung oder eines Geschwindigkeitsvorsprungs.

Diese Modellproben stammen aus dem aktualisierten nativen Quellstand
`a8be0a44a96c0cff23fa2370d93ceb92d6d8abc3`. Cargo-Lockfile, Rustquellen,
Vendorquellen, Modellkonfiguration und Rust-Toolchain sind gegenüber dem oben
benannten Release-Quellstand unverändert. Das wurde per Git-Diff überprüft.
Das spätere npm-Update erforderte einen frischen Frontend-/Releasebuild, keinen
zweiten unveränderten Modelllauf. Es wird keine neue Zeit- oder
Geschwindigkeitsvergleichsaussage abgeleitet.

## Paket und Nachvollziehbarkeit

Das [Release](https://github.com/hoppworks/everlastChallenge/releases/tag/v0.2.0)
enthält ZIP, `SHA256SUMS`, `build-receipt.json` und die vollständige
Main-Historie als Git-Bundle. Der maschinenlesbare Beleg bindet Quellcommit,
Lockfiles, jede Bundle-Datei, ZIP und Historie an SHA-256. Vor dem Verpacken
werden unveränderte Buildinputs gegenüber dem Quellcommit geprüft. Die ZIP
wird erneut entpackt; Signaturen sowie die Bytegleichheit sämtlicher Dateien
werden am entpackten Paket kontrolliert.

Die [Softwareprüfung](tooling-and-packages-2026-10-22.md) dokumentiert Umfang,
Fehlversuche, gezielte Wiederholungen und ausgelassene modellabhängige Tests.
Kein lokaler Testlauf wird als erfolgreicher Remote-CI-Lauf ausgegeben.
