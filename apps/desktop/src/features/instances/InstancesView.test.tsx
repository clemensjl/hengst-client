import { clearMocks } from "@tauri-apps/api/mocks";
import { act, cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { toast } from "sonner";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { TooltipProvider } from "@/components/ui/tooltip";
import { initI18n } from "@/i18n";
import type { InstanceDto } from "@/ipc/bindings/InstanceDto";
import { useApp } from "@/stores/app";
import { err, instance, instancesBackend } from "@/test/instancesBackend";
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
  vi.useRealTimers();
  cleanup();
  clearMocks();
  await new Promise((resolve) => setTimeout(resolve, 30));
});

async function setup(list: InstanceDto[] = [], options: { incomplete?: boolean } = {}) {
  const backend = instancesBackend(list);
  backend.incomplete = options.incomplete ?? false;
  render(
    <TooltipProvider delay={0}>
      <div data-testid="header">
        <InstancesHeaderAction />
      </div>
      <InstancesView />
    </TooltipProvider>,
  );
  await waitFor(() => expect(useApp.getState().instances.status).toBe("ready"));
  return backend;
}

const header = () => screen.getByTestId("header");
const rowButton = (name: string) =>
  screen
    .getAllByRole("button")
    .find((b) => b.dataset.instanceRow !== undefined && b.textContent?.includes(name))!;
const actionButton = (name: string) => screen.getByRole("button", { name: `Actions for ${name}` });
const dialog = () => screen.getByRole("alertdialog");
const openMenu = async (name: string) => {
  fireEvent.click(actionButton(name));
  return screen.findByRole("menu");
};
const isPrimary = (el: Element) => el.getAttribute("data-variant") === "primary";

const three = () => [
  instance("a", { name: "Alpha" }),
  instance("b", { name: "Beta", selected: true }),
  instance("c", { name: "Gamma" }),
];

describe("InstancesView states", () => {
  it("shows the empty state with one primary action", async () => {
    await setup();
    expect(screen.getByRole("heading", { level: 2, name: "No instances yet" })).toBeTruthy();
    expect(
      screen.getByText("An instance is its own game folder with a version and a loader."),
    ).toBeTruthy();
    const primaries = screen.getAllByRole("button").filter(isPrimary);
    expect(primaries).toHaveLength(1);
    expect(primaries[0].textContent).toBe("Create instance");
    fireEvent.click(primaries[0]);
    expect(useApp.getState().createOpen).toBe(true);
  });

  it("the empty state hides the header action and the column header", async () => {
    await setup();
    expect(within(header()).queryByRole("button")).toBeNull();
    expect(screen.queryByText("Last played")).toBeNull();
    expect(screen.queryByRole("list")).toBeNull();
  });

  it("with instances the header holds the primary action and the list has its column header", async () => {
    await setup(three());
    expect(within(header()).getByRole("button", { name: "Create instance" })).toBeTruthy();
    for (const c of ["Name", "Version", "Loader", "Last played"]) {
      expect(screen.getByText(c)).toBeTruthy();
    }
    expect(within(screen.getByRole("list")).getAllByRole("listitem")).toHaveLength(3);
  });

  it("a row shows version, loader and last played without a dot chain", async () => {
    await setup([
      instance("x", {
        name: "Survival",
        loader: { kind: "fabric", displayName: "Fabric", version: "0.17.3" },
      }),
    ]);
    const row = rowButton("Survival");
    expect(within(row).getByText("1.21.11")).toBeTruthy();
    expect(within(row).getByText("Fabric 0.17.3")).toBeTruthy();
    expect(within(row).getByText("Never played")).toBeTruthy();
    expect(row.textContent).not.toMatch(/·/);
  });

  it("an error replaces the list and Try again loads it again", async () => {
    const backend = instancesBackend();
    backend.listFails = err("fileSystem");
    render(
      <TooltipProvider>
        <InstancesView />
      </TooltipProvider>,
    );
    expect(await screen.findByRole("alert")).toBeTruthy();
    expect(screen.getByText("A file could not be read or written")).toBeTruthy();
    backend.listFails = null;
    backend.list = [instance("a")];
    fireEvent.click(screen.getByRole("button", { name: "Try again" }));
    await waitFor(() => expect(screen.getByRole("list")).toBeTruthy());
  });

  it("an incomplete listing says that some folders could not be read", async () => {
    await setup([instance("a")], { incomplete: true });
    expect(
      screen.getByText("Some folders could not be read. The list may be incomplete."),
    ).toBeTruthy();
  });

  it("loading shows the column header and three skeleton rows only after 200 ms", async () => {
    vi.useFakeTimers();
    instancesBackend([instance("a")]);
    // The answer never comes in this test: the list stays in loading.
    useApp.setState({ instances: { status: "loading" } });
    render(
      <TooltipProvider>
        <InstancesView />
      </TooltipProvider>,
    );
    expect(document.querySelectorAll("[data-slot=skeleton]")).toHaveLength(0);
    act(() => vi.advanceTimersByTime(250));
    expect(screen.getByText("Last played")).toBeTruthy();
    expect(document.querySelectorAll("[data-slot=skeleton-row]")).toHaveLength(3);
  });
});

describe("InstancesView rows", () => {
  it("the selected row has aria-current true and a check", async () => {
    await setup(three());
    const selected = rowButton("Beta");
    expect(selected.getAttribute("aria-current")).toBe("true");
    expect(selected.querySelector("[data-slot=check] svg")).not.toBeNull();
    for (const other of [rowButton("Alpha"), rowButton("Gamma")]) {
      expect(other.getAttribute("aria-current")).toBeNull();
      expect(other.querySelector("[data-slot=check] svg")).toBeNull();
    }
  });

  it("without a selection the first readable instance shows as selected", async () => {
    await setup([instance("old", { status: "tooNew" }), instance("a", { name: "Alpha" })]);
    expect(rowButton("Alpha").getAttribute("aria-current")).toBe("true");
  });

  it("clicking a row selects it in the backend and the list follows", async () => {
    const backend = await setup(three());
    fireEvent.click(rowButton("Gamma"));
    await waitFor(() => expect(rowButton("Gamma").getAttribute("aria-current")).toBe("true"));
    expect(backend.of("instance_select")).toEqual([{ id: "c" }]);
  });

  it("too new instance offers only open folder, is aria-disabled and shows the Newer format badge", async () => {
    const backend = await setup([instance("future", { status: "tooNew", game: "" })]);
    const row = rowButton("future");
    expect(row.getAttribute("aria-disabled")).toBe("true");
    // The badge is in the row's name once, not also its description.
    expect(within(row).getByText("Newer format")).toBeTruthy();
    expect(row.getAttribute("aria-describedby")).toBeNull();
    fireEvent.click(row);
    await act(async () => {});
    expect(backend.count("instance_select")).toBe(0);
    const menu = await openMenu("future");
    // Only "Open folder" can be used; "Delete" is there, disabled, with the reason as its
    // description, and every child of the menu is a menu item or a separator.
    const items = within(menu).getAllByRole("menuitem");
    expect(
      items.filter((i) => i.getAttribute("aria-disabled") !== "true").map((i) => i.textContent),
    ).toEqual(["Open folder"]);
    const del = within(menu).getByRole("menuitem", { name: "Delete" });
    expect(del.getAttribute("aria-disabled")).toBe("true");
    const reason = document.getElementById(del.getAttribute("aria-describedby")!)!;
    expect(reason.textContent).toBe(
      "Saved by a newer version of Hengst Client. You can delete it only in Explorer.",
    );
    expect(del.contains(reason)).toBe(true);
    for (const child of menu.children) {
      expect(["menuitem", "separator"]).toContain(child.getAttribute("role"));
    }
    fireEvent.click(del);
    expect(screen.queryByRole("alertdialog")).toBeNull();
    // The Delete key does not open the dialog for it either.
    fireEvent.keyDown(document.body, { key: "Escape" });
    fireEvent.keyDown(row, { key: "Delete" });
    expect(screen.queryByRole("alertdialog")).toBeNull();
  });

  it("an unreadable instance can be opened and moved to the Recycle Bin, but not selected", async () => {
    await setup([instance("bad", { status: "broken", game: "", reason: "damaged" })]);
    const row = rowButton("bad");
    expect(row.getAttribute("aria-disabled")).toBe("true");
    expect(within(row).getByText("Unreadable")).toBeTruthy();
    const menu = await openMenu("bad");
    expect(
      within(menu)
        .getAllByRole("menuitem")
        .map((i) => i.textContent),
    ).toEqual(["Open folder", "Delete"]);
  });

  it("open folder asks the backend for the instance's folder; a failure shows the folder copy", async () => {
    const backend = await setup(three());
    let menu = await openMenu("Alpha");
    fireEvent.click(within(menu).getByRole("menuitem", { name: "Open folder" }));
    await waitFor(() =>
      expect(backend.of("open_location")).toEqual([{ target: { kind: "instance", id: "a" } }]),
    );
    backend.openFails = err("openFailed");
    menu = await openMenu("Alpha");
    fireEvent.click(within(menu).getByRole("menuitem", { name: "Open folder" }));
    expect(await screen.findByText("The folder could not be opened")).toBeTruthy();
  });
});

describe("InstancesView delete", () => {
  it("delete asks first and calls instance_delete after confirm only", async () => {
    const backend = await setup(three());
    const menu = await openMenu("Alpha");
    fireEvent.click(within(menu).getByRole("menuitem", { name: "Delete" }));
    expect(within(dialog()).getByText("Delete Alpha?")).toBeTruthy();
    expect(
      within(dialog()).getByText(
        "The folder with worlds, settings and screenshots is moved to the Recycle Bin.",
      ),
    ).toBeTruthy();
    expect(backend.count("instance_delete")).toBe(0);
    fireEvent.click(within(dialog()).getByRole("button", { name: "Move to Recycle Bin" }));
    await waitFor(() => expect(screen.queryByRole("alertdialog")).toBeNull());
    expect(backend.of("instance_delete")).toEqual([{ id: "a" }]);
    expect(toast).toHaveBeenCalledWith("Alpha is in the Recycle Bin");
    await waitFor(() => expect(screen.queryByText("Alpha")).toBeNull());
  });

  it("keep has initial focus in the delete dialog and deletes nothing", async () => {
    const backend = await setup(three());
    const menu = await openMenu("Gamma");
    fireEvent.click(within(menu).getByRole("menuitem", { name: "Delete" }));
    const keep = within(dialog()).getByRole("button", { name: "Keep" });
    await waitFor(() => expect(document.activeElement).toBe(keep));
    fireEvent.click(keep);
    await waitFor(() => expect(screen.queryByRole("alertdialog")).toBeNull());
    expect(backend.count("instance_delete")).toBe(0);
  });

  it("the delete key on a focused row opens the delete dialog", async () => {
    await setup(three());
    const row = rowButton("Beta");
    act(() => row.focus());
    fireEvent.keyDown(row, { key: "Delete" });
    expect(within(await screen.findByRole("alertdialog")).getByText("Delete Beta?")).toBeTruthy();
  });

  it("delete is disabled with its reason for the active instance", async () => {
    await setup(three());
    act(() => useApp.setState({ activeInstance: "a" }));
    const menu = await openMenu("Alpha");
    const del = within(menu).getByRole("menuitem", { name: "Delete" });
    expect(del.getAttribute("aria-disabled")).toBe("true");
    const reason = within(menu).getByText("Currently running. Stop the game first.");
    expect(del.getAttribute("aria-describedby")).toBe(reason.id);
    expect(del.contains(reason)).toBe(true);
    fireEvent.click(del);
    expect(screen.queryByRole("alertdialog")).toBeNull();
    fireEvent.keyDown(document.body, { key: "Escape" });
    fireEvent.keyDown(rowButton("Alpha"), { key: "Delete" });
    expect(screen.queryByRole("alertdialog")).toBeNull();
  });

  it("a failed move to the Recycle Bin shows the panel with Open folder and keeps the dialog open", async () => {
    const backend = await setup(three());
    backend.deleteFails = err("recycleBinFailed", { id: "a", reason: "tooLarge" });
    const menu = await openMenu("Alpha");
    fireEvent.click(within(menu).getByRole("menuitem", { name: "Delete" }));
    fireEvent.click(within(dialog()).getByRole("button", { name: "Move to Recycle Bin" }));
    const panel = await within(dialog()).findByRole("alert");
    expect(
      within(panel).getByText("The folder could not be moved to the Recycle Bin"),
    ).toBeTruthy();
    expect(panel.textContent).toContain(
      "The folder is probably too large for the Recycle Bin of this drive. Nothing was deleted.",
    );
    expect(dialog().textContent).not.toMatch(/is in the Recycle Bin/);
    fireEvent.click(within(panel).getByRole("button", { name: "Open folder" }));
    await waitFor(() =>
      expect(backend.of("open_location")).toEqual([{ target: { kind: "instance", id: "a" } }]),
    );
    expect(screen.getByRole("alertdialog")).toBeTruthy();
    expect(toast).not.toHaveBeenCalled();
  });

  it("a refused Open folder in the delete dialog shows the folder title and body", async () => {
    const backend = await setup(three());
    backend.deleteFails = err("recycleBinFailed", { id: "a", reason: "tooLarge" });
    backend.openFails = err("openFailed");
    const menu = await openMenu("Alpha");
    fireEvent.click(within(menu).getByRole("menuitem", { name: "Delete" }));
    fireEvent.click(within(dialog()).getByRole("button", { name: "Move to Recycle Bin" }));
    const panel = await within(dialog()).findByRole("alert");
    fireEvent.click(within(panel).getByRole("button", { name: "Open folder" }));
    expect(await within(dialog()).findByText("The folder could not be opened")).toBeTruthy();
    expect(
      within(dialog()).getByText(
        "Windows refused to open it in Explorer. Check that the folder still exists, then try again.",
      ),
    ).toBeTruthy();
  });

  it.each(["binOff", "noBin", "inUse", "unchecked", "other"])(
    "the reason %s is named in the panel",
    async (reason) => {
      const backend = await setup(three());
      backend.deleteFails = err("recycleBinFailed", { id: "a", reason });
      const menu = await openMenu("Alpha");
      fireEvent.click(within(menu).getByRole("menuitem", { name: "Delete" }));
      fireEvent.click(within(dialog()).getByRole("button", { name: "Move to Recycle Bin" }));
      const panel = await within(dialog()).findByRole("alert");
      const { default: en } = await import("@/i18n/en.json");
      expect(panel.textContent).toContain(
        (en as Record<string, string>)[`errors.recycleBinFailed.reason.${reason}`],
      );
    },
  );

  it("a folder gone but not confirmed in the bin is never called in the Recycle Bin; the list refreshes", async () => {
    const backend = await setup(three());
    backend.deleteFails = err("recycleBinUnconfirmed", { id: "b" });
    const menu = await openMenu("Beta");
    fireEvent.click(within(menu).getByRole("menuitem", { name: "Delete" }));
    // The backend removed the folder although it reports an error.
    backend.list = backend.list.filter((i) => i.id !== "b");
    const lists = backend.count("instances_list");
    fireEvent.click(within(dialog()).getByRole("button", { name: "Move to Recycle Bin" }));
    const panel = await within(dialog()).findByRole("alert");
    expect(
      within(panel).getByText("The folder is gone and may not be in the Recycle Bin"),
    ).toBeTruthy();
    expect(toast).not.toHaveBeenCalled();
    await waitFor(() => expect(backend.count("instances_list")).toBeGreaterThan(lists));
    // Nothing is left to move: only closing is offered.
    expect(within(dialog()).queryByRole("button", { name: "Move to Recycle Bin" })).toBeNull();
    fireEvent.click(within(dialog()).getByRole("button", { name: "Close" }));
    await waitFor(() => expect(screen.queryByRole("alertdialog")).toBeNull());
    // The selection moved on to a readable instance.
    await waitFor(() => expect(backend.of("instance_select")).toEqual([{ id: "a" }]));
  });

  it.each([
    ["recycleBinUnknown", "The result is unknown"],
    ["recycleBinPending", "An earlier delete is still running"],
  ] as const)(
    "%s makes no claim that nothing was deleted, offers only closing and reloads the list",
    async (code, title) => {
      const backend = await setup(three());
      backend.deleteFails = err(code, { id: "a" });
      const menu = await openMenu("Alpha");
      fireEvent.click(within(menu).getByRole("menuitem", { name: "Delete" }));
      const lists = backend.count("instances_list");
      fireEvent.click(within(dialog()).getByRole("button", { name: "Move to Recycle Bin" }));
      const panel = await within(dialog()).findByRole("alert");
      expect(within(panel).getByText(title)).toBeTruthy();
      expect(panel.textContent).not.toMatch(/nothing was deleted/i);
      expect(within(dialog()).queryByRole("button", { name: "Move to Recycle Bin" })).toBeNull();
      expect(within(dialog()).getByRole("button", { name: "Close" })).toBeTruthy();
      expect(toast).not.toHaveBeenCalled();
      await waitFor(() => expect(backend.count("instances_list")).toBeGreaterThan(lists));
    },
  );

  it.each(["recycleBinUnknown", "recycleBinUnconfirmed", "recycleBinPartial", "recycleBinPending"])(
    "%s offers Open folder to check what is left",
    async (code) => {
      const backend = await setup(three());
      backend.deleteFails = err(code, { id: "a" });
      const menu = await openMenu("Alpha");
      fireEvent.click(within(menu).getByRole("menuitem", { name: "Delete" }));
      fireEvent.click(within(dialog()).getByRole("button", { name: "Move to Recycle Bin" }));
      const panel = await within(dialog()).findByRole("alert");
      fireEvent.click(within(panel).getByRole("button", { name: "Open folder" }));
      await waitFor(() =>
        expect(backend.of("open_location")).toEqual([{ target: { kind: "instance", id: "a" } }]),
      );
    },
  );

  it("a partial move says to check both places and offers only closing", async () => {
    const backend = await setup(three());
    backend.deleteFails = err("recycleBinPartial", { id: "a" });
    const menu = await openMenu("Alpha");
    fireEvent.click(within(menu).getByRole("menuitem", { name: "Delete" }));
    fireEvent.click(within(dialog()).getByRole("button", { name: "Move to Recycle Bin" }));
    const panel = await within(dialog()).findByRole("alert");
    expect(within(panel).getByText("The instance was only partly moved")).toBeTruthy();
    expect(within(dialog()).queryByRole("button", { name: "Move to Recycle Bin" })).toBeNull();
    expect(within(dialog()).getByRole("button", { name: "Close" })).toBeTruthy();
    expect(toast).not.toHaveBeenCalled();
  });

  it("busy asks to try again, and Try again deletes once more", async () => {
    const backend = await setup(three());
    backend.deleteFails = err("busy");
    const menu = await openMenu("Alpha");
    fireEvent.click(within(menu).getByRole("menuitem", { name: "Delete" }));
    fireEvent.click(within(dialog()).getByRole("button", { name: "Move to Recycle Bin" }));
    const panel = await within(dialog()).findByRole("alert");
    expect(within(panel).getByText("Another action is in progress")).toBeTruthy();
    backend.deleteFails = null;
    fireEvent.click(within(panel).getByRole("button", { name: "Try again" }));
    await waitFor(() => expect(screen.queryByRole("alertdialog")).toBeNull());
    expect(backend.count("instance_delete")).toBe(2);
    expect(toast).toHaveBeenCalledWith("Alpha is in the Recycle Bin");
  });

  it("while moving both buttons are inert and a second click deletes once", async () => {
    const backend = await setup(three());
    let release!: () => void;
    backend.deleteGate = new Promise((r) => (release = r));
    const menu = await openMenu("Alpha");
    fireEvent.click(within(menu).getByRole("menuitem", { name: "Delete" }));
    const move = within(dialog()).getByRole("button", { name: "Move to Recycle Bin" });
    fireEvent.click(move);
    fireEvent.click(move);
    const busy = await within(dialog()).findByRole("button", { name: "Moving…" });
    expect(busy.getAttribute("aria-disabled")).toBe("true");
    expect(
      within(dialog()).getByRole("button", { name: "Keep" }).getAttribute("aria-disabled"),
    ).toBe("true");
    fireEvent.keyDown(document.body, { key: "Escape" });
    expect(screen.getByRole("alertdialog")).toBeTruthy();
    release();
    await waitFor(() => expect(screen.queryByRole("alertdialog")).toBeNull());
    expect(backend.count("instance_delete")).toBe(1);
  });

  it("after a delete focus moves to the next row's action button, else the previous, else Create instance", async () => {
    const backend = await setup(three());
    const remove = async (name: string) => {
      const menu = await openMenu(name);
      fireEvent.click(within(menu).getByRole("menuitem", { name: "Delete" }));
      fireEvent.click(within(dialog()).getByRole("button", { name: "Move to Recycle Bin" }));
      await waitFor(() => expect(screen.queryByRole("alertdialog")).toBeNull());
    };
    // The middle one: the next row.
    await remove("Beta");
    await waitFor(() => expect(document.activeElement).toBe(actionButton("Gamma")));
    // The last one: the previous row.
    await remove("Gamma");
    await waitFor(() => expect(document.activeElement).toBe(actionButton("Alpha")));
    // The only one: Create instance (in the empty state).
    await remove("Alpha");
    await waitFor(() =>
      expect(document.activeElement).toBe(screen.getByRole("button", { name: "Create instance" })),
    );
    expect(backend.count("instance_delete")).toBe(3);
  });

  it("a successful delete of the selected instance saves the fallback selection", async () => {
    const backend = await setup(three());
    expect(rowButton("Beta").getAttribute("aria-current")).toBe("true");
    const menu = await openMenu("Beta");
    fireEvent.click(within(menu).getByRole("menuitem", { name: "Delete" }));
    fireEvent.click(within(dialog()).getByRole("button", { name: "Move to Recycle Bin" }));
    await waitFor(() => expect(screen.queryByRole("alertdialog")).toBeNull());
    // The first readable instance in list order, saved in the backend and shown as selected.
    expect(backend.of("instance_select")).toEqual([{ id: "a" }]);
    await waitFor(() => expect(rowButton("Alpha").getAttribute("aria-current")).toBe("true"));
  });

  it("deleting an instance that is already gone says so, offers only closing and reloads", async () => {
    const backend = await setup(three());
    backend.deleteFails = err("instanceNotFound", { id: "c" });
    const menu = await openMenu("Gamma");
    fireEvent.click(within(menu).getByRole("menuitem", { name: "Delete" }));
    backend.list = backend.list.filter((i) => i.id !== "c");
    const lists = backend.count("instances_list");
    fireEvent.click(within(dialog()).getByRole("button", { name: "Move to Recycle Bin" }));
    const panel = await within(dialog()).findByRole("alert");
    expect(within(panel).getByText("Instance not found")).toBeTruthy();
    expect(within(dialog()).queryByRole("button", { name: "Move to Recycle Bin" })).toBeNull();
    expect(toast).not.toHaveBeenCalled();
    await waitFor(() => expect(backend.count("instances_list")).toBeGreaterThan(lists));
    fireEvent.click(within(dialog()).getByRole("button", { name: "Close" }));
    await waitFor(() => expect(screen.queryByRole("alertdialog")).toBeNull());
    await waitFor(() => expect(screen.queryByText("Gamma")).toBeNull());
  });
});

describe("InstancesView layouts", () => {
  /** `matchMedia` as the WebView has it: `narrow` answers the query of the narrow layout. */
  function mockWidth(narrow: boolean) {
    window.matchMedia = ((query: string) => ({
      matches: query === "(width < 880px)" ? narrow : false,
      media: query,
      addEventListener: () => {},
      removeEventListener: () => {},
    })) as unknown as typeof window.matchMedia;
  }
  afterEach(() => {
    delete (window as { matchMedia?: unknown }).matchMedia;
  });

  const loaded = [
    instance("a", {
      name: "Alpha",
      game: "1.14 Pre-Release 1",
      loader: { kind: "fabric", displayName: "Fabric", version: "0.17.3" },
      selected: true,
    }),
  ];

  it("below 880 CSS px: 64 px rows with version and loader under the name, no Last played, an icon-only header action", async () => {
    mockWidth(true);
    await setup(loaded);
    const row = rowButton("Alpha");
    expect(row.className).toContain("h-16");
    expect(row.className).toContain("grid-cols-[24px_minmax(0,1fr)_40px]");
    expect(within(row).getByText("1.14 Pre-Release 1").parentElement?.textContent).toBe(
      "1.14 Pre-Release 1Fabric 0.17.3",
    );
    expect(within(row).queryByText("Never played")).toBeNull();
    expect(screen.queryByText("Last played")).toBeNull();
    const action = within(header()).getByRole("button", { name: "Create instance" });
    expect(action.textContent).toBe("");
  });

  it("from 880 CSS px: 48 px rows with the five columns and the labelled header action", async () => {
    mockWidth(false);
    await setup(loaded);
    const row = rowButton("Alpha");
    expect(row.className).toContain("h-12");
    expect(row.className).toContain("grid-cols-[24px_minmax(0,1fr)_128px_168px_136px_40px]");
    expect(within(row).getByText("Never played")).toBeTruthy();
    expect(screen.getByText("Last played")).toBeTruthy();
    // A long version id stays on one line, cut with an ellipsis.
    expect(within(row).getByText("1.14 Pre-Release 1").className).toContain("truncate");
    expect(within(header()).getByRole("button", { name: "Create instance" }).textContent).toBe(
      "Create instance",
    );
  });
});
