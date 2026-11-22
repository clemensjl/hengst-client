import { useEffect, useRef, useState, type ReactNode } from "react";
import { useTranslation } from "react-i18next";
import { AccountChip } from "@/features/accounts/AccountChip";
import { cn } from "@/lib/utils";
import { Sidebar } from "./Sidebar";
import { VIEW_ITEMS, type View } from "./views";

interface Props {
  view: View;
  onSelect: (view: View) => void;
  devBuild: boolean;
  children: ReactNode;
}

/** True while the content below the sentinel is scrolled away (the header then gets its line). */
function useScrolled(
  root: React.RefObject<HTMLElement | null>,
  sentinel: React.RefObject<HTMLElement | null>,
) {
  const [scrolled, setScrolled] = useState(false);
  useEffect(() => {
    if (!root.current || !sentinel.current || typeof IntersectionObserver !== "function") return;
    const observer = new IntersectionObserver(([entry]) => setScrolled(!entry.isIntersecting), {
      root: root.current,
    });
    observer.observe(sentinel.current);
    return () => observer.disconnect();
  }, [root, sentinel]);
  return scrolled;
}

/** DESIGN.md 7.2: sidebar, then the content column with its 56 px header and the scrolling view. */
export function Shell({ view, onSelect, devBuild, children }: Props) {
  const { t } = useTranslation();
  const scroller = useRef<HTMLDivElement>(null);
  const sentinel = useRef<HTMLDivElement>(null);
  const scrolled = useScrolled(scroller, sentinel);
  const title = t(VIEW_ITEMS.find((i) => i.view === view)!.labelKey);
  return (
    <div className="flex h-dvh bg-background text-foreground">
      <Sidebar view={view} onSelect={onSelect} devBuild={devBuild} />
      <div className="flex min-w-0 flex-1 flex-col bg-surface">
        <header
          className={cn(
            "flex min-h-14 shrink-0 items-center gap-4 border-b px-6 py-3 wide:px-8",
            scrolled ? "border-border" : "border-transparent",
          )}
        >
          {/* A long title wraps to a second line (the header grows) instead of being cut: a tooltip
              would not be reachable by keyboard (DESIGN.md 10.15). */}
          <h1 className="min-w-40 font-display text-display">{title}</h1>
          {/* The view's primary action (Task 11) goes here, then the account chip. */}
          <div className="ml-auto flex shrink-0 items-center gap-4" data-slot="header-end">
            <AccountChip />
          </div>
        </header>
        <div
          ref={scroller}
          className="flex-1 overflow-x-hidden overflow-y-auto px-6 pb-6 wide:px-8 wide:pb-8"
        >
          {/* 1 px at the very top that takes no space (-mb-px): once it is scrolled out of view the
              header gets its line. The 8 px from header to content are the main's own padding. */}
          <div ref={sentinel} aria-hidden="true" className="-mb-px h-px" />
          <main className="max-w-[1120px] pt-2">{children}</main>
        </div>
      </div>
    </div>
  );
}
