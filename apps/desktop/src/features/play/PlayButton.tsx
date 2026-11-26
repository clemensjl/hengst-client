import {
  BoxesIcon,
  LoaderCircleIcon,
  LogInIcon,
  PlayIcon,
  PlusIcon,
  SquareIcon,
  UserRoundIcon,
  XIcon,
  type LucideIcon,
} from "lucide-react";
import { type Ref } from "react";
import { useTranslation } from "react-i18next";
import { Button } from "@/components/ui/button";
import { cn } from "@/lib/utils";
import { useChangeGuard } from "./changeGuard";
import type { PlayButtonState } from "./playState";

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
    case "chooseAccount":
      return { label: t("accounts.choose"), variant: "primary", icon: UserRoundIcon };
    case "signInAgain":
      return { label: t("accounts.needsSignIn"), variant: "primary", icon: LogInIcon };
    case "ready":
      return { label: t("play.button.play"), variant: "primary", icon: PlayIcon };
  }
}

const REASON_ID = "play-button-reason";

/** The states that undo what the click before started: the click guard arms on these. */
const OPPOSITE: PlayButtonState["kind"][] = ["preparing", "running"];

/** A game is on its way, runs or ends: leaving these for Play (or any other action) arms the guard too. */
const BUSY: PlayButtonState["kind"][] = ["starting", "preparing", "running", "stopping"];

/**
 * DESIGN.md 10.2: one button whose label, variant and icon follow the state; it never moves,
 * never changes height and is never `disabled`. An inert state keeps the button focusable with
 * `aria-disabled` and its reason as the description. When the button turns into the opposite
 * action (Play into Cancel or Stop, and Cancel or Stop back into Play when the start ends or the
 * game exits) a click within 600 ms is ignored, so the second click of a double click never hits
 * it (DESIGN.md 10.0); any other change (Sign in into Play) takes the next click at once.
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
  const guarded = useChangeGuard(
    state.kind,
    (left, now) => OPPOSITE.includes(now) || (BUSY.includes(left) && !BUSY.includes(now)),
  );

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
          if (look.reason || guarded()) return;
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
