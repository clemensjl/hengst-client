import { InfoIcon } from "lucide-react";
import type { ReactNode } from "react";
import { api } from "@/ipc/client";
import type { UrlTarget } from "@/ipc/bindings/UrlTarget";
import { useState } from "react";

/**
 * DESIGN.md 11.6: not an error. The `info` icon in the muted color, then the title and the text.
 * Shared by "Sign-in is not available yet" (a block in the sign-in dialog) and "This build cannot
 * sign in" (its own dialog), so both look alike. `title` is the heading of the block or of the dialog.
 */
export function InfoBlock({ title, children }: { title: ReactNode; children: ReactNode }) {
  return (
    <div className="flex items-start gap-3">
      <InfoIcon
        className="mt-[5px] size-[18px] shrink-0 text-muted-foreground"
        aria-hidden="true"
      />
      <div className="flex min-w-0 flex-col gap-2">
        {title}
        {children}
      </div>
    </div>
  );
}

/**
 * Opens one of the fixed pages in the browser. `failed` is set when the system refused, so the
 * dialog can say so beside the button instead of the click doing nothing.
 */
export function useOpenUrl(target: UrlTarget): { open: () => void; failed: boolean } {
  const [failed, setFailed] = useState(false);
  return {
    failed,
    open: () => {
      setFailed(false);
      api.openUrl(target).catch(() => setFailed(true));
    },
  };
}
