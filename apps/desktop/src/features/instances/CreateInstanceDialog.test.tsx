import { clearMocks } from "@tauri-apps/api/mocks";
import { act, cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { toast } from "sonner";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { TooltipProvider } from "@/components/ui/tooltip";
import { initI18n } from "@/i18n";
import type { InstanceDto } from "@/ipc/bindings/InstanceDto";
import { useApp } from "@/stores/app";
import { deferred, err, instance, instancesBackend } from "@/test/instancesBackend";
import { InstancesHeaderAction, InstancesView } from "./InstancesView";

vi.mock("sonner", () => ({ toast: vi.fn(), Toaster: () => null }));

const initial = useApp.getState();
beforeEach(() => {
  initI18n("en", "en-US");
  useApp.setState(initial, true);
  vi.mocked(toast).mockClear();
  Element.prototype.scrollIntoView = vi.fn();
});
afterEach(async () => {
  cleanup();
  clearMocks();
  await new Promise((resolve) => setTimeout(resolve, 30));
});

/** The view with the dialog, the list loaded; the dialog opened unless `open` is false. */
async function setup(
  list: InstanceDto[] = [instance("x", { name: "Existing" })],
  tune: (b: ReturnType<typeof instancesBackend>) => void = () => {},
  open = true,
) {
  const backend = instancesBackend(list);
  tune(backend);
  render(
    <TooltipProvider delay={0}>
      <InstancesHeaderAction />
      <InstancesView />
    </TooltipProvider>,
  );
  await waitFor(() => expect(useApp.getState().instances.status).toBe("ready"));
  if (open) await openDialog();
  return backend;
}

async function openDialog() {
  act(() => useApp.getState().openCreate());
  await screen.findByRole("dialog");
}

const dialog = () => screen.getByRole("dialog");
const versionTrigger = () => within(dialog()).getByRole("combobox", { name: /Game version/ });
const createButton = () =>
  within(dialog())
    .getAllByRole("button")
    .find((b) => b.getAttribute("data-variant") === "primary")!;
const nameField = () => within(dialog()).getByRole("textbox", { name: "Name" });
const loaderVersionTrigger = () =>
  within(dialog()).getByRole("combobox", { name: /Loader version/ });
/** Waits until the version list is there (the trigger shows the latest release). */
const versionsLoaded = () => waitFor(() => expect(versionTrigger().textContent).toBe("1.21.11"));

async function chooseVersion(id: string, search?: string) {
  fireEvent.click(versionTrigger());
  const input = await screen.findByPlaceholderText("Search");
  if (search !== undefined) fireEvent.change(input, { target: { value: search } });
  fireEvent.click(await screen.findByRole("option", { name: new RegExp(`^${escape(id)}`) }));
  await waitFor(() => expect(versionTrigger().textContent).toBe(id));
}
const escape = (s: string) => s.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");

async function chooseLoader(name: string) {
  fireEvent.click(within(dialog()).getByRole("radio", { name }));
  await waitFor(() =>
    expect(within(dialog()).getByRole("radio", { name }).getAttribute("aria-checked")).toBe("true"),
  );
}

describe("CreateInstanceDialog", () => {
  it("double_submit_creates_one_instance", async () => {
    const gate = deferred();
    const backend = await setup(undefined, (b) => (b.createGate = gate.promise));
    await versionsLoaded();
    fireEvent.click(createButton());
    fireEvent.click(createButton());
    await waitFor(() => expect(createButton().textContent).toBe("Creating…"));
    expect(createButton().getAttribute("aria-disabled")).toBe("true");
    fireEvent.click(createButton());
    gate.resolve();
    await waitFor(() => expect(screen.queryByRole("dialog")).toBeNull());
    expect(backend.count("instance_create")).toBe(1);
  });

  it("preselects the recommended loader version", async () => {
    await setup(undefined, (b) => {
      b.loaderVersions = () => ({
        versions: [
          { id: "0.18.0-beta.1", stable: false },
          { id: "0.17.3", stable: true },
          { id: "0.17.2", stable: true },
        ],
        recommended: "0.17.3",
      });
    });
    await versionsLoaded();
    await chooseLoader("Fabric");
    await waitFor(() => expect(loaderVersionTrigger().textContent).toContain("0.17.3"));
    fireEvent.click(loaderVersionTrigger());
    const options = await screen.findAllByRole("option");
    expect(options.map((o) => o.textContent)).toEqual([
      "0.18.0-beta.1Unstable",
      "0.17.3Recommended",
      "0.17.2",
    ]);
    expect(options[1].getAttribute("aria-selected")).toBe("true");
  });

  it("sends only the slug and the version to instance_create", async () => {
    const backend = await setup();
    await versionsLoaded();
    await chooseVersion("1.8.9");
    await chooseLoader("Legacy Fabric");
    await waitFor(() => expect(loaderVersionTrigger().textContent).toContain("0.17.3"));
    expect(nameField()).toHaveProperty("value", "1.8.9 Legacy Fabric");
    fireEvent.click(createButton());
    await waitFor(() => expect(backend.count("instance_create")).toBe(1));
    expect(backend.of("instance_create")[0]).toEqual({
      req: {
        name: "1.8.9 Legacy Fabric",
        game: "1.8.9",
        loader: { kind: "legacy-fabric", version: "0.17.3" },
      },
    });
    expect(backend.of("loader_versions")).toContainEqual({ kind: "legacy-fabric", game: "1.8.9" });
  });

  it("explains why create is disabled when the loader has no builds", async () => {
    const backend = await setup(undefined, (b) => {
      b.loaderVersions = (_kind, game) => {
        throw err("noLoaderVersions", { loader: "NeoForge", game, hint: "neoForgeStartsAt1201" });
      };
    });
    await versionsLoaded();
    await chooseLoader("NeoForge");
    const hint = await within(dialog()).findByText("No NeoForge version for 1.21.11");
    expect(dialog().textContent).toContain("NeoForge starts at 1.20.1.");
    const button = createButton();
    expect(button.getAttribute("aria-disabled")).toBe("true");
    const reason = document.getElementById(button.getAttribute("aria-describedby")!)!;
    expect(reason.contains(hint)).toBe(true);
    expect(within(dialog()).queryByRole("combobox", { name: /Loader version/ })).toBeNull();
    fireEvent.click(button);
    await act(async () => {});
    expect(backend.count("instance_create")).toBe(0);
  });

  it("keeps a typed name when the version changes", async () => {
    await setup();
    await versionsLoaded();
    expect(nameField()).toHaveProperty("value", "1.21.11");
    fireEvent.change(nameField(), { target: { value: "Mira_K world" } });
    await chooseVersion("1.20.1");
    await chooseLoader("Forge");
    expect(nameField()).toHaveProperty("value", "Mira_K world");
  });

  it("the name follows the choice until the user edits it", async () => {
    await setup();
    await versionsLoaded();
    await chooseVersion("1.20.1");
    await chooseLoader("Forge");
    expect(nameField()).toHaveProperty("value", "1.20.1 Forge");
  });

  it("keeps inputs after a failed create and shows the error at the form", async () => {
    await setup(undefined, (b) => (b.createFails = err("fileSystem")));
    await versionsLoaded();
    await chooseVersion("1.20.1");
    fireEvent.change(nameField(), { target: { value: "Kept" } });
    fireEvent.click(createButton());
    const panel = await within(dialog()).findByRole("alert");
    const title = within(panel).getByText("A file could not be read or written");
    await waitFor(() => expect(document.activeElement).toBe(title));
    expect(nameField()).toHaveProperty("value", "Kept");
    expect(versionTrigger().textContent).toBe("1.20.1");
    // The panel sits directly above the footer.
    const footer = dialog().querySelector("[data-slot=dialog-footer]")!;
    expect(footer.previousElementSibling?.lastElementChild).toBe(panel);
  });

  it("escape closes and keeps the draft, reopening restores it, a successful create clears it", async () => {
    await setup();
    await versionsLoaded();
    await chooseVersion("1.20.1");
    fireEvent.change(nameField(), { target: { value: "Draft" } });
    fireEvent.keyDown(document.activeElement ?? document.body, { key: "Escape" });
    await waitFor(() => expect(screen.queryByRole("dialog")).toBeNull());
    await openDialog();
    expect(nameField()).toHaveProperty("value", "Draft");
    expect(versionTrigger().textContent).toBe("1.20.1");
    fireEvent.click(createButton());
    await waitFor(() => expect(screen.queryByRole("dialog")).toBeNull());
    await openDialog();
    await versionsLoaded();
    expect(nameField()).toHaveProperty("value", "1.21.11");
  });

  it("an outside pointer-down does not close the dialog and focus starts on the version trigger", async () => {
    await setup();
    await waitFor(() => expect(document.activeElement).toBe(versionTrigger()));
    const overlay = document.querySelector("[data-slot=dialog-overlay]")!;
    fireEvent.pointerDown(overlay);
    fireEvent.mouseDown(overlay);
    fireEvent.click(overlay);
    await act(async () => {
      await new Promise((r) => setTimeout(r, 50));
    });
    expect(screen.getByRole("dialog")).toBeTruthy();
  });

  it("validates the name: empty gives 'Enter a name.', 65 characters give 'At most 64 characters.', pasted line breaks are removed", async () => {
    const backend = await setup();
    await versionsLoaded();
    fireEvent.change(nameField(), { target: { value: "   " } });
    expect(within(dialog()).queryByText("Enter a name.")).toBeNull();
    fireEvent.blur(nameField());
    expect(within(dialog()).getByText("Enter a name.")).toBeTruthy();
    expect(nameField().getAttribute("aria-invalid")).toBe("true");
    // After the first error it checks on every keystroke.
    fireEvent.change(nameField(), { target: { value: "x".repeat(65) } });
    expect(within(dialog()).getByText("At most 64 characters.")).toBeTruthy();
    expect(within(dialog()).getByText("65/64")).toBeTruthy();
    fireEvent.click(createButton());
    await act(async () => {});
    expect(backend.count("instance_create")).toBe(0);
    // The long name of the brief has 65 characters: one too many.
    fireEvent.change(nameField(), {
      target: { value: "Aleksandra Wiśniewska-Kowalczyks Überlebenswelt mit Freunden 26.3" },
    });
    expect(within(dialog()).getByText("At most 64 characters.")).toBeTruthy();
    // The longest allowed name, with the counter from 80 % of the limit.
    const longest = "Aleksandra Wiśniewska-Kowalczyk Überlebenswelt mit Freunden 26.3";
    expect(Array.from(longest)).toHaveLength(64);
    fireEvent.change(nameField(), { target: { value: longest } });
    expect(within(dialog()).queryByText("At most 64 characters.")).toBeNull();
    expect(within(dialog()).getByText("64/64")).toBeTruthy();
    fireEvent.change(nameField(), { target: { value: "Line one\nline two\r\n" } });
    expect(nameField()).toHaveProperty("value", "Line oneline two");
  });

  it("offline with a cached list shows the offline caption, with another refresh failure the not-current caption", async () => {
    await setup();
    await versionsLoaded();
    act(() => useApp.setState({ versionsStale: "offline" }));
    expect(within(dialog()).getByText("Offline: saved list")).toBeTruthy();
    act(() => useApp.setState({ versionsStale: "other" }));
    expect(within(dialog()).getByText("Saved list, not current")).toBeTruthy();
    expect(createButton().getAttribute("aria-disabled")).toBeNull();
  });

  it("offline without a cached list shows the connection error with Try again and a disabled create button described by it", async () => {
    const backend = await setup(undefined, (b) => (b.versionsFails = err("offline")));
    const text = await within(dialog()).findByText(
      "No connection. The version list needs the internet the first time.",
    );
    const button = createButton();
    expect(button.getAttribute("aria-disabled")).toBe("true");
    expect(button.getAttribute("aria-describedby")).toBe(text.id);
    backend.versionsFails = null;
    fireEvent.click(within(dialog()).getByRole("button", { name: "Try again" }));
    await versionsLoaded();
    expect(createButton().getAttribute("aria-disabled")).toBeNull();
  });

  it("another version list failure shows its own inline error", async () => {
    await setup(undefined, (b) => (b.versionsFails = err("serverError")));
    expect(await within(dialog()).findByText("The version list could not be loaded.")).toBeTruthy();
  });

  it("loader versions offline show their own text with Try again", async () => {
    let fail = true;
    const backend = await setup(undefined, (b) => {
      const ok = b.loaderVersions;
      b.loaderVersions = (k, g) => {
        if (fail) throw err("network");
        return ok(k, g);
      };
    });
    await versionsLoaded();
    await chooseLoader("Quilt");
    const text = await within(dialog()).findByText(
      "No connection. Loader versions cannot be loaded right now.",
    );
    expect(createButton().getAttribute("aria-describedby")).toBe(text.id);
    fail = false;
    fireEvent.click(within(dialog()).getByRole("button", { name: "Try again" }));
    await waitFor(() => expect(loaderVersionTrigger().textContent).toContain("0.17.3"));
    expect(backend.count("loader_versions")).toBe(2);
  });

  it("the created row is selected, scrolled into view and focused", async () => {
    await setup();
    await versionsLoaded();
    await chooseVersion("1.20.1");
    await chooseLoader("Forge");
    fireEvent.click(createButton());
    await waitFor(() => expect(screen.queryByRole("dialog")).toBeNull());
    expect(toast).toHaveBeenCalledWith("1.20.1 Forge created");
    const row = await waitFor(() => {
      const el = document.querySelector<HTMLElement>('[data-instance-row="1.20.1-forge"]');
      expect(el).not.toBeNull();
      return el!;
    });
    await waitFor(() => expect(document.activeElement).toBe(row));
    expect(row.getAttribute("aria-current")).toBe("true");
    expect(row.scrollIntoView).toHaveBeenCalled();
  });

  it("from the empty state the first instance takes the focus of the unmounted button", async () => {
    await setup([]);
    await versionsLoaded();
    fireEvent.click(createButton());
    await waitFor(() =>
      expect(document.activeElement).toBe(document.querySelector('[data-instance-row="1.21.11"]')),
    );
  });

  it("search matches case-insensitively, and an empty result offers to reset the filters where that helps", async () => {
    await setup();
    await versionsLoaded();
    fireEvent.click(versionTrigger());
    const input = await screen.findByPlaceholderText("Search");
    fireEvent.change(input, { target: { value: "PRE" } });
    // Releases only: no match, and the default filter would not show one either.
    expect(await screen.findByText("No version matches “PRE”")).toBeTruthy();
    expect(screen.queryByRole("button", { name: "Reset filters" })).toBeNull();
    fireEvent.keyDown(input, { key: "Escape" });
    await waitFor(() => expect(screen.queryByPlaceholderText("Search")).toBeNull());
    fireEvent.click(within(dialog()).getByRole("checkbox", { name: "Snapshots" }));
    await chooseVersion("1.14 Pre-Release 1", "pre");
    // All filters off hides everything; resetting brings the releases back.
    for (const name of ["Releases", "Snapshots"]) {
      fireEvent.click(within(dialog()).getByRole("checkbox", { name }));
    }
    fireEvent.click(versionTrigger());
    expect(await screen.findByText("The filters hide every version")).toBeTruthy();
    fireEvent.click(screen.getByRole("button", { name: "Reset filters" }));
    expect(await screen.findByRole("option", { name: "1.21.11" })).toBeTruthy();
  });

  it("without any loader from the backend only 'No loader' is offered", async () => {
    await setup(undefined, (b) => (b.loaders = []));
    await versionsLoaded();
    expect(
      within(dialog())
        .getAllByRole("radio")
        .map((r) => r.textContent),
    ).toEqual(["No loader"]);
  });
});
