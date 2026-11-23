import { describe, expect, it } from "vitest";
import type { InstanceDto } from "@/ipc/bindings/InstanceDto";
import { effectiveSelection } from "./selection";

const i = (id: string, extra: Partial<InstanceDto> = {}): InstanceDto => ({
  id,
  name: id,
  game: "1.21.11",
  loader: null,
  memoryMb: null,
  lastPlayed: null,
  status: "ready",
  reason: null,
  selected: false,
  ...extra,
});

describe("effectiveSelection", () => {
  it("effectiveSelection prefers the selected ready instance, else the first ready one, else null", () => {
    expect(effectiveSelection([i("a"), i("b", { selected: true })])).toBe("b");
    // The selected one cannot be read: the first readable one in list order (the last played).
    expect(
      effectiveSelection([
        i("new", { status: "tooNew" }),
        i("b", { status: "broken", selected: true }),
        i("c"),
        i("d"),
      ]),
    ).toBe("c");
    // Nothing selected (first start, or it was deleted).
    expect(effectiveSelection([i("a"), i("b")])).toBe("a");
    expect(effectiveSelection([i("x", { status: "tooNew" }), i("y", { status: "broken" })])).toBe(
      null,
    );
    expect(effectiveSelection([])).toBe(null);
  });
});
