import { act, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { toast } from "sonner";
import { describe, expect, it, vi } from "vitest";
import { TooltipProvider } from "@/components/ui/tooltip";
import i18n from "@/i18n";
import { AccountChip } from "@/features/accounts/AccountChip";
import { AccountDialogs } from "@/features/accounts/AccountDialogs";
import { useApp } from "@/stores/app";
import { account, fakeBackend } from "@/test/fakeBackend";
import {
  button,
  dialog,
  err,
  footer,
  footerButtons,
  footerLabels,
  installAccountsTestLifecycle,
  isPrimary,
  mira,
  setup,
  startBrowser,
} from "@/test/accountsHarness";

vi.mock("sonner", () => ({ toast: vi.fn(), Toaster: () => null }));
installAccountsTestLifecycle();

describe("SignInDialog phases", () => {
  it("idle: the explanation, the code link, Cancel and the primary action last", async () => {
    await setup();
    expect(screen.getByRole("heading", { name: "Add account" })).toBeTruthy();
    expect(
      within(dialog()).getByText(
        "You sign in with Microsoft in your browser. Hengst Client never sees your password.",
      ),
    ).toBeTruthy();
    expect(button("Use a code instead")).toBeTruthy();
    expect(footerLabels()).toEqual(["Cancel", "Sign in with Microsoft"]);
    expect(isPrimary(footerButtons()[1])).toBe(true);
    expect(isPrimary(footerButtons()[0])).toBe(false);
  });

  it("the first focus is the primary button", async () => {
    await setup();
    await waitFor(() => expect(document.activeElement).toBe(button("Sign in with Microsoft")));
  });

  it("starting: the primary reads Opening with a spinner and ignores clicks, Cancel stays active", async () => {
    const backend = await setup();
    fireEvent.click(button("Sign in with Microsoft"));
    const primary = await screen.findByRole("button", { name: "Opening…" });
    expect(primary.getAttribute("aria-disabled")).toBe("true");
    expect(primary.querySelector(".spinner")).not.toBeNull();
    fireEvent.click(primary);
    expect(backend.count("login_start")).toBe(1);
    expect(footerLabels()).toEqual(["Cancel", "Opening…"]);
    expect(button("Cancel").getAttribute("aria-disabled")).toBeNull();
  });

  it("waitingBrowser: spinner text, the 5 minute caption, the code link, no primary", async () => {
    const backend = await setup();
    await startBrowser(backend);
    act(() => backend.emit({ event: "browserOpened" }));
    expect(
      await within(dialog()).findByText(
        "Complete the sign-in in your browser. This window continues on its own afterward.",
      ),
    ).toBeTruthy();
    expect(within(dialog()).getByText("Waits up to 5 minutes.")).toBeTruthy();
    expect(button("Use a code instead")).toBeTruthy();
    expect(footerLabels()).toEqual(["Cancel", "Open browser again"]);
    expect(footerButtons().some(isPrimary)).toBe(false);
  });

  it("open microsoft.com/link opens the fixed target, never the address the service gave", async () => {
    const backend = await setup();
    fireEvent.click(button("Use a code instead"));
    await waitFor(() => expect(backend.count("login_start")).toBe(1));
    act(() => {
      backend.started();
      backend.emit({
        event: "deviceCode",
        data: {
          userCode: "ABCD1234",
          verificationUri: "https://evil.example/steal",
          expiresAt: Math.floor(Date.now() / 1000) + 900,
        },
      });
    });
    // The address is shown as text, never as a link.
    expect(dialog().querySelector("a")).toBeNull();
    fireEvent.click(button("Open microsoft.com/link"));
    await waitFor(() => expect(backend.count("open_url")).toBe(1));
    expect(backend.of("open_url")).toEqual([{ target: "deviceLogin" }]);
  });

  it("completing: spinner and text, both buttons inert, no way to close", async () => {
    const backend = await setup();
    await startBrowser(backend);
    act(() => {
      backend.emit({ event: "browserOpened" });
      backend.emit({ event: "completing" });
    });
    expect(await within(dialog()).findAllByText("Signing in…")).not.toHaveLength(0);
    expect(footerButtons()).toHaveLength(2);
    for (const b of footerButtons()) expect(b.getAttribute("aria-disabled")).toBe("true");
    // The close button is not disabled out of reach: it says why closing waits, and does nothing.
    const x = button("Close");
    expect(x.hasAttribute("disabled")).toBe(false);
    expect(x.getAttribute("aria-disabled")).toBe("true");
    expect(document.getElementById(x.getAttribute("aria-describedby")!)?.textContent).toBe(
      "Closing works once the sign-in is done",
    );
    fireEvent.click(x);
    fireEvent.keyDown(document.body, { key: "Escape" });
    await act(async () => {});
    expect(backend.count("login_cancel")).toBe(0);
    expect(screen.queryByRole("dialog")).not.toBeNull();
  });
});

describe("SignInDialog outcomes", () => {
  it("signedIn closes the dialog with the toast", async () => {
    const backend = await setup();
    await startBrowser(backend);
    act(() => {
      backend.emit({ event: "browserOpened" });
      backend.emit({
        event: "signedIn",
        data: { account: account("b", "Jonas_T", { selected: true }) },
      });
    });
    await waitFor(() => expect(screen.queryByRole("dialog")).toBeNull());
    expect(toast).toHaveBeenCalledWith("Signed in as Jonas_T");
  });

  it("re-signing in an account uses the resign title and names the old account when another one signed in", async () => {
    const backend = await setup([mira, account("c", "Old_Mate", { signInRequired: true })], {
      open: { id: "c", name: "Old_Mate" },
    });
    expect(screen.getByRole("heading", { name: "Sign in Old_Mate again" })).toBeTruthy();
    await startBrowser(backend);
    act(() => {
      backend.emit({ event: "browserOpened" });
      backend.emit({
        event: "signedIn",
        data: { account: account("b", "Jonas_T", { selected: true }) },
      });
    });
    await waitFor(() => expect(screen.queryByRole("dialog")).toBeNull());
    expect(toast).toHaveBeenCalledWith("Signed in as Jonas_T. Old_Mate is still signed out.");
  });

  it("re-signing in as the same account gets the plain message", async () => {
    const backend = await setup([mira], { open: { id: "a", name: "Mira_K" } });
    await startBrowser(backend);
    act(() => {
      backend.emit({ event: "browserOpened" });
      backend.emit({ event: "signedIn", data: { account: mira } });
    });
    await waitFor(() => expect(screen.queryByRole("dialog")).toBeNull());
    expect(toast).toHaveBeenCalledWith("Signed in as Mira_K");
  });

  it("appNotApproved shows the explanation, not the generic error", async () => {
    const backend = await setup();
    await startBrowser(backend);
    act(() => backend.emit({ event: "failed", data: { error: err("appNotApproved") } }));
    expect(await within(dialog()).findByText("Sign-in is not available yet")).toBeTruthy();
    expect(
      within(dialog()).getByText(/Mojang has not approved Hengst Client's Microsoft sign-in yet/),
    ).toBeTruthy();
    expect(within(dialog()).queryByRole("alert")).toBeNull();
    expect(footerLabels()).toEqual(["Close", "Learn more"]);
    expect(isPrimary(footerButtons()[1])).toBe(true);
  });

  it("learn more on appNotApproved opens the SignInHelp target", async () => {
    const backend = await setup();
    await startBrowser(backend);
    act(() => backend.emit({ event: "failed", data: { error: err("appNotApproved") } }));
    fireEvent.click(await screen.findByRole("button", { name: "Learn more" }));
    await waitFor(() => expect(backend.count("open_url")).toBe(1));
    expect(backend.of("open_url")).toEqual([{ target: "signInHelp" }]);
  });

  it("failed: the panel with Try again, which restarts the same method; focus on the title", async () => {
    const backend = await setup();
    fireEvent.click(button("Use a code instead"));
    await waitFor(() => expect(backend.count("login_start")).toBe(1));
    act(() => {
      backend.started();
      backend.emit({ event: "failed", data: { error: err("network") } });
    });
    const alert = await screen.findByRole("alert");
    expect(within(alert).getByText("Cannot reach Microsoft")).toBeTruthy();
    expect(within(alert).getByText("Check your internet connection and try again.")).toBeTruthy();
    await waitFor(() =>
      expect(document.activeElement).toBe(within(alert).getByText("Cannot reach Microsoft")),
    );
    expect(footerLabels()).toEqual(["Close"]);
    expect(isPrimary(footerButtons()[0])).toBe(false);
    fireEvent.click(within(alert).getByRole("button", { name: "Try again" }));
    await waitFor(() => expect(backend.count("login_start")).toBe(2));
    expect(backend.of("login_start")[1]).toMatchObject({ method: "deviceCode" });
  });

  it("loopbackUnavailable offers the code flow as the action", async () => {
    const backend = await setup();
    await startBrowser(backend);
    act(() => backend.emit({ event: "failed", data: { error: err("loopbackUnavailable") } }));
    const alert = await screen.findByRole("alert");
    expect(within(alert).getByText("Browser sign-in is not possible right now")).toBeTruthy();
    expect(within(alert).queryByRole("button", { name: "Try again" })).toBeNull();
    fireEvent.click(within(alert).getByRole("button", { name: "Use a code instead" }));
    await waitFor(() => expect(backend.count("login_start")).toBe(2));
    expect(backend.of("login_start")[1]).toMatchObject({ method: "deviceCode" });
  });

  it("openFailed: its own message, and the code instead of a retry with the same method", async () => {
    const backend = await setup();
    await startBrowser(backend);
    act(() => backend.emit({ event: "failed", data: { error: err("openFailed") } }));
    const alert = await screen.findByRole("alert");
    expect(within(alert).getByText("The browser could not be opened")).toBeTruthy();
    expect(within(alert).getByText("Use a code instead.")).toBeTruthy();
    expect(within(alert).queryByRole("button", { name: "Try again" })).toBeNull();
    fireEvent.click(within(alert).getByRole("button", { name: "Use a code instead" }));
    await waitFor(() => expect(backend.count("login_start")).toBe(2));
    expect(backend.of("login_start")[1]).toMatchObject({ method: "deviceCode" });
  });

  it("noXboxAccount shows the link to the Xbox page", async () => {
    const backend = await setup();
    await startBrowser(backend);
    act(() =>
      backend.emit({
        event: "failed",
        data: {
          error: { ...err("noXboxAccount"), action: { kind: "openUrl", target: "xboxSignup" } },
        },
      }),
    );
    const alert = await screen.findByRole("alert");
    fireEvent.click(within(alert).getByRole("button", { name: "Create Xbox profile" }));
    await waitFor(() => expect(backend.of("open_url")).toEqual([{ target: "xboxSignup" }]));
  });

  it("appMisconfigured has no retry, only the details", async () => {
    const backend = await setup();
    await startBrowser(backend);
    act(() => backend.emit({ event: "failed", data: { error: err("appMisconfigured") } }));
    const alert = await screen.findByRole("alert");
    expect(within(alert).queryByRole("button", { name: "Try again" })).toBeNull();
    expect(within(alert).getByRole("button", { name: "Details" })).toBeTruthy();
  });

  it("missingClientId from the backend shows the no-client-id wording", async () => {
    const backend = await setup();
    await startBrowser(backend);
    act(() => backend.emit({ event: "failed", data: { error: err("missingClientId") } }));
    const alert = await screen.findByRole("alert");
    expect(within(alert).getByText("This build cannot sign in")).toBeTruthy();
  });

  it("a failing login_start is a failure of that sign-in", async () => {
    const backend = fakeBackend({ accounts: [mira], canSignIn: true });
    backend.startFails = err("invalidInput");
    render(
      <TooltipProvider>
        <AccountChip />
        <AccountDialogs />
      </TooltipProvider>,
    );
    await useApp.getState().loadAccounts();
    act(() => useApp.getState().openSignIn(null));
    fireEvent.click(button("Sign in with Microsoft"));
    expect(await screen.findByRole("alert")).toBeTruthy();
  });
});

describe("SignInDialog closing and cancelling", () => {
  it("escape cancels the running login with the op from Started and closes", async () => {
    const backend = await setup();
    await startBrowser(backend);
    act(() => backend.emit({ event: "browserOpened" }));
    fireEvent.keyDown(document.body, { key: "Escape" });
    await waitFor(() => expect(backend.count("login_cancel")).toBe(1));
    expect(backend.of("login_cancel")).toEqual([{ op: 1 }]);
    await waitFor(() => expect(screen.queryByRole("dialog")).toBeNull());
  });

  it("escape in idle just closes, there is nothing to cancel", async () => {
    const backend = await setup();
    fireEvent.keyDown(document.body, { key: "Escape" });
    await waitFor(() => expect(screen.queryByRole("dialog")).toBeNull());
    expect(backend.count("login_cancel")).toBe(0);
  });

  it("closing returns focus to the chip", async () => {
    await setup();
    fireEvent.click(button("Cancel"));
    await waitFor(() => expect(screen.queryByRole("dialog")).toBeNull());
    await waitFor(() =>
      expect(document.activeElement).toBe(screen.getByRole("button", { name: "Account: Mira_K" })),
    );
  });

  it("escape right after the click, before the op is known, cancels it as soon as it is", async () => {
    const backend = await setup();
    fireEvent.click(button("Sign in with Microsoft"));
    // No `started` yet: the op is unknown. Leaving must not leave a sign-in running.
    fireEvent.keyDown(document.body, { key: "Escape" });
    await waitFor(() => expect(screen.queryByRole("dialog")).toBeNull());
    await waitFor(() => expect(backend.count("login_cancel")).toBe(1));
    expect(backend.of("login_cancel")).toEqual([{ op: 1 }]);
  });

  it("a cancel that comes back false does not close or assume cancelled: the sign-in finishes", async () => {
    const backend = await setup();
    await startBrowser(backend);
    act(() => backend.emit({ event: "browserOpened" }));
    backend.cancelAnswer = false;
    fireEvent.click(button("Cancel"));
    await waitFor(() => expect(backend.count("login_cancel")).toBe(1));
    await act(async () => {});
    expect(screen.queryByRole("dialog")).not.toBeNull();
    // The backend was saving: `completing`, then the terminal event.
    act(() => backend.emit({ event: "completing" }));
    expect((await within(dialog()).findAllByText("Signing in…")).length).toBeGreaterThan(0);
    act(() =>
      backend.emit({
        event: "signedIn",
        data: { account: account("b", "Jonas_T", { selected: true }) },
      }),
    );
    await waitFor(() => expect(screen.queryByRole("dialog")).toBeNull());
    expect(toast).toHaveBeenCalledWith("Signed in as Jonas_T");
  });

  it("outside pointer-down does not close the dialog", async () => {
    const backend = await setup();
    fireEvent.pointerDown(document.body);
    fireEvent.mouseDown(document.body);
    fireEvent.click(document.body);
    await act(async () => {});
    expect(screen.queryByRole("dialog")).not.toBeNull();
    expect(backend.count("login_cancel")).toBe(0);
  });

  it("the close button cancels like Cancel", async () => {
    const backend = await setup();
    await startBrowser(backend);
    fireEvent.click(button("Close"));
    await waitFor(() => expect(backend.of("login_cancel")).toEqual([{ op: 1 }]));
    await waitFor(() => expect(screen.queryByRole("dialog")).toBeNull());
  });

  it("failed: Close closes without cancelling anything", async () => {
    const backend = await setup();
    await startBrowser(backend);
    act(() => backend.emit({ event: "failed", data: { error: err("network") } }));
    await screen.findByRole("alert");
    fireEvent.click(within(footer()).getByRole("button", { name: "Close" }));
    await waitFor(() => expect(screen.queryByRole("dialog")).toBeNull());
    expect(backend.count("login_cancel")).toBe(0);
  });
});

describe("SignInDialog focus, announcements and races", () => {
  it("starting keeps the focus inside the dialog", async () => {
    await setup();
    fireEvent.click(button("Sign in with Microsoft"));
    await screen.findByRole("button", { name: "Opening…" });
    await waitFor(() => expect(dialog().contains(document.activeElement)).toBe(true));
    expect(document.activeElement).not.toBe(document.body);
  });

  it("the device code is announced once when it appears, character by character", async () => {
    const backend = await setup();
    fireEvent.click(button("Use a code instead"));
    await waitFor(() => expect(backend.count("login_start")).toBe(1));
    act(() => {
      backend.started();
      backend.emit({
        event: "deviceCode",
        data: {
          userCode: "ABCD1234",
          verificationUri: "https://www.microsoft.com/link",
          expiresAt: Math.floor(Date.now() / 1000) + 900,
        },
      });
    });
    const live = await screen.findByText("Your code is A B C D 1 2 3 4");
    expect(live.getAttribute("role")).toBe("status");
    expect(live.closest("[aria-live]")?.getAttribute("aria-live")).toBe("polite");
  });

  it("announces a code once: switching the language does not announce it again", async () => {
    const backend = await setup();
    fireEvent.click(button("Use a code instead"));
    await waitFor(() => expect(backend.count("login_start")).toBe(1));
    act(() => {
      backend.started();
      backend.emit({
        event: "deviceCode",
        data: {
          userCode: "ABCD1234",
          verificationUri: "https://www.microsoft.com/link",
          expiresAt: Math.floor(Date.now() / 1000) + 900,
        },
      });
    });
    const live = await screen.findByText("Your code is A B C D 1 2 3 4");
    let changes = 0;
    const observer = new MutationObserver((records) => (changes += records.length));
    observer.observe(live, { childList: true, characterData: true, subtree: true });
    await act(async () => {
      await i18n.changeLanguage("de");
    });
    await act(async () => {});
    observer.disconnect();
    expect(changes).toBe(0);
    expect(live.textContent).toBe("Your code is A B C D 1 2 3 4");
    await act(async () => {
      await i18n.changeLanguage("en");
    });
  });

  it("shows the address the service gave as text, only if it is https", async () => {
    const backend = await setup();
    fireEvent.click(button("Use a code instead"));
    await waitFor(() => expect(backend.count("login_start")).toBe(1));
    const code = (uri: string) =>
      act(() => {
        backend.started();
        backend.emit({
          event: "deviceCode",
          data: {
            userCode: "ABCD1234",
            verificationUri: uri,
            expiresAt: Math.floor(Date.now() / 1000) + 900,
          },
        });
      });
    code("https://www.microsoft.com/link");
    expect(
      await within(dialog()).findByText("Address: https://www.microsoft.com/link"),
    ).toBeTruthy();
    expect(dialog().querySelector("a")).toBeNull();
  });

  it("a verification address that is not https is not shown", async () => {
    const backend = await setup();
    fireEvent.click(button("Use a code instead"));
    await waitFor(() => expect(backend.count("login_start")).toBe(1));
    act(() => {
      backend.started();
      backend.emit({
        event: "deviceCode",
        data: {
          userCode: "ABCD1234",
          verificationUri: "javascript:alert(1)",
          expiresAt: Math.floor(Date.now() / 1000) + 900,
        },
      });
    });
    await within(dialog()).findByText("ABCD1234");
    expect(within(dialog()).queryByText(/Address:/)).toBeNull();
  });

  it("escape during the switch to the code closes the dialog and leaves nothing running", async () => {
    const backend = await setup();
    await startBrowser(backend);
    act(() => backend.emit({ event: "browserOpened" }));
    fireEvent.click(button("Use a code instead"));
    fireEvent.keyDown(document.body, { key: "Escape" });
    await waitFor(() => expect(screen.queryByRole("dialog")).toBeNull());
    await act(async () => {
      await new Promise((r) => setTimeout(r, 50));
    });
    // The switch's own cancel ended the browser sign-in; no code sign-in was started after the close.
    expect(backend.count("login_start")).toBe(1);
  });

  it("escape during the switch whose cancel is refused (the account is being saved) keeps the dialog until the sign-in ends", async () => {
    const backend = await setup();
    await startBrowser(backend);
    act(() => backend.emit({ event: "browserOpened" }));
    backend.cancelAnswer = false;
    fireEvent.click(button("Use a code instead"));
    fireEvent.keyDown(document.body, { key: "Escape" });
    await waitFor(() => expect(backend.count("login_cancel")).toBeGreaterThanOrEqual(1));
    await act(async () => {
      await new Promise((r) => setTimeout(r, 50));
    });
    // Neither a new sign-in nor a close: the browser sign-in is saving and goes on.
    expect(backend.count("login_start")).toBe(1);
    expect(screen.queryByRole("dialog")).not.toBeNull();
    act(() => backend.emit({ event: "completing" }));
    act(() =>
      backend.emit({
        event: "signedIn",
        data: { account: account("b", "Jonas_T", { selected: true }) },
      }),
    );
    await waitFor(() => expect(screen.queryByRole("dialog")).toBeNull());
    expect(toast).toHaveBeenCalledWith("Signed in as Jonas_T");
  });

  it("escape during the switch whose cancel is rejected closes the dialog, as a plain cancel does", async () => {
    const backend = await setup();
    await startBrowser(backend);
    act(() => backend.emit({ event: "browserOpened" }));
    backend.cancelFails = { code: "internal", params: {}, action: null, detail: "x" };
    fireEvent.click(button("Use a code instead"));
    fireEvent.keyDown(document.body, { key: "Escape" });
    await waitFor(() => expect(screen.queryByRole("dialog")).toBeNull());
    await act(async () => {
      await new Promise((r) => setTimeout(r, 50));
    });
    // No code sign-in was started after the close.
    expect(backend.count("login_start")).toBe(1);
  });

  it("the close button during completing shows why it waits, as a tooltip as well as a description", async () => {
    const backend = await setup();
    await startBrowser(backend);
    act(() => {
      backend.emit({ event: "browserOpened" });
      backend.emit({ event: "completing" });
    });
    const x = await within(dialog()).findByRole("button", { name: "Close" });
    expect(x.getAttribute("aria-disabled")).toBe("true");
    // It does not look pressable: the ghost hover and pressed fills are switched off while it waits
    // (jsdom computes no :hover, so the classes are what can be checked here).
    expect(x.className).toContain("aria-disabled:hover:bg-transparent");
    expect(x.className).toContain("aria-disabled:active:bg-transparent");
    fireEvent.keyDown(document.body, { key: "Tab" });
    act(() => x.focus());
    await waitFor(() =>
      expect(document.querySelector("[data-slot=tooltip-content]")?.textContent).toBe(
        "Closing works once the sign-in is done",
      ),
    );
  });

  it("a rejected login_cancel still closes the dialog and raises no unhandled rejection", async () => {
    const unhandled: unknown[] = [];
    const listener = (e: PromiseRejectionEvent) => unhandled.push(e.reason);
    window.addEventListener("unhandledrejection", listener);
    const backend = await setup();
    await startBrowser(backend);
    act(() => backend.emit({ event: "browserOpened" }));
    backend.cancelFails = { code: "internal", params: {}, action: null, detail: "x" };
    fireEvent.click(button("Cancel"));
    await waitFor(() => expect(screen.queryByRole("dialog")).toBeNull());
    await act(async () => {
      await new Promise((r) => setTimeout(r, 30));
    });
    window.removeEventListener("unhandledrejection", listener);
    expect(unhandled).toEqual([]);
  });
});

describe("SignInDialog click guards and stale events", () => {
  it("double click on sign in starts one login", async () => {
    const backend = await setup();
    const primary = button("Sign in with Microsoft");
    fireEvent.click(primary);
    fireEvent.click(primary);
    fireEvent.click(primary);
    await act(async () => {});
    expect(backend.count("login_start")).toBe(1);
  });

  it("a late event of an earlier attempt does not touch the next one", async () => {
    const backend = await setup();
    await startBrowser(backend);
    act(() => backend.emit({ event: "failed", data: { error: err("network") } }));
    const alert = await screen.findByRole("alert");
    fireEvent.click(within(alert).getByRole("button", { name: "Try again" }));
    await waitFor(() => expect(backend.count("login_start")).toBe(2));
    // The first channel speaks again (a late event): it is nobody's business now.
    act(() => backend.emit({ event: "signedIn", data: { account: mira } }, 0));
    act(() => backend.emit({ event: "cancelled" }, 0));
    expect(screen.queryByRole("dialog")).not.toBeNull();
    expect(toast).not.toHaveBeenCalled();
    // The new attempt works as usual.
    act(() => backend.started(1));
    act(() => backend.emit({ event: "browserOpened" }, 1));
    expect(await within(dialog()).findByText("Waits up to 5 minutes.")).toBeTruthy();
  });

  it("use a code instead cancels the browser sign-in with its op, then starts the code; its cancelled is ignored", async () => {
    const backend = await setup();
    await startBrowser(backend);
    act(() => backend.emit({ event: "browserOpened" }));
    fireEvent.click(button("Use a code instead"));
    await waitFor(() => expect(backend.count("login_start")).toBe(2));
    expect(backend.of("login_cancel")).toEqual([{ op: 1 }]);
    expect(backend.of("login_start")[1]).toMatchObject({ method: "deviceCode" });
    // The cancelled browser sign-in reports `cancelled`: that is expected, not a reason to close.
    act(() => backend.emit({ event: "cancelled" }, 0));
    expect(screen.queryByRole("dialog")).not.toBeNull();
    act(() => backend.started(1));
    act(() =>
      backend.emit(
        {
          event: "deviceCode",
          data: {
            userCode: "WXYZ",
            verificationUri: "x",
            expiresAt: Math.floor(Date.now() / 1000) + 600,
          },
        },
        1,
      ),
    );
    expect(await within(dialog()).findByText("WXYZ")).toBeTruthy();
  });

  it("use a code instead during the final save starts nothing", async () => {
    const backend = await setup();
    await startBrowser(backend);
    act(() => backend.emit({ event: "browserOpened" }));
    backend.cancelAnswer = false;
    fireEvent.click(button("Use a code instead"));
    await waitFor(() => expect(backend.count("login_cancel")).toBe(1));
    await act(async () => {});
    expect(backend.count("login_start")).toBe(1);
    // The browser sign-in goes on and ends as usual.
    act(() => backend.emit({ event: "signedIn", data: { account: mira } }));
    await waitFor(() => expect(screen.queryByRole("dialog")).toBeNull());
  });

  it("open browser again calls the backend with the op and shows the inline failure when it fails", async () => {
    const backend = await setup();
    await startBrowser(backend);
    act(() => backend.emit({ event: "browserOpened" }));
    fireEvent.click(button("Open browser again"));
    await waitFor(() => expect(backend.of("login_reopen_browser")).toEqual([{ op: 1 }]));
    expect(within(dialog()).queryByText(/could not be opened/)).toBeNull();
    backend.reopenFails = err("openFailed");
    await act(async () => {
      await new Promise((r) => setTimeout(r, 0));
    });
    fireEvent.click(button("Open browser again"));
    expect(
      await within(dialog()).findByText("The browser could not be opened. Use a code instead."),
    ).toBeTruthy();
    // The wait goes on.
    expect(within(dialog()).getByText("Waits up to 5 minutes.")).toBeTruthy();
    expect(button("Use a code instead")).toBeTruthy();
  });

  it("every phase has at most two footer buttons and the primary one is last", async () => {
    const backend = await setup();
    const check = () => {
      const buttons = footerButtons();
      expect(buttons.length).toBeLessThanOrEqual(2);
      const primaries = buttons.filter(isPrimary);
      expect(primaries.length).toBeLessThanOrEqual(1);
      if (primaries.length === 1) expect(buttons.at(-1)).toBe(primaries[0]);
    };
    check(); // idle
    await startBrowser(backend);
    check(); // starting
    act(() => backend.emit({ event: "browserOpened" }));
    check(); // waitingBrowser
    act(() => backend.emit({ event: "completing" }));
    check(); // completing
    act(() => backend.emit({ event: "failed", data: { error: err("network") } }));
    await screen.findByRole("alert");
    check(); // failed
  });
});

describe("no client id", () => {
  it("no client id never opens the sign-in dialog: the chip shows the no-client-id dialog", async () => {
    const backend = await setup([], { canSignIn: false, open: "none" });
    fireEvent.click(await screen.findByRole("button", { name: "Sign in" }));
    const d = await screen.findByRole("alertdialog");
    expect(within(d).getByText("This build cannot sign in")).toBeTruthy();
    expect(screen.queryByRole("dialog")).toBeNull();
    expect(backend.count("login_start")).toBe(0);
    // Close it before the test ends: an open dialog that is only unmounted leaves Base UI's focus
    // return pending, which steals the focus from the next test's dialog.
    fireEvent.click(within(d).getByRole("button", { name: "Close" }));
    await waitFor(() => expect(screen.queryByRole("alertdialog")).toBeNull());
  });

  it("build instructions in the no-client-id dialog open the BuildHelp target", async () => {
    const backend = await setup([], { canSignIn: false, open: "none" });
    fireEvent.click(await screen.findByRole("button", { name: "Sign in" }));
    const d = await screen.findByRole("alertdialog");
    expect(within(d).getByText(/built without a Microsoft client id/)).toBeTruthy();
    // Close is the first focus; the primary action is last.
    await waitFor(() =>
      expect(document.activeElement).toBe(within(d).getByRole("button", { name: "Close" })),
    );
    const buttons = [...d.querySelectorAll<HTMLElement>("[data-slot='dialog-footer'] button")];
    expect(buttons.map((b) => b.textContent)).toEqual(["Close", "Build instructions"]);
    expect(isPrimary(buttons[1])).toBe(true);
    fireEvent.click(buttons[1]);
    await waitFor(() => expect(backend.count("open_url")).toBe(1));
    expect(backend.of("open_url")).toEqual([{ target: "buildHelp" }]);
  });
});
