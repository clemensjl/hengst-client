import { cleanup, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it } from "vitest";
import { TooltipProvider } from "@/components/ui/tooltip";
import { initI18n } from "@/i18n";
import { Shell } from "./Shell";

initI18n("en", "en-US");
afterEach(cleanup);

describe("Shell", () => {
  it("has one h1 with the view title and the tab order sidebar, header, content", () => {
    const { container } = render(
      <TooltipProvider>
        <Shell view="settings" onSelect={() => {}} devBuild={false}>
          <button type="button">Content action</button>
        </Shell>
      </TooltipProvider>,
    );
    expect(screen.getAllByRole("heading", { level: 1 })).toHaveLength(1);
    expect(screen.getByRole("heading", { level: 1 }).textContent).toBe("Settings");
    // No positive tabindex anywhere, so the document order is the tab order.
    expect(
      container.querySelector("[tabindex]:not([tabindex='-1']):not([tabindex='0'])"),
    ).toBeNull();
    const focusable = [...container.querySelectorAll("button, a[href], input")].map(
      (el) => el.getAttribute("aria-label") ?? el.textContent,
    );
    expect(focusable).toEqual(["Instances", "Settings", "Content action"]);
    const regions = [...container.querySelectorAll("aside, header, main")].map((e) => e.tagName);
    expect(regions).toEqual(["ASIDE", "HEADER", "MAIN"]);
  });
});
