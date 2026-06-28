import React from "react";
import ReactDOM from "react-dom/client";
import App from "./App";
import "./index.css";
import {
  applyDarkClass,
  applyInterfaceRootFont,
  DEFAULT_APPEARANCE,
  readInterfaceScaleHint,
  readThemeHint,
  resolveDark,
  systemPrefersDark,
} from "@/lib/appearance";

// Pre-paint: apply the last chosen theme and interface scale synchronously from the webview-local
// hints (theme falling back to System → the OS preference) before React mounts, so an explicit
// Light/Dark choice never flashes the OS theme and a non-medium scale never reflows on cold start.
// AppearanceProvider becomes the sole runtime authority once mounted — it follows OS changes and
// the persisted record.
applyDarkClass(resolveDark(readThemeHint() ?? DEFAULT_APPEARANCE.theme, systemPrefersDark()));
applyInterfaceRootFont(readInterfaceScaleHint() ?? DEFAULT_APPEARANCE.interface_font_scale);

ReactDOM.createRoot(document.getElementById("root")!).render(
  <React.StrictMode>
    <App />
  </React.StrictMode>,
);
