import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { act, cleanup, fireEvent, render, screen, within } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { initI18n } from "@/i18n";
import type { ErrorDto } from "@/ipc/bindings/ErrorDto";
import { ErrorPanel } from "./ErrorPanel";

const base: ErrorDto = {
  code: "network",
  params: {},
  action: { kind: "retry" },
  detail: "request to https://x/y failed: connection refused",
};

beforeEach(() => {
  initI18n("en", "en-US");
  Element.prototype.scrollIntoView = vi.fn();
});
afterEach(() => {
  cleanup();
  clearMocks();
  vi.useRealTimers();
});

describe("ErrorPanel", () => {
  it("is an alert with the title and the body", () => {
    render(<ErrorPanel error={base} />);
    const alert = screen.getByRole("alert");
    expect(within(alert).getByText("Cannot reach Microsoft")).toBeTruthy();
    expect(within(alert).getByText("Check your internet connection and try again.")).toBeTruthy();
  });

  it("moves focus to the title when it appears", () => {
    render(<ErrorPanel error={base} />);
    expect(document.activeElement).toBe(screen.getByText("Cannot reach Microsoft"));
  });

  it("does not take focus when asked not to", () => {
    render(<ErrorPanel error={base} autoFocus={false} />);
    expect(document.activeElement).toBe(document.body);
  });

  it("shows retry only when onRetry is given and calls it once on double click", () => {
    const { rerender } = render(<ErrorPanel error={base} />);
    expect(screen.queryByRole("button", { name: "Try again" })).toBeNull();
    const onRetry = vi.fn();
    rerender(<ErrorPanel error={base} onRetry={onRetry} />);
    const button = screen.getByRole("button", { name: "Try again" });
    fireEvent.click(button);
    fireEvent.click(button);
    expect(onRetry).toHaveBeenCalledTimes(1);
  });

  it("allows retrying again once the first retry has finished", async () => {
    let finish = () => {};
    const onRetry = vi.fn(() => new Promise<void>((resolve) => (finish = resolve)));
    render(<ErrorPanel error={base} onRetry={onRetry} />);
    const button = screen.getByRole("button", { name: "Try again" });
    fireEvent.click(button);
    fireEvent.click(button);
    expect(onRetry).toHaveBeenCalledTimes(1);
    await act(async () => finish());
    fireEvent.click(button);
    expect(onRetry).toHaveBeenCalledTimes(2);
  });

  it("details are collapsed and contain the detail text", () => {
    render(<ErrorPanel error={base} />);
    const trigger = screen.getByRole("button", { name: "Details" });
    expect(trigger.getAttribute("aria-expanded")).toBe("false");
    expect(screen.queryByText(base.detail)).toBeNull();
    fireEvent.click(trigger);
    expect(trigger.getAttribute("aria-expanded")).toBe("true");
    expect(screen.getByText(base.detail)).toBeTruthy();
  });

  it("detail stays English in the German UI and has a copy button", async () => {
    initI18n("de", "de-AT");
    const writeText = vi.fn().mockResolvedValue(undefined);
    Object.defineProperty(navigator, "clipboard", { value: { writeText }, configurable: true });
    render(<ErrorPanel error={{ ...base, detail: "the operation was cancelled" }} />);
    expect(screen.getByText("Keine Verbindung zu Microsoft")).toBeTruthy();
    fireEvent.click(screen.getByRole("button", { name: "Details" }));
    expect(screen.getByText("the operation was cancelled")).toBeTruthy();
    await act(async () => fireEvent.click(screen.getByRole("button", { name: "Kopieren" })));
    expect(writeText).toHaveBeenCalledTimes(1);
    expect(writeText).toHaveBeenCalledWith("the operation was cancelled");
    expect(screen.getByRole("button", { name: "Kopiert" })).toBeTruthy();
  });

  it("a long detail without spaces sits in a bounded, scrollable, focusable box", () => {
    const detail = "x".repeat(300);
    render(<ErrorPanel error={{ ...base, detail }} />);
    fireEvent.click(screen.getByRole("button", { name: "Details" }));
    const box = screen.getByText(detail);
    expect(box.className).toContain("max-h-40");
    expect(box.className).toContain("overflow-auto");
    expect(box.className).toContain("[overflow-wrap:anywhere]");
    expect(box.getAttribute("tabindex")).toBe("0");
  });

  it("extraAction renders as an additional secondary button and calls its handler once", () => {
    const onClick = vi.fn();
    render(
      <ErrorPanel
        error={base}
        onRetry={() => {}}
        extraAction={{ label: "Open folder", onClick }}
      />,
    );
    const extra = screen.getByRole("button", { name: "Open folder" });
    expect(extra.getAttribute("data-slot")).toBe("button");
    fireEvent.click(extra);
    fireEvent.click(extra);
    expect(onClick).toHaveBeenCalledTimes(1);
    expect(screen.getByRole("button", { name: "Try again" })).toBeTruthy();
  });

  it("an openUrl action calls api.openUrl with the target and nothing else, not onAction", async () => {
    const calls: [string, unknown][] = [];
    mockIPC((cmd, args) => {
      calls.push([cmd, args]);
    });
    const onAction = vi.fn();
    const error: ErrorDto = {
      code: "noXboxAccount",
      params: {},
      action: { kind: "openUrl", target: "xboxSignup" },
      detail: "",
    };
    render(<ErrorPanel error={error} onAction={onAction} />);
    const button = screen.getByRole("button", { name: "Create Xbox profile" });
    await act(async () => {
      fireEvent.click(button);
      fireEvent.click(button);
    });
    expect(calls).toEqual([["open_url", { target: "xboxSignup" }]]);
    expect(onAction).not.toHaveBeenCalled();
  });

  it("says so inline when the link could not be opened", async () => {
    mockIPC(() => {
      throw { code: "openFailed", params: {}, action: null, detail: "no browser" };
    });
    const error: ErrorDto = {
      code: "noProfile",
      params: {},
      action: { kind: "openUrl", target: "minecraftProfile" },
      detail: "",
    };
    render(<ErrorPanel error={error} />);
    await act(async () => fireEvent.click(screen.getByRole("button", { name: "Create profile" })));
    expect(screen.getByText("The link could not be opened")).toBeTruthy();
  });

  it("other actions go to onAction, and without a handler they are not shown", () => {
    const error: ErrorDto = {
      ...base,
      code: "loginExpired",
      params: { name: "Mira_K" },
      action: { kind: "signIn" },
    };
    const onAction = vi.fn();
    const { rerender } = render(<ErrorPanel error={error} />);
    expect(screen.queryByRole("button", { name: "Sign in again" })).toBeNull();
    rerender(<ErrorPanel error={error} onAction={onAction} />);
    fireEvent.click(screen.getByRole("button", { name: "Sign in again" }));
    expect(onAction).toHaveBeenCalledWith({ kind: "signIn" });
    expect(screen.getByText("Mira_K needs to sign in again")).toBeTruthy();
  });
});
