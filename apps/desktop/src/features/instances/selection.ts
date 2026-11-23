import type { InstanceDto } from "@/ipc/bindings/InstanceDto";

/**
 * DESIGN.md 10.2 "Selection fallback": the selected instance if it can be started, else the first
 * one that can in list order (the list comes newest played first), else none.
 */
export function effectiveSelection(list: InstanceDto[]): string | null {
  const selected = list.find((i) => i.selected && i.status === "ready");
  return (selected ?? list.find((i) => i.status === "ready"))?.id ?? null;
}
