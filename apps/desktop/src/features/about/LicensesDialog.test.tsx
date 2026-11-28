import { cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { afterEach, describe, expect, it } from "vitest";
import { TooltipProvider } from "@/components/ui/tooltip";
import { initI18n } from "@/i18n";
import type { Licenses } from "./licenseData";
import { LicensesDialog } from "./LicensesDialog";

initI18n("en", "en-US");
afterEach(cleanup);

const data: Licenses = {
  format: 1,
  inputs: {},
  rust: [
    { name: "serde", version: "1.0.228", license: "MIT OR Apache-2.0", texts: [0, 1] },
    { name: "tauri", version: "2.12.7", license: "Apache-2.0 OR MIT", texts: [1] },
  ],
  javascript: [{ name: "react", version: "19.3.1", license: "MIT", texts: [2] }],
  texts: ["MIT text <b>not html</b>", "Apache text", "React MIT text"],
};

function open(load: () => Promise<Licenses>) {
  render(
    <TooltipProvider>
      <LicensesDialog load={load} />
    </TooltipProvider>,
  );
  const trigger = screen.getByRole("button", { name: "Third-party licenses" });
  fireEvent.click(trigger);
  return trigger;
}

describe("LicensesDialog", () => {
  it("lists both groups with name, version and license, and shows the text as plain text", async () => {
    open(async () => data);
    const dialog = await screen.findByRole("dialog", { name: "Third-party licenses" });
    expect(
      within(dialog).getByText(
        "Hengst Client uses these libraries. The full texts are under each entry.",
      ),
    ).toBeTruthy();
    expect(within(dialog).getByRole("heading", { name: "Rust" })).toBeTruthy();
    expect(within(dialog).getByRole("heading", { name: "JavaScript" })).toBeTruthy();
    const serde = await within(dialog).findByRole("button", { name: /serde/ });
    expect(serde.textContent).toContain("1.0.228");
    expect(serde.textContent).toContain("MIT OR Apache-2.0");
    expect(serde.getAttribute("aria-expanded")).toBe("false");
    fireEvent.click(serde);
    expect(serde.getAttribute("aria-expanded")).toBe("true");
    expect(within(dialog).getByText("MIT text <b>not html</b>")).toBeTruthy();
    expect(dialog.querySelector("b")).toBeNull();
    expect(within(dialog).getByText("Apache text")).toBeTruthy();
  });

  it("has the header close button and one Close in the footer, and returns focus to its trigger", async () => {
    const trigger = open(async () => data);
    const dialog = await screen.findByRole("dialog", { name: "Third-party licenses" });
    // DESIGN.md 10.9: the X top right; 11.10: the footer only dismisses.
    expect(within(dialog).getAllByRole("button", { name: "Close" })).toHaveLength(2);
    const footer = dialog.querySelector("[data-slot=dialog-footer]") as HTMLElement;
    const closes = within(footer).getAllByRole("button");
    expect(closes).toHaveLength(1);
    expect(closes[0].textContent).toBe("Close");
    fireEvent.click(closes[0]);
    await waitFor(() => expect(screen.queryByRole("dialog")).toBeNull());
    await waitFor(() => expect(document.activeElement).toBe(trigger));
  });

  it("says it is a build error when the list is empty", async () => {
    open(async () => ({ ...data, rust: [], javascript: [] }));
    expect(await screen.findByText("No license data found. This is a build error.")).toBeTruthy();
  });

  it("a failed load shows the error panel, and Try again loads again", async () => {
    let fail = true;
    let calls = 0;
    open(async () => {
      calls += 1;
      if (fail) throw new Error("chunk failed");
      return data;
    });
    expect(await screen.findByText("License data could not be loaded")).toBeTruthy();
    fail = false;
    fireEvent.click(screen.getByRole("button", { name: "Try again" }));
    expect(await screen.findByRole("button", { name: /tauri/ })).toBeTruthy();
    expect(calls).toBe(2);
  });
});
