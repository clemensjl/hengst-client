import { cleanup, render, screen } from "@testing-library/react";
import { act } from "react";
import { afterEach, describe, expect, it } from "vitest";
import { TooltipProvider } from "@/components/ui/tooltip";
import { initI18n } from "@/i18n";
import { useApp } from "@/stores/app";
import { Shell } from "./Shell";

initI18n("en", "en-US");
afterEach(cleanup);

describe("Shell", () => {
  it("the account chip sits in the header: sidebar, then chip, then the content in the tab order", () => {
    act(() =>
      useApp.setState({
        accounts: {
          status: "ready",
          data: {
            canSignIn: true,
            accounts: [
              { id: "a", name: "Mira_K", kind: "microsoft", selected: true, signInRequired: false },
            ],
          },
        },
      }),
    );
    const { container } = render(
      <TooltipProvider>
        <Shell view="settings" onSelect={() => {}} devBuild={false}>
          <button type="button">Content action</button>
        </Shell>
      </TooltipProvider>,
    );
    const focusable = [...container.querySelectorAll("button, a[href], input")].map(
      (el) => el.getAttribute("aria-label") ?? el.textContent,
    );
    expect(focusable).toEqual(["Instances", "Settings", "Account: Mira_K", "Content action"]);
    const header = container.querySelector("header")!;
    expect(header.querySelector("[aria-label='Account: Mira_K']")).not.toBeNull();
    act(() => useApp.setState({ accounts: { status: "idle" } }));
  });

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
    // 8 px from the header to the first content (DESIGN.md 7.2): the main's own padding; the
    // sentinel for the header line takes no space and the scroller has no top padding.
    const main = container.querySelector("main")!;
    expect(main.className).toContain("pt-2");
    const scroller = main.parentElement!;
    expect(scroller.className).not.toMatch(/(^|\s)(pt|py|p)-/);
    expect(scroller.firstElementChild!.className).toContain("-mb-px");
    const regions = [...container.querySelectorAll("aside, header, main")].map((e) => e.tagName);
    expect(regions).toEqual(["ASIDE", "HEADER", "MAIN"]);
  });
});
