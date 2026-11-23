import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import { AppRoot } from "@/app/AppRoot";
import { Boot } from "@/app/Boot";
import { installWebviewDefaults } from "@/lib/webviewDefaults";
import { installInputMode } from "@/theme/inputMode";
import "@/styles/index.css";

installInputMode();
installWebviewDefaults();

const root = createRoot(document.getElementById("root")!);

// Dev server only: pages of sample error panels and of the sign-in dialog for the visual check. `import.meta.env.DEV` is
// false in a production build, so the branch and the page are removed from the bundle.
if (import.meta.env.DEV && location.hash === "#error-gallery") {
  void import("@/dev/ErrorGallery").then(({ ErrorGallery }) =>
    root.render(
      <StrictMode>
        <ErrorGallery />
      </StrictMode>,
    ),
  );
} else if (import.meta.env.DEV && location.hash.startsWith("#sign-in-gallery")) {
  void import("@/dev/SignInGallery").then(({ SignInGallery }) =>
    root.render(
      <StrictMode>
        <SignInGallery />
      </StrictMode>,
    ),
  );
} else {
  root.render(
    <StrictMode>
      <AppRoot>
        <Boot />
      </AppRoot>
    </StrictMode>,
  );
}
