import {
  BoxesIcon,
  LoaderCircleIcon,
  LogInIcon,
  PlayIcon,
  PlusIcon,
  SquareIcon,
  XIcon,
  type LucideIcon,
} from "lucide-react";
import { useLayoutEffect, useRef, type Ref } from "react";
import { useTranslation } from "react-i18next";
import { Button } from "@/components/ui/button";
import { cn } from "@/lib/utils";
import type { PlayButtonState } from "./playState";

/** DESIGN.md 10.0: after the button turned into another action, clicks are ignored this long. */
export const CHANGE_GUARD_MS = 600;

type Look = {
  label: string;
  variant: "primary" | "secondary";
  icon: LucideIcon | "spinner";
  /** Inert: the label says what happens, the click does nothing (never `disabled`). */
  reason?: string;
};

function lookOf(state: PlayButtonState, t: (k: string) => string): Look {
  switch (state.kind) {
    case "stopping":
      return {
        label: t("play.button.stopping"),
        variant: "secondary",
        icon: "spinner",
        reason: t("play.button.stoppingReason"),
      };
    case "running":
      return { label: t("play.button.stop"), variant: "secondary", icon: SquareIcon };
    case "starting":
      return {
        label: t("play.button.starting"),
        variant: "primary",
        icon: "spinner",
        reason: t("play.button.startingReason"),
      };
    case "preparing":
      return { label: t("common.cancel"), variant: "secondary", icon: XIcon };
    case "noInstance":
      return { label: t("instances.create"), variant: "primary", icon: PlusIcon };
    case "chooseInstance":
      return { label: t("play.button.choose"), variant: "primary", icon: BoxesIcon };
    case "signIn":
      return { label: t("accounts.signInButton"), variant: "primary", icon: LogInIcon };
    case "signInAgain":
      return { label: t("accounts.needsSignIn"), variant: "primary", icon: LogInIcon };
    case "ready":
      return { label: t("play.button.play"), variant: "primary", icon: PlayIcon };
  }
}

const REASON_ID = "play-button-reason";

/**
 * DESIGN.md 10.2: one button whose label, variant and icon follow the state; it never moves,
 * never changes height and is never `disabled`. An inert state keeps the button focusable with
 * `aria-disabled` and its reason as the description. After every change of the state a click
 * within 600 ms is ignored, so the second click of a double click never hits the new action.
 */
export function PlayButton({
  state,
  onActivate,
  buttonRef,
}: {
  state: PlayButtonState;
  onActivate: (state: PlayButtonState, button: HTMLElement) => void;
  buttonRef?: Ref<HTMLButtonElement>;
}) {
  const { t } = useTranslation();
  const look = lookOf(state, t);
  const kind = useRef(state.kind);
  const changedAt = useRef(-Infinity);
  // Before the browser paints the new label, so no click can reach it unguarded.
  useLayoutEffect(() => {
    if (kind.current !== state.kind) {
      kind.current = state.kind;
      changedAt.current = Date.now();
    }
  }, [state.kind]);

  const Icon = look.icon;
  return (
    <>
      <Button
        ref={buttonRef}
        size="lg"
        variant={look.variant}
        aria-disabled={look.reason ? true : undefined}
        aria-describedby={look.reason ? REASON_ID : undefined}
        // Inert states show no press: only an action presses.
        className={cn(!look.reason && "play-press")}
        onClick={(e) => {
          if (look.reason || Date.now() - changedAt.current < CHANGE_GUARD_MS) return;
          onActivate(state, e.currentTarget);
        }}
      >
        {Icon === "spinner" ? (
          <LoaderCircleIcon className="spinner" aria-hidden="true" />
        ) : (
          <Icon aria-hidden="true" />
        )}
        {look.label}
      </Button>
      {look.reason && (
        <span id={REASON_ID} className="sr-only">
          {look.reason}
        </span>
      )}
    </>
  );
}
