import { createRoot } from "react-dom/client";

const root = document.getElementById("root");
if (root) {
  createRoot(root).render(
    <main>
      <h1>Everlast Dictation</h1>
      <p>Die Diktierfunktion wird eingerichtet.</p>
    </main>,
  );
}
