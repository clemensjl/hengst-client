import { clearMocks } from "@tauri-apps/api/mocks";
import { act, cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { toast } from "sonner";
import { afterEach, beforeEach, expect, vi } from "vitest";
import { TooltipProvider } from "@/components/ui/tooltip";
import { initI18n } from "@/i18n";
import type { ErrorDto } from "@/ipc/bindings/ErrorDto";
import { useApp } from "@/stores/app";
import { account, fakeBackend } from "@/test/fakeBackend";
import { AccountChip } from "@/features/accounts/AccountChip";
import { AccountDialogs } from "@/features/accounts/AccountDialogs";

const initial = useApp.getState();
export const mira = account("a", "Mira_K", { selected: true });
export const err = (code: string): ErrorDto =>
  ({ code, params: {}, action: null, detail: "d" }) as ErrorDto;

/** Registers the lifecycle every test of the accounts dialogs needs. Call it once at the top of a test file. */
export function installAccountsTestLifecycle() {
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
    // Base UI returns focus and releases its scroll lock a moment after a dialog is unmounted; let
    // that pass, or it lands in the next test.
    await new Promise((resolve) => setTimeout(resolve, 30));
  });
}

/** The chip and the dialogs, as the app mounts them, with the list already loaded. */
export async function setup(
  accounts = [mira],
  options: { canSignIn?: boolean; open?: { id: string; name: string } | null | "none" } = {},
) {
  const backend = fakeBackend({ accounts, canSignIn: options.canSignIn ?? true });
  render(
    <TooltipProvider>
      <AccountChip />
      <AccountDialogs />
    </TooltipProvider>,
  );
  await useApp.getState().loadAccounts();
  const resign = options.open === "none" ? undefined : (options.open ?? null);
  if (resign !== undefined) act(() => useApp.getState().openSignIn(resign));
  return backend;
}

export const dialog = () => screen.getByRole("dialog");
export const footer = () => dialog().querySelector<HTMLElement>('[data-slot="dialog-footer"]')!;
export const footerButtons = () => [...footer().querySelectorAll<HTMLElement>("button")];
/** What a person reads on the buttons: the text without what is hidden from them. */
const readable = (el: HTMLElement) => {
  const copy = el.cloneNode(true) as HTMLElement;
  copy.querySelectorAll("[aria-hidden='true']").forEach((n) => n.remove());
  return copy.textContent;
};
export const footerLabels = () => footerButtons().map(readable);
export const button = (name: string | RegExp) => within(dialog()).getByRole("button", { name });
/** The primary button of the design (the Button variant), not a class name. */
export const isPrimary = (el: Element) => el.getAttribute("data-variant") === "primary";

/**
 * Ends a test that ran on fake timers by closing its dialog on real ones: Base UI's focus return
 * is timer-driven, and a dialog that is only unmounted under fake timers leaves it undone for the
 * next test.
 */
export async function closeWithRealTimers() {
  vi.useRealTimers();
  fireEvent.click(button("Cancel"));
  await waitFor(() => expect(screen.queryByRole("dialog")).toBeNull());
}

/** Starts the browser sign-in and delivers `started`. */
export async function startBrowser(backend: ReturnType<typeof fakeBackend>) {
  fireEvent.click(button("Sign in with Microsoft"));
  await waitFor(() => expect(backend.count("login_start")).toBe(1));
  act(() => backend.started());
}
