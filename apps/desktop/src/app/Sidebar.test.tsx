import { cleanup, fireEvent, render, screen, within } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { TooltipProvider } from "@/components/ui/tooltip";
import { initI18n } from "@/i18n";
import { Sidebar } from "./Sidebar";

initI18n("de", "de-AT");

afterEach(() => {
  cleanup();
  vi.unstubAllGlobals();
});

function stubWidth(wide: boolean) {
  vi.stubGlobal("matchMedia", () => ({
    matches: wide,
    addEventListener: () => {},
    removeEventListener: () => {},
  }));
}

function renderSidebar(props: Partial<Parameters<typeof Sidebar>[0]> = {}) {
  const onSelect = vi.fn();
  render(
    <TooltipProvider>
      <Sidebar view="instances" onSelect={onSelect} devBuild={false} {...props} />
    </TooltipProvider>,
  );
  return onSelect;
}

describe.each([
  ["wide", true],
  ["icon rail", false],
])("Sidebar (%s)", (_, wide) => {
  it("the nav is a landmark with a label, one item is current and every item has a name", () => {
    stubWidth(wide);
    renderSidebar();
    const nav = screen.getByRole("navigation", { name: "Hauptnavigation" });
    const items = within(nav).getAllByRole("button");
    expect(items.map((b) => b.getAttribute("aria-label") ?? b.textContent)).toEqual([
      "Spielen",
      "Instanzen",
      "Einstellungen",
    ]);
    expect(items.filter((b) => b.getAttribute("aria-current") === "page")).toHaveLength(1);
    expect(screen.getByRole("button", { name: "Instanzen" }).getAttribute("aria-current")).toBe(
      "page",
    );
    expect(screen.getByRole("button", { name: "Einstellungen" }).hasAttribute("aria-current")).toBe(
      false,
    );
  });

  it("selects a view on click", () => {
    stubWidth(wide);
    const onSelect = renderSidebar();
    fireEvent.click(screen.getByRole("button", { name: "Einstellungen" }));
    expect(onSelect).toHaveBeenCalledWith("settings");
  });
});

describe("Sidebar development badge", () => {
  it("shows the badge in dev builds only", () => {
    stubWidth(true);
    renderSidebar({ devBuild: true });
    expect(screen.getByText("Entwicklungs-Build")).toBeTruthy();
    cleanup();
    renderSidebar({ devBuild: false });
    expect(screen.queryByText("Entwicklungs-Build")).toBeNull();
  });

  it("keeps the brand row out of the navigation and out of the buttons", () => {
    stubWidth(true);
    renderSidebar();
    expect(screen.getByText("Hengst Client").closest("button")).toBeNull();
  });
});
