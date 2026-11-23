import { afterEach, describe, expect, it, vi } from "vitest";
import { applyTheme, resolveTheme, watchTheme } from "./theme";

afterEach(() => {
  document.documentElement.classList.remove("dark");
  vi.unstubAllGlobals();
});

function stubMedia(initial: boolean) {
  let listener: ((e: { matches: boolean }) => void) | undefined;
  const mql = {
    matches: initial,
    addEventListener: (_: string, l: typeof listener) => (listener = l),
    removeEventListener: () => (listener = undefined),
  };
  vi.stubGlobal("matchMedia", () => mql);
  return {
    change(matches: boolean) {
      mql.matches = matches;
      listener?.({ matches });
    },
    listening: () => listener !== undefined,
  };
}

describe("theme", () => {
  it("system follows prefersDark, explicit wins", () => {
    expect(resolveTheme("system", true)).toBe("dark");
    expect(resolveTheme("system", false)).toBe("light");
    expect(resolveTheme("light", true)).toBe("light");
    expect(resolveTheme("dark", false)).toBe("dark");
  });

  it("applyTheme sets and removes .dark on <html>", () => {
    applyTheme("dark");
    expect(document.documentElement.classList.contains("dark")).toBe(true);
    applyTheme("light");
    expect(document.documentElement.classList.contains("dark")).toBe(false);
  });

  it("watchTheme follows the system only for the system setting", () => {
    const media = stubMedia(true);
    const seen: string[] = [];
    const stop = watchTheme("system", (r) => seen.push(r));
    expect(seen).toEqual(["dark"]);
    expect(document.documentElement.classList.contains("dark")).toBe(true);
    media.change(false);
    expect(seen).toEqual(["dark", "light"]);
    expect(document.documentElement.classList.contains("dark")).toBe(false);
    stop();
    expect(media.listening()).toBe(false);

    const fixed = stubMedia(true);
    watchTheme("light", (r) => seen.push(r));
    expect(fixed.listening()).toBe(false);
    expect(document.documentElement.classList.contains("dark")).toBe(false);
  });
});
