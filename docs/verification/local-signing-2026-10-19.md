# Lokale Signierung ohne Developer-Account, 19.10.2026

Zwei unterschiedlich kompilierte kleine Testprogramme wurden gültig signiert, ohne Ausführung. Anschließend zwei vollständige Everlast-App-Builds aus Produktquelle40e7eaf: A mit Konfigurationsversion0.1.0, B mit0.1.1. Unterschiedliche Codehashes, identischer Bundle-Identifier und identische Designated Requirement; beide strikte Signaturprüfung und Mikrofondeklaration/Entitlement bestanden. [Strukturierte Belege](local-signing-2026-10-19.json).

`npm run release:local` baut künftig die App und verwendet die private lokale Konfiguration `.everlast-signing.json` mit ausschließlich dem Zertifikatsfingerprint. Keine Schlüssel im Repo. `sign-local-bundle.mjs` prüft Konfiguration, App-ID, einzigen Mach-O-Hauptprozess, Ressourcenstruktur und Entitlements vor der Signierung; danach erneute Bundleprüfung. Reale Signierung von B mit diesem Helper bestanden.

`verify-signing-continuity.mjs` prüft tatsächliche Signaturen, Identifier, Requirement und geänderten Codehash. Metadatenparsing allein gilt nicht als Signaturprüfung; Ad-hoc, fehlende Requirement oder unveränderter Code scheitern. `permissionsPreserved` bleibt unbekannt. Optionsartige Pfade werden als Pfade normalisiert und als ungültige Bundleverzeichnisse abgelehnt. Neun fokussierte neue Node-Tests bestanden, unabhängiger kombinierter Review ohne verbleibende Befunde. Erweiterter autonomer Lauf: neun Schritte,84Rusttests und15Node-Einstiegstests bestanden; keine neue ASR-Qualifikation.

Eigener Testbeobachter mit automatischem Zustandsbericht ist lokal gebaut und signiert, noch nicht gestartet. Echte Fokus-/Empfangsabnahme bleibt offen. Keine neuen formalen Paketabschlüsse.
