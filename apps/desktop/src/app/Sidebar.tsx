import { useTranslation } from "react-i18next";
import mark from "../../brand/mark.svg";
import { Badge } from "@/components/Badge";
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip";
import { useWide } from "@/lib/useMediaQuery";
import { cn } from "@/lib/utils";
import { VIEW_ITEMS, type View, type ViewItem } from "./views";

interface Props {
  view: View;
  onSelect: (view: View) => void;
  devBuild: boolean;
}

/** DESIGN.md 7.2: 216 px from 1200 px wide, a 64 px icon rail below; names stay in the element tree. */
export function Sidebar({ view, onSelect, devBuild }: Props) {
  const { t } = useTranslation();
  const wide = useWide();
  const item = (it: ViewItem) => {
    const label = t(it.labelKey);
    const active = it.view === view;
    const Icon = it.icon;
    const button = (
      <button
        type="button"
        aria-label={label}
        aria-current={active ? "page" : undefined}
        onClick={() => onSelect(it.view)}
        className={cn(
          "mx-auto flex h-10 w-10 items-center justify-center gap-3 rounded-md px-0 text-small text-muted-foreground transition-[background-color,color] duration-(--dur-press) ease-out hover:bg-hover hover:text-foreground active:bg-selected active:duration-0 wide:w-full wide:justify-start wide:px-3",
          active && "bg-selected font-semibold text-foreground",
        )}
      >
        <Icon className={cn("size-[18px] shrink-0", active && "text-primary")} aria-hidden="true" />
        <span className="hidden wide:inline">{label}</span>
      </button>
    );
    if (wide) return <div key={it.view}>{button}</div>;
    return (
      <Tooltip key={it.view}>
        <TooltipTrigger render={button} />
        <TooltipContent side="right">{label}</TooltipContent>
      </Tooltip>
    );
  };

  return (
    <aside className="flex w-16 shrink-0 flex-col border-r bg-background wide:w-54">
      <div className="flex h-14 shrink-0 items-center justify-center gap-2 wide:justify-start wide:px-3">
        <img src={mark} alt="" className="size-6 shrink-0" />
        <span className="hidden text-body font-semibold wide:inline">Hengst Client</span>
      </div>
      <nav aria-label={t("nav.label")} className="flex flex-1 flex-col gap-2 p-2">
        {VIEW_ITEMS.filter((i) => !i.bottom).map(item)}
        <div className="mt-auto flex flex-col gap-2">
          {devBuild && (
            <div className="hidden px-1 wide:block">
              <Badge>{t("common.devBuild")}</Badge>
            </div>
          )}
          {VIEW_ITEMS.filter((i) => i.bottom).map(item)}
        </div>
      </nav>
    </aside>
  );
}
