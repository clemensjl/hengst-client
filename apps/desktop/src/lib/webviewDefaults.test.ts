import { afterEach, describe, expect, it } from "vitest";
import {
  allowsNativeContextMenu,
  blocksBrowserShortcut,
  installWebviewDefaults,
} from "./webviewDefaults";

const el = (html: string) => {
  const host = document.createElement("div");
  host.innerHTML = html;
  document.body.append(host);
  return host.firstElementChild as HTMLElement;
};

afterEach(() => {
  document.body.innerHTML = "";
});

describe("webview defaults", () => {
  it("context menu is allowed only in text inputs and the log area", () => {
    expect(allowsNativeContextMenu(el('<input type="text">'))).toBe(true);
    expect(allowsNativeContextMenu(el("<input>"))).toBe(true);
    expect(allowsNativeContextMenu(el('<input type="search">'))).toBe(true);
    expect(allowsNativeContextMenu(el("<textarea></textarea>"))).toBe(true);
    const log = el('<div data-native-menu><span id="line">x</span></div>');
    expect(allowsNativeContextMenu(log.querySelector("#line"))).toBe(true);
    expect(allowsNativeContextMenu(el("<div>a</div>"))).toBe(false);
    expect(allowsNativeContextMenu(el("<button>a</button>"))).toBe(false);
    expect(allowsNativeContextMenu(el("<a href='#'>a</a>"))).toBe(false);
    expect(allowsNativeContextMenu(el('<input type="checkbox">'))).toBe(false);
    expect(allowsNativeContextMenu(null)).toBe(false);
  });

  it("blocks reload, print, developer tools and find, and keeps zoom", () => {
    const key = (init: KeyboardEventInit) =>
      blocksBrowserShortcut(new KeyboardEvent("keydown", init));
    expect(key({ key: "F5" })).toBe(true);
    expect(key({ key: "F5", ctrlKey: true })).toBe(true);
    expect(key({ key: "r", ctrlKey: true })).toBe(true);
    expect(key({ key: "R", ctrlKey: true, shiftKey: true })).toBe(true);
    expect(key({ key: "p", ctrlKey: true })).toBe(true);
    expect(key({ key: "F12" })).toBe(true);
    expect(key({ key: "f", ctrlKey: true })).toBe(true);
    expect(key({ key: "F3" })).toBe(true);
    expect(key({ key: "+", ctrlKey: true })).toBe(false);
    expect(key({ key: "-", ctrlKey: true })).toBe(false);
    expect(key({ key: "0", ctrlKey: true })).toBe(false);
    expect(key({ key: "r" })).toBe(false);
    expect(key({ key: "c", ctrlKey: true })).toBe(false);
  });

  it("the installed handlers cancel a blocked key and a context menu outside inputs", () => {
    const stop = installWebviewDefaults(true);
    const f5 = new KeyboardEvent("keydown", { key: "F5", cancelable: true, bubbles: true });
    document.dispatchEvent(f5);
    expect(f5.defaultPrevented).toBe(true);
    const menu = new MouseEvent("contextmenu", { cancelable: true, bubbles: true });
    el("<div>a</div>").dispatchEvent(menu);
    expect(menu.defaultPrevented).toBe(true);
    const inInput = new MouseEvent("contextmenu", { cancelable: true, bubbles: true });
    el("<input>").dispatchEvent(inInput);
    expect(inInput.defaultPrevented).toBe(false);
    stop();
  });

  it("installs nothing when disabled (debug builds keep the menu and the keys)", () => {
    const stop = installWebviewDefaults(false);
    const f5 = new KeyboardEvent("keydown", { key: "F5", cancelable: true, bubbles: true });
    document.dispatchEvent(f5);
    expect(f5.defaultPrevented).toBe(false);
    stop();
  });
});
