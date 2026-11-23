import { Toaster as Sonner, type ToasterProps } from "sonner";
import { CheckIcon, CircleAlertIcon, InfoIcon, TriangleAlertIcon } from "lucide-react";

/* DESIGN.md 10.10: bottom right, 24 px from the edges, 356 wide, at most 3 visible, 4 s. The
   colors are tokens; the floating-layer outline is --border in light and --border-strong in dark.
   Sonner keeps its own easing and timing. */
const Toaster = ({ theme = "light", ...props }: ToasterProps) => {
  return (
    <Sonner
      theme={theme}
      position="bottom-right"
      offset={24}
      mobileOffset={24}
      visibleToasts={3}
      duration={4000}
      icons={{
        success: <CheckIcon className="size-4 text-success" aria-hidden="true" />,
        info: <InfoIcon className="size-4 text-muted-foreground" aria-hidden="true" />,
        warning: <TriangleAlertIcon className="size-4 text-warning" aria-hidden="true" />,
        error: <CircleAlertIcon className="size-4 text-danger" aria-hidden="true" />,
      }}
      style={
        {
          "--width": "356px",
          "--normal-bg": "var(--surface-raised)",
          "--normal-text": "var(--text)",
          "--normal-border": theme === "dark" ? "var(--border-strong)" : "var(--border)",
          "--border-radius": "var(--radius-panel)",
        } as React.CSSProperties
      }
      toastOptions={{
        classNames: {
          toast: "px-4! py-3! shadow-md! font-sans!",
          title: "text-label! text-foreground!",
          description: "text-caption! text-muted-foreground!",
        },
      }}
      {...props}
    />
  );
};

export { Toaster };
