import { createRoot } from "react-dom/client";
import { App } from "./App";
import "./features/app/styles.css";
const root = document.getElementById("root");
if (!root) throw new Error("Application root is unavailable");
createRoot(root).render(<App />);
