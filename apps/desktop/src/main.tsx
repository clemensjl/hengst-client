import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import { Boot } from "@/app/Boot";
import { installWebviewDefaults } from "@/lib/webviewDefaults";
import { installInputMode } from "@/theme/inputMode";
import "@/styles/index.css";

installInputMode();
installWebviewDefaults();

const root = createRoot(document.getElementById("root")!);

// Dev server only: a page of sample error panels for the visual check. `import.meta.env.DEV` is
// false in a production build, so the branch and the page are removed from the bundle.
if (import.meta.env.DEV && location.hash === "#error-gallery") {
  void import("@/dev/ErrorGallery").then(({ ErrorGallery }) =>
    root.render(
      <StrictMode>
        <ErrorGallery />
      </StrictMode>,
    ),
  );
} else {
  root.render(
    <StrictMode>
      <Boot />
    </StrictMode>,
  );
}
