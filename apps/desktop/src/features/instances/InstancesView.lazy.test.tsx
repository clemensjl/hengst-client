import { clearMocks } from "@tauri-apps/api/mocks";
import { act, cleanup, render, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import { TooltipProvider } from "@/components/ui/tooltip";
import { initI18n } from "@/i18n";
import { useApp } from "@/stores/app";
import { instance, instancesBackend } from "@/test/instancesBackend";
import { InstancesView } from "./InstancesView";

vi.mock("sonner", () => ({ toast: vi.fn(), Toaster: () => null }));
// The dialog's chunk cannot be loaded (a broken or missing file in the bundle).
vi.mock("./CreateInstanceDialog", () => {
  throw new Error("chunk failed to load");
});

const initial = useApp.getState();
beforeEach(() => {
  initI18n("en", "en-US");
  useApp.setState(initial, true);
});
afterEach(() => {
  cleanup();
  clearMocks();
});

it("a create dialog chunk that fails to load leaves the view rendered and says so", async () => {
  const unhandled: unknown[] = [];
  const listener = (e: PromiseRejectionEvent) => unhandled.push(e.reason);
  window.addEventListener("unhandledrejection", listener);
  // React logs the caught error; the test only cares that it is caught.
  const quiet = vi.spyOn(console, "error").mockImplementation(() => {});
  instancesBackend([instance("a", { name: "Alpha" })]);
  render(
    <TooltipProvider>
      <InstancesView />
    </TooltipProvider>,
  );
  await waitFor(() => expect(screen.getByRole("list")).toBeTruthy());
  act(() => useApp.getState().openCreate());
  expect(await screen.findByText("The create dialog could not be loaded")).toBeTruthy();
  expect(screen.getByRole("button", { name: "Try again" })).toBeTruthy();
  // The list is still there and the dialog is no longer wanted open.
  expect(screen.getByRole("list")).toBeTruthy();
  expect(useApp.getState().createOpen).toBe(false);
  await new Promise((r) => setTimeout(r, 30));
  window.removeEventListener("unhandledrejection", listener);
  quiet.mockRestore();
  expect(unhandled).toEqual([]);
});
