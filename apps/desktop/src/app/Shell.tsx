import { useEffect, useRef, useState, type ReactNode } from "react";
import { useTranslation } from "react-i18next";
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
    <div className="flex h-screen bg-background text-foreground">
      <Sidebar view={view} onSelect={onSelect} devBuild={devBuild} />
      <div className="flex min-w-0 flex-1 flex-col bg-surface">
        <header
          className={cn(
            "flex h-14 shrink-0 items-center gap-4 border-b border-transparent px-6 wide:px-8",
            scrolled && "border-border",
          )}
        >
          <h1 className="min-w-40 truncate font-display text-display">{title}</h1>
          {/* The view's primary action and the account chip (Task 8) go here. */}
          <div className="ml-auto flex shrink-0 items-center gap-4" data-slot="header-end" />
        </header>
        <div
          ref={scroller}
          className="flex-1 overflow-x-hidden overflow-y-auto px-6 pt-2 pb-6 wide:px-8 wide:pb-8"
        >
          <div ref={sentinel} aria-hidden="true" className="-mt-2 h-px" />
          <main className="max-w-[1120px]">{children}</main>
        </div>
      </div>
    </div>
  );
}
