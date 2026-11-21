import { Boxes, Settings, type LucideIcon } from "lucide-react";

/** 4b adds "play" and "log". */
export type View = "instances" | "settings";

export const START_VIEW: View = "instances";

export interface ViewItem {
  view: View;
  /** i18n key of the navigation label and the page title. */
  labelKey: "nav.instances" | "nav.settings";
  icon: LucideIcon;
  /** Pinned to the bottom of the sidebar. */
  bottom?: boolean;
}

export const VIEW_ITEMS: ViewItem[] = [
  { view: "instances", labelKey: "nav.instances", icon: Boxes },
  { view: "settings", labelKey: "nav.settings", icon: Settings, bottom: true },
];
