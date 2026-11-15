import React from "react";
import ReactDOM from "react-dom/client";
import App from "./App";

async function start() {
  // Plain browser during development: fake the backend so the panel can be previewed.
  if (import.meta.env.DEV && !("__TAURI_INTERNALS__" in window)) {
    const { installMocks } = await import("./dev-mocks");
    installMocks();
  }
  ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
    <React.StrictMode>
      <App />
    </React.StrictMode>,
  );
}

void start();
