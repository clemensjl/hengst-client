import { act, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { toast } from "sonner";
import { afterEach, describe, expect, it, vi } from "vitest";
import { TooltipProvider } from "@/components/ui/tooltip";
import { AccountChip } from "@/features/accounts/AccountChip";
import { AccountDialogs } from "@/features/accounts/AccountDialogs";
import { useApp } from "@/stores/app";
import { account, fakeBackend } from "@/test/fakeBackend";
import { installAccountsTestLifecycle, mira, setup } from "@/test/accountsHarness";

vi.mock("sonner", () => ({ toast: vi.fn(), Toaster: () => null }));
installAccountsTestLifecycle();

const jonas = account("b", "Jonas_T");
const chip = (name = "Account: Mira_K") => screen.getByRole("button", { name });
const openMenu = async (name?: string) => {
  fireEvent.click(chip(name));
  return screen.findByRole("menu");
};

afterEach(() => {
  vi.restoreAllMocks();
});

describe("AccountChip", () => {
  it("shows the selected account name and switches on click", async () => {
    const backend = await setup([mira, jonas], { open: "none" });
    expect(chip().textContent).toContain("Mira_K");
    const menu = await openMenu();
    const items = within(menu).getAllByRole("menuitemradio");
    expect(items.map((i) => i.textContent)).toEqual(["Mira_K", "Jonas_T"]);
    expect(items[0].getAttribute("aria-checked")).toBe("true");
    expect(items[1].getAttribute("aria-checked")).toBe("false");
    fireEvent.click(items[1]);
    await waitFor(() => expect(backend.of("account_select")).toEqual([{ id: "b" }]));
  });

  it("the initial circle shows the first letter, the name is the accessible name 'Account: name'", async () => {
    await setup([account("a", "mira", { selected: true })], { open: "none" });
    const button = chip("Account: mira");
    expect(within(button).getByText("M")).toBeTruthy();
    expect(button.getAttribute("aria-haspopup")).toBe("menu");
  });

  it("without an account the chip is a Sign in button that opens the dialog", async () => {
    await setup([], { open: "none" });
    fireEvent.click(await screen.findByRole("button", { name: "Sign in" }));
    expect(await screen.findByRole("dialog")).toBeTruthy();
    expect(screen.getByRole("heading", { name: "Add account" })).toBeTruthy();
    fireEvent.click(screen.getByRole("button", { name: "Cancel" }));
    await waitFor(() => expect(screen.queryByRole("dialog")).toBeNull());
  });

  it("without a client id the menu's Add account stays enabled and shows the no-client-id dialog", async () => {
    const backend = await setup([mira], { canSignIn: false, open: "none" });
    const menu = await openMenu();
    const add = within(menu).getByRole("menuitem", { name: "Add account" });
    expect(add.hasAttribute("data-disabled")).toBe(false);
    fireEvent.click(add);
    const dialog = await screen.findByRole("alertdialog");
    expect(within(dialog).getByText("This build cannot sign in")).toBeTruthy();
    expect(screen.queryByRole("dialog")).toBeNull();
    expect(backend.count("login_start")).toBe(0);
    fireEvent.click(within(dialog).getByRole("button", { name: "Close" }));
    await waitFor(() => expect(screen.queryByRole("alertdialog")).toBeNull());
  });

  it("while the list loads for the first time the chip is a static block after 200 ms", async () => {
    vi.useFakeTimers({ toFake: ["setTimeout", "clearTimeout"] });
    fakeBackend();
    const { container } = render(
      <TooltipProvider>
        <AccountChip />
      </TooltipProvider>,
    );
    expect(container.querySelector("[aria-busy]")).toBeNull();
    act(() => {
      vi.advanceTimersByTime(250);
    });
    expect(container.querySelector("[aria-busy]")).not.toBeNull();
    vi.useRealTimers();
  });

  it("the account list failing shows Accounts unavailable with a retry in the menu", async () => {
    const backend = fakeBackend();
    backend.listFails = { code: "fileSystem", params: {}, action: null, detail: "x" };
    render(
      <TooltipProvider>
        <AccountChip />
        <AccountDialogs />
      </TooltipProvider>,
    );
    await useApp.getState().loadAccounts();
    const button = await screen.findByRole("button", { name: "Accounts unavailable" });
    fireEvent.click(button);
    const menu = await screen.findByRole("menu");
    expect(within(menu).getByText("A file could not be read or written")).toBeTruthy();
    backend.listFails = null;
    backend.accounts = { accounts: [mira], canSignIn: true };
    fireEvent.click(within(menu).getByRole("menuitem", { name: "Try again" }));
    expect(await screen.findByRole("button", { name: "Account: Mira_K" })).toBeTruthy();
  });

  it("the dev account carries the Development build badge", async () => {
    await setup([mira, account("offline-dev", "Dev", { kind: "offlineDev" })], { open: "none" });
    const menu = await openMenu();
    const items = within(menu).getAllByRole("menuitemradio");
    expect(within(items[1]).getByText("Development build")).toBeTruthy();
    expect(within(items[0]).queryByText("Development build")).toBeNull();
  });

  it("an account that needs to sign in again shows the badge on the chip and in the menu; choosing it starts that sign-in", async () => {
    const stale = account("c", "Old_Mate", { selected: true, signInRequired: true });
    await setup([stale, jonas], { open: "none" });
    expect(within(chip("Account: Old_Mate")).getByText("Sign in again")).toBeTruthy();
    const menu = await openMenu("Account: Old_Mate");
    const items = within(menu).getAllByRole("menuitemradio");
    expect(within(items[0]).getByText("Sign in again")).toBeTruthy();
    fireEvent.click(items[0]);
    expect(await screen.findByRole("heading", { name: "Sign in Old_Mate again" })).toBeTruthy();
    fireEvent.click(screen.getByRole("button", { name: "Cancel" }));
    await waitFor(() => expect(screen.queryByRole("dialog")).toBeNull());
  });

  it("a name that is cut off shows the full name in a tooltip reachable by keyboard", async () => {
    const wide = vi.spyOn(HTMLElement.prototype, "scrollWidth", "get").mockReturnValue(300);
    vi.spyOn(HTMLElement.prototype, "clientWidth", "get").mockReturnValue(100);
    expect(wide).toBeDefined();
    render(
      <TooltipProvider delay={0}>
        <AccountChip />
      </TooltipProvider>,
    );
    fakeBackend({
      accounts: [account("a", "Sixteen_Chars_Nm", { selected: true })],
      canSignIn: true,
    });
    await useApp.getState().loadAccounts();
    const button = await screen.findByRole("button", { name: "Account: Sixteen_Chars_Nm" });
    // A key press first: the focus that follows is a keyboard focus.
    fireEvent.keyDown(document.body, { key: "Tab" });
    act(() => button.focus());
    await waitFor(() =>
      expect(document.querySelector("[data-slot=tooltip-content]")?.textContent).toBe(
        "Sixteen_Chars_Nm",
      ),
    );
  });

  it("the chip names the account and, in its description, what it opens; a stale login is described too", async () => {
    await setup([account("c", "Old_Mate", { selected: true, signInRequired: true })], {
      open: "none",
    });
    const button = chip("Account: Old_Mate");
    const description = button
      .getAttribute("aria-describedby")!
      .split(" ")
      .map((id) => document.getElementById(id)?.textContent)
      .join(" ");
    expect(description).toContain("Opens the account menu");
    expect(description).toContain("Sign in again");
  });

  it("with accounts but none selected the chip says so and never shows another account's name", async () => {
    await setup([account("a", "Mira_K"), jonas], { open: "none" });
    expect(screen.queryByRole("button", { name: /Account: / })).toBeNull();
    const button = screen.getByRole("button", { name: "Choose account" });
    expect(button.textContent).not.toContain("Mira_K");
    fireEvent.click(button);
    const menu = await screen.findByRole("menu");
    for (const item of within(menu).getAllByRole("menuitemradio")) {
      expect(item.getAttribute("aria-checked")).toBe("false");
    }
  });

  it("choosing an account closes the menu", async () => {
    await setup([mira, jonas], { open: "none" });
    const menu = await openMenu();
    fireEvent.click(within(menu).getAllByRole("menuitemradio")[1]);
    await waitFor(() => expect(screen.queryByRole("menu")).toBeNull());
  });

  it("a failed select says so and the chip stays truthful: the list is read again and nothing switched", async () => {
    const backend = await setup([mira, jonas], { open: "none" });
    backend.selectFails = { code: "accountNotFound", params: {}, action: null, detail: "x" };
    const menu = await openMenu();
    fireEvent.click(within(menu).getAllByRole("menuitemradio")[1]);
    await waitFor(() => expect(toast).toHaveBeenCalledWith("Account not found"));
    await waitFor(() => expect(backend.count("accounts_list")).toBe(2));
    expect(chip("Account: Mira_K")).toBeTruthy();
    expect(screen.queryByRole("button", { name: "Account: Jonas_T" })).toBeNull();
  });
});

describe("keyboard", () => {
  it("the arrow keys open the menu and move through the accounts, Enter selects", async () => {
    const backend = await setup([mira, jonas], { open: "none" });
    const button = chip();
    act(() => button.focus());
    fireEvent.keyDown(button, { key: "ArrowDown" });
    const menu = await screen.findByRole("menu");
    const items = within(menu).getAllByRole("menuitemradio");
    await waitFor(() => expect(document.activeElement).toBe(items[0]));
    fireEvent.keyDown(items[0], { key: "ArrowDown" });
    await waitFor(() => expect(document.activeElement).toBe(items[1]));
    fireEvent.keyDown(items[1], { key: "Enter" });
    await waitFor(() => expect(backend.of("account_select")).toEqual([{ id: "b" }]));
  });

  it("Escape closes the menu and the focus is back on the chip", async () => {
    await setup([mira, jonas], { open: "none" });
    const button = chip();
    act(() => button.focus());
    fireEvent.keyDown(button, { key: "ArrowDown" });
    await screen.findByRole("menu");
    fireEvent.keyDown(document.activeElement ?? document.body, { key: "Escape" });
    await waitFor(() => expect(screen.queryByRole("menu")).toBeNull());
    await waitFor(() => expect(document.activeElement).toBe(chip()));
  });
});

describe("remove account", () => {
  const openRemove = async (backend: ReturnType<typeof fakeBackend>) => {
    const menu = await openMenu();
    fireEvent.click(within(menu).getByRole("menuitem", { name: "Sign out" }));
    const sub = await screen.findAllByRole("menu");
    const list = sub[sub.length - 1];
    const items = within(list).getAllByRole("menuitem");
    expect(items.map((i) => i.textContent)).toEqual(["Mira_K", "Jonas_T"]);
    fireEvent.click(items[1]);
    expect(backend.count("account_remove")).toBe(0);
    return screen.findByRole("alertdialog");
  };

  it("remove asks first and names the account; account_remove only after the confirmation", async () => {
    const backend = await setup([mira, jonas], { open: "none" });
    const dialog = await openRemove(backend);
    expect(within(dialog).getByRole("heading", { name: "Sign out Jonas_T?" })).toBeTruthy();
    expect(
      within(dialog).getByText(
        "The saved sign-in is removed from this PC. Your Microsoft account stays unchanged.",
      ),
    ).toBeTruthy();
    expect(backend.count("account_remove")).toBe(0);
    fireEvent.click(within(dialog).getByRole("button", { name: "Sign out" }));
    await waitFor(() => expect(backend.of("account_remove")).toEqual([{ id: "b" }]));
    await waitFor(() => expect(toast).toHaveBeenCalledWith("Signed out Jonas_T"));
    await waitFor(() => expect(screen.queryByRole("alertdialog")).toBeNull());
  });

  it("remove keeps focus on Keep first, and Keep or Escape leave the account alone and return focus to the chip", async () => {
    const backend = await setup([mira, jonas], { open: "none" });
    const dialog = await openRemove(backend);
    await waitFor(() =>
      expect(document.activeElement).toBe(within(dialog).getByRole("button", { name: "Keep" })),
    );
    const buttons = [...dialog.querySelectorAll("[data-slot='dialog-footer'] button")];
    expect(buttons.map((b) => b.textContent)).toEqual(["Keep", "Sign out"]);
    fireEvent.click(buttons[0]);
    await waitFor(() => expect(screen.queryByRole("alertdialog")).toBeNull());
    expect(backend.count("account_remove")).toBe(0);
    await waitFor(() => expect(document.activeElement).toBe(chip()));
  });

  it("a double click on Sign out removes once", async () => {
    const backend = await setup([mira, jonas], { open: "none" });
    const dialog = await openRemove(backend);
    const confirm = within(dialog).getByRole("button", { name: "Sign out" });
    fireEvent.click(confirm);
    fireEvent.click(confirm);
    await waitFor(() => expect(backend.count("account_remove")).toBe(1));
  });

  it("a failing sign-out stays in the dialog with the error and a retry; the account stays", async () => {
    const backend = await setup([mira, jonas], { open: "none" });
    backend.removeFails = {
      code: "credentialStore",
      params: {},
      action: { kind: "retry" },
      detail: "denied",
    };
    const dialog = await openRemove(backend);
    fireEvent.click(within(dialog).getByRole("button", { name: "Sign out" }));
    const alert = await within(dialog).findByRole("alert");
    expect(within(alert).getByText("Access to the stored credentials failed")).toBeTruthy();
    expect(screen.queryByRole("alertdialog")).not.toBeNull();
    expect(toast).not.toHaveBeenCalled();
    backend.removeFails = null;
    fireEvent.click(within(alert).getByRole("button", { name: "Try again" }));
    await waitFor(() => expect(backend.count("account_remove")).toBe(2));
    await waitFor(() => expect(screen.queryByRole("alertdialog")).toBeNull());
  });

  it("the sign-out entry is hidden when there is no Microsoft account to sign out", async () => {
    await setup([account("offline-dev", "Dev", { kind: "offlineDev", selected: true })], {
      open: "none",
    });
    const menu = await openMenu("Account: Dev");
    expect(within(menu).queryByRole("menuitem", { name: "Sign out" })).toBeNull();
    expect(within(menu).getByRole("menuitem", { name: "Add account" })).toBeTruthy();
  });
});
