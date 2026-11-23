import { afterEach, describe, expect, it } from "vitest";
import { installInputMode } from "./inputMode";

let stop: (() => void) | undefined;
afterEach(() => {
  stop?.();
  document.documentElement.removeAttribute("data-input");
});

describe("input mode", () => {
  it("pointer-down sets data-input to pointer and keydown to keyboard", () => {
    stop = installInputMode();
    document.dispatchEvent(new KeyboardEvent("keydown", { key: "Tab", bubbles: true }));
    expect(document.documentElement.dataset.input).toBe("keyboard");
    document.dispatchEvent(new Event("pointerdown", { bubbles: true }));
    expect(document.documentElement.dataset.input).toBe("pointer");
  });

  it("stops listening when removed", () => {
    stop = installInputMode();
    stop();
    document.dispatchEvent(new KeyboardEvent("keydown", { key: "a", bubbles: true }));
    expect(document.documentElement.dataset.input).toBeUndefined();
  });
});
