import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import { initI18n } from "@/i18n";
import { LogRoute } from "./LogRoute";

// The log view's chunk cannot be loaded (a broken or missing file in the bundle).
const attempts = vi.hoisted(() => ({ count: 0 }));
vi.mock("./LogView", () => {
  attempts.count += 1;
  throw new Error("chunk failed to load");
});

let quiet: ReturnType<typeof vi.spyOn>;
beforeEach(() => {
  initI18n("en", "en-US");
  // React logs the caught error; the test only cares that it is caught.
  quiet = vi.spyOn(console, "error").mockImplementation(() => {});
});
afterEach(() => {
  cleanup();
  quiet.mockRestore();
});

it("a log view chunk that fails to load says so, and Try again loads it again", async () => {
  render(<LogRoute />);
  expect(await screen.findByText("The log could not be loaded")).toBeTruthy();
  const before = attempts.count;
  fireEvent.click(screen.getByRole("button", { name: "Try again" }));
  await waitFor(() => expect(attempts.count).toBeGreaterThan(before));
  expect(await screen.findByText("The log could not be loaded")).toBeTruthy();
});
