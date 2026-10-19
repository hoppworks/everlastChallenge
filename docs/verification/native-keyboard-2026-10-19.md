# Native Tastaturverträge, 2026-10-19

Bestanden: Unsichtbare eigene NSWindows verarbeiten echte AppKit-NSEvents über `performKeyEquivalent`. Die produktiven NSButtons bieten ⌘R für Start/Stop, Esc für Cancel, ⌘, für Einstellungen und ⌘⇧C für den letzten Text. Gewöhnliches ⌘C bleibt dem nativen Auswahl-Responder vorbehalten. Tooltip-Texte machen Aufnahme- und Kopierkürzel auffindbar.

Die Header-Prüfung beobachtet Start, Stop und Cancel mit den richtigen Sitzungs-IDs 17/18 sowie Einstellungen; einfaches `r`, deaktivierte Einstellungen und verborgene Aufnahmeaktionen lösen keine Aktion aus. Die Workspace-Prüfung beobachtet die Kopieraktion mit genau dem angezeigten Text. Callback-Kanäle ersetzen in diesen Verträgen die auslösenden Dienste: keine Aufnahme und keine Zwischenablagenänderung.

Die fokussierten nativen Tests und striktes Clippy bestanden. Der unabhängige Source-Review meldete keine Befunde; danach ergänzte Root die ausgeführten ⌘,- und Negativfälle. Die erste Aufnahme-/Kopierprüfung schlug vor den Mappings fehl. Für ⌘⇧C verlangt AppKit hier das Großbuchstaben-KeyEquivalent `C`; der frühere Kleinbuchstabenversuch war erfolglos und bleibt im Rohlog erhalten.

Diese Ebene-N-Belege beweisen keine physische Tastenzustellung, keinen globalen ⌥D-Start und keine sichtbare Tab-Reihenfolge/Fokusdarstellung. Dafür gelten die separaten Fokusverträge, verfügbare Sichtprüfung und der eine gemeinsame G-Lauf. Die Kürzel ersetzen keine Prüfungen der produktiven Zustands- und Sitzungsbarrieren.

[Hashes und genaue Beleggrenzen](native-keyboard-2026-10-19.json). Rohlogs: `artifacts/native-keyboard-clipboard-final.log` und `artifacts/native-keyboard-header-expanded.log`.
