import { act, fireEvent } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import {
  button,
  closeWithRealTimers,
  dialog,
  footerButtons,
  footerLabels,
  installAccountsTestLifecycle,
  isPrimary,
  setup,
} from "@/test/accountsHarness";
import { within } from "@testing-library/react";

vi.mock("sonner", () => ({ toast: vi.fn(), Toaster: () => null }));
installAccountsTestLifecycle();

// These two tests run on fake timers. Base UI's focus handling is timer-driven, so they live in a
// file of their own: what a fake-timer run leaves behind must not reach the other dialog tests.
describe("SignInDialog code view", () => {
  it("showingCode: the code, Copy code focused, the remaining time counting down in whole seconds", async () => {
    vi.useFakeTimers({
      toFake: ["setTimeout", "clearTimeout", "setInterval", "clearInterval", "Date"],
    });
    vi.setSystemTime(new Date(1_000_000_000_000));
    const backend = await setup();
    fireEvent.click(button("Use a code instead"));
    await act(async () => {
      await vi.advanceTimersByTimeAsync(0);
    });
    expect(backend.of("login_start")[0]).toMatchObject({ method: "deviceCode", language: "en" });
    act(() => {
      backend.started();
      backend.emit({
        event: "deviceCode",
        data: {
          userCode: "ABCD1234",
          verificationUri: "https://example.invalid/link",
          expiresAt: 1_000_000_000 + 872,
        },
      });
    });
    expect(within(dialog()).getByText("ABCD1234")).toBeTruthy();
    expect(within(dialog()).getByText("Open microsoft.com/link and enter this code.")).toBeTruthy();
    expect(within(dialog()).getByText("14:32 left")).toBeTruthy();
    // The countdown is plain text: no live region announces it every second.
    expect(within(dialog()).getByText("14:32 left").closest("[aria-live]")).toBeNull();
    expect(document.activeElement).toBe(button("Copy code"));
    act(() => {
      vi.advanceTimersByTime(3000);
    });
    expect(within(dialog()).getByText("14:29 left")).toBeTruthy();
    expect(footerLabels()).toEqual(["Cancel", "Open microsoft.com/link"]);
    expect(isPrimary(footerButtons()[1])).toBe(true);
    await closeWithRealTimers();
  });

  it("the copy button reads Copied for 2 seconds", async () => {
    vi.useFakeTimers({
      toFake: ["setTimeout", "clearTimeout", "setInterval", "clearInterval", "Date"],
    });
    const written: string[] = [];
    Object.defineProperty(navigator, "clipboard", {
      value: { writeText: (t: string) => (written.push(t), Promise.resolve()) },
      configurable: true,
    });
    const backend = await setup();
    fireEvent.click(button("Use a code instead"));
    await act(async () => {
      await vi.advanceTimersByTimeAsync(0);
    });
    act(() => {
      backend.started();
      backend.emit({
        event: "deviceCode",
        data: {
          userCode: "ABCD1234",
          verificationUri: "x",
          expiresAt: Math.floor(Date.now() / 1000) + 900,
        },
      });
    });
    await act(async () => {
      fireEvent.click(button("Copy code"));
      await vi.advanceTimersByTimeAsync(0);
    });
    expect(written).toEqual(["ABCD1234"]);
    expect(button("Copied")).toBeTruthy();
    act(() => {
      vi.advanceTimersByTime(1900);
    });
    expect(button("Copied")).toBeTruthy();
    act(() => {
      vi.advanceTimersByTime(200);
    });
    expect(button("Copy code")).toBeTruthy();
    await closeWithRealTimers();
  });
});
