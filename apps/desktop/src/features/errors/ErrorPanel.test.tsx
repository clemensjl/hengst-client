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
    render(<ErrorPanel error={base} context="signIn" />);
    const alert = screen.getByRole("alert");
    expect(within(alert).getByText("Cannot reach Microsoft")).toBeTruthy();
    expect(within(alert).getByText("Check your internet connection and try again.")).toBeTruthy();
  });

  it("does not take focus by default: a panel that appears on its own must not pull focus out of a field", () => {
    const { rerender } = render(
      <>
        <input aria-label="field" />
        {null}
      </>,
    );
    screen.getByLabelText("field").focus();
    rerender(
      <>
        <input aria-label="field" />
        <ErrorPanel error={base} />
      </>,
    );
    expect(document.activeElement).toBe(screen.getByLabelText("field"));
  });

  it("moves focus to the title when it appears and the caller opted in", () => {
    render(<ErrorPanel error={base} autoFocus />);
    expect(document.activeElement).toBe(screen.getByText("No connection"));
  });

  it("moves focus to the title again when the same panel receives a new error", () => {
    const { rerender } = render(
      <>
        <input aria-label="field" />
        <ErrorPanel error={base} autoFocus />
      </>,
    );
    screen.getByLabelText("field").focus();
    const next: ErrorDto = { ...base, code: "serverError", detail: "HTTP 503" };
    rerender(
      <>
        <input aria-label="field" />
        <ErrorPanel error={next} autoFocus />
      </>,
    );
    expect(document.activeElement).toBe(screen.getByText("The server did not respond as expected"));
  });

  it("does not steal focus again when the parent re-renders with an equal error", () => {
    const { rerender } = render(
      <>
        <input aria-label="field" />
        <ErrorPanel error={base} autoFocus />
      </>,
    );
    screen.getByLabelText("field").focus();
    rerender(
      <>
        <input aria-label="field" />
        <ErrorPanel error={{ ...base, params: { ...base.params } }} autoFocus />
      </>,
    );
    expect(document.activeElement).toBe(screen.getByLabelText("field"));
  });

  it("takes no focus when asked not to", () => {
    render(<ErrorPanel error={base} autoFocus={false} />);
    expect(document.activeElement).toBe(document.body);
  });

  it("uses the copy of its context", () => {
    render(<ErrorPanel error={{ ...base, code: "offline" }} context="launch" />);
    expect(screen.getByText("No connection")).toBeTruthy();
    expect(
      screen.getByText(
        "Files for this version are missing. Check your internet connection and try again.",
      ),
    ).toBeTruthy();
  });

  it("without a context a network error does not name a service", () => {
    render(<ErrorPanel error={base} />);
    expect(screen.getByText("No connection")).toBeTruthy();
    expect(screen.queryByText(/Microsoft/)).toBeNull();
  });

  it("warns in development when the error's action needs a handler that is missing", () => {
    const warn = vi.spyOn(console, "warn").mockImplementation(() => {});
    const error: ErrorDto = { ...base, action: { kind: "signIn" } };
    render(<ErrorPanel error={error} />);
    expect(warn).toHaveBeenCalledTimes(1);
    expect(String(warn.mock.calls[0][0])).toContain("onAction");
    warn.mockClear();
    render(<ErrorPanel error={error} onAction={() => {}} />);
    render(<ErrorPanel error={{ ...base, action: { kind: "retry" } }} />);
    expect(warn).not.toHaveBeenCalled();
    warn.mockRestore();
  });

  it("marks the detail as English text inside any UI language", () => {
    initI18n("de", "de-AT");
    render(<ErrorPanel error={base} />);
    fireEvent.click(screen.getByRole("button", { name: "Details" }));
    expect(screen.getByText(base.detail).getAttribute("lang")).toBe("en");
  });

  it("says so when copying fails and announces a successful copy politely", async () => {
    const writeText = vi.fn().mockRejectedValue(new Error("denied"));
    Object.defineProperty(navigator, "clipboard", { value: { writeText }, configurable: true });
    render(<ErrorPanel error={base} />);
    fireEvent.click(screen.getByRole("button", { name: "Details" }));
    await act(async () => fireEvent.click(screen.getByRole("button", { name: "Copy" })));
    expect(screen.getByText("Copying failed: select the text and press Ctrl+C.")).toBeTruthy();
    writeText.mockResolvedValue(undefined);
    await act(async () => fireEvent.click(screen.getByRole("button", { name: "Copy" })));
    const status = screen.getByRole("status");
    expect(status.textContent).toBe("Copied");
    expect(status.getAttribute("aria-live")).toBe("polite");
    expect(screen.queryByText(/Copying failed/)).toBeNull();
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
    expect(screen.getByText("Keine Verbindung")).toBeTruthy();
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
