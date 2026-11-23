import { Boxes, House, Settings, type LucideIcon } from "lucide-react";

/** 4b adds "log" with the log view (Task 19). */
export type View = "play" | "instances" | "settings";

/** DESIGN.md 7.2: from 4b the start page is the start view. */
export const START_VIEW: View = "play";

export interface ViewItem {
  view: View;
  /** i18n key of the navigation label and the page title. */
  labelKey: "nav.play" | "nav.instances" | "nav.settings";
  icon: LucideIcon;
  /** Pinned to the bottom of the sidebar. */
  bottom?: boolean;
}

export const VIEW_ITEMS: ViewItem[] = [
  { view: "play", labelKey: "nav.play", icon: House },
  { view: "instances", labelKey: "nav.instances", icon: Boxes },
  { view: "settings", labelKey: "nav.settings", icon: Settings, bottom: true },
];
