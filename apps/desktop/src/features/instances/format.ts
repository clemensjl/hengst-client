import type { TFunction } from "i18next";
import type { InstanceDto } from "@/ipc/bindings/InstanceDto";

/** "Fabric 0.17.3", "Forge" (no version saved yet) or "No loader" (DESIGN.md 10.4). */
export function loaderText(instance: Pick<InstanceDto, "loader">, t: TFunction): string {
  const l = instance.loader;
  if (!l) return t("instances.noLoader");
  return l.version ? `${l.displayName} ${l.version}` : l.displayName;
}

/**
 * DESIGN.md 3.3: relative time up to 6 days ("2 days ago"), from 7 days the date in the medium
 * style; "Never played" for an instance that was never started. `seconds` are Unix seconds.
 */
export function lastPlayedText(
  seconds: number | null,
  lang: string,
  t: TFunction,
  now = Date.now(),
): string {
  if (seconds === null) return t("instances.neverPlayed");
  const elapsed = Math.max(0, now / 1000 - seconds);
  // Under a minute it is "now"; after that always with a number ("vor 2 Tagen", not "vorgestern").
  if (elapsed < 60)
    return new Intl.RelativeTimeFormat(lang, { numeric: "auto" }).format(0, "second");
  const relative = new Intl.RelativeTimeFormat(lang, { numeric: "always" });
  if (elapsed < 3600) return relative.format(-Math.floor(elapsed / 60), "minute");
  if (elapsed < 86400) return relative.format(-Math.floor(elapsed / 3600), "hour");
  const days = Math.floor(elapsed / 86400);
  if (days <= 6) return relative.format(-days, "day");
  return new Intl.DateTimeFormat(lang, { dateStyle: "medium" }).format(new Date(seconds * 1000));
}
