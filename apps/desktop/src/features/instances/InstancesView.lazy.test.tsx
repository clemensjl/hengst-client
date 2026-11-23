import { clearMocks } from "@tauri-apps/api/mocks";
import { act, cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import { TooltipProvider } from "@/components/ui/tooltip";
import { initI18n } from "@/i18n";
import { useApp } from "@/stores/app";
import { instance, instancesBackend } from "@/test/instancesBackend";
import { InstancesView } from "./InstancesView";

vi.mock("sonner", () => ({ toast: vi.fn(), Toaster: () => null }));
// The dialog's chunk cannot be loaded (a broken or missing file in the bundle). Every attempt to
// load it is counted. An unhandled rejection on the way would fail the run: vitest reports those
// on its own.
const attempts = vi.hoisted(() => ({ count: 0 }));
vi.mock("./CreateInstanceDialog", () => {
  attempts.count += 1;
  throw new Error("chunk failed to load");
});

const initial = useApp.getState();
let quiet: ReturnType<typeof vi.spyOn>;
beforeEach(() => {
  initI18n("en", "en-US");
  useApp.setState(initial, true);
  // React logs the caught error; the tests only care that it is caught.
  quiet = vi.spyOn(console, "error").mockImplementation(() => {});
});
afterEach(() => {
  cleanup();
  clearMocks();
  quiet.mockRestore();
});

async function showView() {
  instancesBackend([instance("a", { name: "Alpha" })]);
  render(
    <TooltipProvider>
      <InstancesView />
    </TooltipProvider>,
  );
  await waitFor(() => expect(screen.getByRole("list")).toBeTruthy());
}

const failureTitle = () => screen.findByText("The create dialog could not be loaded");

it("a create dialog chunk that fails to load leaves the view rendered and says so", async () => {
  await showView();
  act(() => useApp.getState().openCreate());
  expect(await failureTitle()).toBeTruthy();
  expect(screen.getByRole("button", { name: "Try again" })).toBeTruthy();
  // The list is still there and the dialog is no longer wanted open.
  expect(screen.getByRole("list")).toBeTruthy();
  expect(useApp.getState().createOpen).toBe(false);
});

it("the failure takes the focus on its title, as a failed create does (DESIGN.md 11.2)", async () => {
  await showView();
  act(() => useApp.getState().openCreate());
  const title = await failureTitle();
  await waitFor(() => expect(document.activeElement).toBe(title));
});

it("try again loads the chunk again; a second failure says so again and takes the focus", async () => {
  await showView();
  act(() => useApp.getState().openCreate());
  await failureTitle();
  const before = attempts.count;
  fireEvent.click(screen.getByRole("button", { name: "Try again" }));
  await waitFor(() => expect(attempts.count).toBeGreaterThan(before));
  const title = await failureTitle();
  await waitFor(() => expect(document.activeElement).toBe(title));
  expect(screen.getAllByText("The create dialog could not be loaded")).toHaveLength(1);
  expect(useApp.getState().createOpen).toBe(false);
  expect(screen.getByRole("list")).toBeTruthy();
});

it("after a failure, opening the dialog from anywhere else loads the chunk again", async () => {
  await showView();
  act(() => useApp.getState().openCreate());
  await failureTitle();
  const before = attempts.count;
  // The header's "New instance" (or the empty state's) only opens the dialog.
  act(() => useApp.getState().openCreate());
  await waitFor(() => expect(attempts.count).toBeGreaterThan(before));
  const title = await failureTitle();
  await waitFor(() => expect(document.activeElement).toBe(title));
});
