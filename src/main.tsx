import React from "react";
import ReactDOM from "react-dom/client";
import App from "./App";
import "@stillmvd/tauri-ship/ship.css";
import "./styles.css";

document.addEventListener("pointerup", (e) => {
  const control = (e.target as Element | null)?.closest<HTMLElement>(
    'button, a, input[type="range"], input[type="checkbox"], [role="switch"], [role="radio"], [role="menuitem"]',
  );
  if (control) window.setTimeout(() => control.blur(), 0);
});

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <App />
  </React.StrictMode>,
);
