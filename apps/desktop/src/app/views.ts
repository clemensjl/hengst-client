import { Boxes, House, ScrollText, Settings, type LucideIcon } from "lucide-react";

export type View = "play" | "instances" | "log" | "settings";

/** DESIGN.md 7.2: from 4b the start page is the start view. */
export const START_VIEW: View = "play";

export interface ViewItem {
  view: View;
  /** i18n key of the navigation label and the page title. */
  labelKey: "nav.play" | "nav.instances" | "nav.log" | "nav.settings";
  icon: LucideIcon;
  /** Pinned to the bottom of the sidebar. */
  bottom?: boolean;
}

export const VIEW_ITEMS: ViewItem[] = [
  { view: "play", labelKey: "nav.play", icon: House },
  { view: "instances", labelKey: "nav.instances", icon: Boxes },
  { view: "log", labelKey: "nav.log", icon: ScrollText },
  { view: "settings", labelKey: "nav.settings", icon: Settings, bottom: true },
];
