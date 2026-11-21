import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import { Boot } from "@/app/Boot";
import { installWebviewDefaults } from "@/lib/webviewDefaults";
import { installInputMode } from "@/theme/inputMode";
import "@/styles/index.css";

installInputMode();
installWebviewDefaults();

createRoot(document.getElementById("root")!).render(
  <StrictMode>
    <Boot />
  </StrictMode>,
);
