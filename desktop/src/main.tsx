import React, { useEffect } from "react";
import ReactDOM from "react-dom/client";
import { invoke } from "@tauri-apps/api/core";
import { getCurrentWindow } from "@tauri-apps/api/window";

import App from "./App";
import { RenamerPage } from "./components/RenamerPage";
import { watchAppearance } from "./lib/appearance";
import {
  applyWindowClass,
  isImmersiveWindow,
  watchWindowClass,
} from "./lib/windowChrome";
import i18n from "./i18n";
import "./index.css";

document.documentElement.dataset.windowChrome = isImmersiveWindow()
  ? "immersive"
  : "native";
applyWindowClass();

function ThemeBootstrap({ children }: { children: React.ReactNode }) {
  useEffect(() => {
    const stopWindowClass = watchWindowClass();
    let stop = () => {};
    void invoke<{ appearance?: string; accent?: string; uiLocale?: string }>("get_config")
      .then((config) => {
        stop = watchAppearance(config.appearance ?? "system", config.accent ?? "indigo");
        if (config.uiLocale) {
          void i18n.changeLanguage(config.uiLocale);
        }
      })
      .catch(() => {
        stop = watchAppearance("system", "indigo");
      });
    return () => {
      stopWindowClass();
      stop();
    };
  }, []);
  return children;
}

const root = ReactDOM.createRoot(document.getElementById("root") as HTMLElement);
const label = getCurrentWindow().label;

root.render(
  <React.StrictMode>
    <ThemeBootstrap>
      {label === "renamer" ? (
        <div className="flex h-screen flex-col overflow-hidden">
          <RenamerPage />
        </div>
      ) : (
        <App />
      )}
    </ThemeBootstrap>
  </React.StrictMode>,
);
