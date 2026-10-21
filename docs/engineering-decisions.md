# Technischer Einstieg für Reviewer

Die Aufgabe ist eine kleine Diktier-App. Die schwierigsten Entscheidungen liegen
an den Übergängen: vom Audio-Callback zur Verarbeitung, von einer Sitzung zur
nächsten und vom eigenen Prozess zu einem fremden Textfeld. Die Tabelle verbindet
jede Entscheidung mit dem überprüfbaren Vertrag. Der detaillierte
[Code-Wegweiser](code-map.md) ergänzt die Einstiegspunkte.

| Entscheidung | Grund | Einstieg und Nachweis |
|---|---|---|
| Kleine WebView, native Funktionspfade | TypeScript erfüllt die UI-Vorgabe; Aufnahme und Einfügen bleiben systemnah | `src/main.tsx`, `src/ipc/`, `src-tauri/src/audio.rs`; IPC-Tests und Importgraph |
| Getrennter Sitzungskern | Alte Ergebnisse dürfen kein neues Diktat überschreiben | `crates/dictation-core/src/session.rs`, `src-tauri/src/controller.rs`; Sitzungs- und Controller-Verträge |
| Begrenzter Ring und Arbeitswarteschlangen | Langsame Inferenz darf den Aufnahme-Callback nicht blockieren oder unbegrenzt Speicher ansammeln | `audio.rs`, `inference_service.rs`; Overflow-, Flush- und Shutdown-Tests |
| Ein Besitzer für das Modell | MLX-Objekte werden auf demselben Thread erzeugt, benutzt und freigegeben | `inference_service.rs`, `transcription/`; Worker-/Modellgeneration-Verträge |
| Cancel entzieht die Wirkung | Ein bereits laufender GPU-Kernel kann enden, sein Ergebnis darf danach nicht eingefügt werden | `runtime.rs`, `controller.rs`; verspätete Ergebnisse und Abbruchrennen |
| Zielidentität vor der Aufnahme erfassen | Ein Fokuswechsel darf keinen Text in das falsche Fenster schicken | `native_service.rs`, `crates/macos-integration/src/target/`; Zielwechsel- und Delivery-Verträge |
| Kein automatischer zweiter Paste-Versuch | Nach irreversiblem Versand ist unbekannter Empfang kein sicherer Grund für einen zweiten Text | `delivery.rs`, `paste.rs`; Commit-/Cleanup-Verträge |
| Modellbytes bei jedem Laden prüfen | Dateiname, Größe oder mtime allein begründen kein Vertrauen | `model/`; Chunk-Integritätsprüfung und Mutationsfälle |
| Hybrid nach Qualitätsbefund | Ein schnellerer Decoder ist kein Fortschritt, wenn er Aussagen verändert | [MLX-Entscheidung](verification/full-mlx-decision-2026-10-21/README.md), [Zusatzversuch](verification/pure-mlx-screen-2026-10-21/README.md) |

## Ein konkreter behobener Fehler

Ein Timeout direkt nach dem nativen Paste-Versand konnte die Zwischenablage zu
früh wiederherstellen. Die Lesefrist von 500 ms gehört jetzt zum tatsächlichen
Versand, nicht erst zur Bestätigung an den wartenden Worker. Frühes Cleanup
behält den Besitzer bis zur Frist; das Warten erfolgt außerhalb des Hauptthreads.
Der Regressionstest ordnet Versand, Timeout und Cleanup kontrolliert. Die 500 ms
sind eine Policy, keine Empfangsgarantie jeder fremden Anwendung.
[Review-Auflösung und Grenzen](review-resolution-2026-10-21.md).

## Ein bewusst verworfener Weg

Reines MLX-ASR wurde untersucht. Der zusätzliche kleine Versuch war als ganzer
Prozess schneller, zeigte aber höheren Spitzen-RAM und einen bekannten
Bedeutungsfehler. Deshalb ist der Hybrid der freigegebene Produktpfad; weitere
Forschung läuft getrennt. Historische Programme bleiben zur Reproduktion
vorhanden, experimentelle FullPrefix-Auswahl gehört nicht zum normalen Build.

## Was eine grüne Prüfung tatsächlich aussagt

[Ausführbare Regeln](guardrails.md) erzwingen die Modulgrenzen und zentrale
Fehlerbehandlung. [Der Vorlagenabgleich](guardrail-crosswalk.md) nennt auch die
bewussten Abweichungen. Komponenten-/Adaptertests beweisen ihre jeweiligen
Verträge; die physische Hotkey-Prüfung beweist den beobachteten realen Ablauf.
Beides ersetzt weder alle macOS-Anwendungen noch alle denkbaren Audios.
Die [Leistungsübersicht](performance.md) benennt deshalb jeweils Bezugssystem,
Messstrecke und Grenzen. Diese Nachweise sollen technische Entscheidungen
prüfbar machen, nicht pauschale technical itäts- oder Produktreifeetiketten ersetzen.
