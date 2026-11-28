import { CircleAlertIcon, RefreshCwIcon } from "lucide-react";
import { Component, lazy, Suspense, useState, type ComponentType, type ReactNode } from "react";
import { useTranslation } from "react-i18next";
import { Button } from "@/components/ui/button";

/**
 * The log view and its list library are their own chunk (they stay out of the start). The app
 * asks for it right after it starts; once it is there the view renders at once, without the
 * one empty frame a lazy component shows on its first render.
 */
type Module = { LogView: ComponentType };
let loaded: Module | null = null;
const load = (): Promise<Module> =>
  import("./LogView").then((m) => {
    loaded = m;
    return m;
  });

export function preloadLogView(): void {
  // A failure shows when the view is opened (the boundary below).
  load().catch(() => {});
}

const lazyView = () => lazy(() => load().then((m) => ({ default: m.LogView })));

/** Catches a chunk that cannot be loaded (or an error of the view), so the app stays. */
class ChunkBoundary extends Component<{ fallback: ReactNode; children: ReactNode }> {
  state = { failed: false };
  static getDerivedStateFromError() {
    return { failed: true };
  }
  render() {
    return this.state.failed ? this.props.fallback : this.props.children;
  }
}

export function LogRoute() {
  const { t } = useTranslation();
  // A failed lazy component keeps its rejection: "Try again" puts a fresh one in place.
  const [attempt, setAttempt] = useState(0);
  const [View, setView] = useState(lazyView);
  if (loaded) return <loaded.LogView />;
  const failure = (
    <div
      role="alert"
      className="flex max-w-120 flex-col gap-3 rounded-lg border bg-surface-raised p-4"
    >
      <div className="flex items-start gap-3">
        <CircleAlertIcon className="mt-[3px] size-[18px] shrink-0 text-danger" aria-hidden="true" />
        <div className="flex min-w-0 flex-col">
          <p className="text-body font-semibold">{t("log.loadFailed.title")}</p>
          <p className="text-small text-muted-foreground">{t("log.loadFailed.body")}</p>
        </div>
      </div>
      <div className="pl-[30px]">
        <Button
          onClick={() => {
            setView(lazyView);
            setAttempt((n) => n + 1);
          }}
        >
          <RefreshCwIcon aria-hidden="true" />
          {t("common.retry")}
        </Button>
      </div>
    </div>
  );
  return (
    <ChunkBoundary key={attempt} fallback={failure}>
      <Suspense fallback={null}>
        <View />
      </Suspense>
    </ChunkBoundary>
  );
}
