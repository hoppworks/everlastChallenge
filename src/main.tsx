// The installed macOS app renders its workspace and preferences with AppKit.
// Keep the retained Wry root inert: hidden React forms must not steal native focus.
import "./features/app/styles.css";
const root = document.getElementById("root");
if (root) root.textContent = "Everlast Dictation wird gestartet …";
