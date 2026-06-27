import React from "react";
import ReactDOM from "react-dom/client";
import App from "./App";
import "./index.css";
import { applyDarkClass, readThemeHint, resolveDark, systemPrefersDark } from "@/lib/appearance";

// Pre-paint: apply the last chosen theme synchronously from the webview-local hint (falling
// back to System → the OS preference) before React mounts, so an explicit Light/Dark choice
// never flashes the OS theme on cold start. AppearanceProvider becomes the sole runtime
// authority once mounted — it follows OS changes and the persisted record.
applyDarkClass(resolveDark(readThemeHint() ?? "system", systemPrefersDark()));

ReactDOM.createRoot(document.getElementById("root")!).render(
  <React.StrictMode>
    <App />
  </React.StrictMode>,
);
