import { describe, expect, it } from "vitest";
import i18n, { initI18n } from "@/i18n";
import type { AccountDto } from "@/ipc/bindings/AccountDto";
import type { ErrorCode } from "@/ipc/bindings/ErrorCode";
import type { ErrorDto } from "@/ipc/bindings/ErrorDto";
import type { LoginMethod } from "@/ipc/bindings/LoginMethod";
import { tableRows } from "@/styles/designDoc";
import de_json from "@/i18n/de.json";
import en_json from "@/i18n/en.json";
import {
  isActive,
  signedInMessage,
  signInErrorAction,
  signInReducer,
  type SignInInput,
  type SignInState,
} from "./signIn";

initI18n("en", "en-US");
const de = i18n.getFixedT("de");
const en = i18n.getFixedT("en");

const account = (id: string, name: string): AccountDto => ({
  id,
  name,
  kind: "microsoft",
  selected: true,
  signInRequired: false,
});
const A = account("a", "Mira_K");
const error = (code: ErrorCode): ErrorDto => ({ code, params: {}, action: null, detail: "d" });

const starting = (method: LoginMethod, op: number | null): SignInState => ({
  phase: "starting",
  method,
  op,
});
const waiting = (reopenFailed = false): SignInState => ({
  phase: "waitingBrowser",
  op: 5,
  reopenFailed,
});
const showing: SignInState = {
  phase: "showingCode",
  op: 5,
  userCode: "ABCD1234",
  verificationUri: "https://example.invalid/link",
  expiresAt: 1000,
};
const completing = (method: LoginMethod): SignInState => ({ phase: "completing", op: 5, method });

describe("signInReducer: every state against every event", () => {
  // The events, by name. Each state below lists only the cells where the state CHANGES; every
  // other cell must return the very same object (the event is ignored).
  const events: Record<string, SignInInput> = {
    "start browser": { event: "start", method: "browser" },
    "start deviceCode": { event: "start", method: "deviceCode" },
    switchToCode: { event: "switchToCode" },
    "started 5": { event: "started", data: { op: 5 } },
    "started 6": { event: "started", data: { op: 6 } },
    browserOpened: { event: "browserOpened" },
    deviceCode: {
      event: "deviceCode",
      data: {
        userCode: "ABCD1234",
        verificationUri: "https://example.invalid/link",
        expiresAt: 1000,
      },
    },
    completing: { event: "completing" },
    signedIn: { event: "signedIn", data: { account: A } },
    "failed network": { event: "failed", data: { error: error("network") } },
    "failed appNotApproved": { event: "failed", data: { error: error("appNotApproved") } },
    cancelled: { event: "cancelled" },
    reopenFailed: { event: "reopenFailed" },
    reopened: { event: "reopened" },
    reset: { event: "reset" },
  };

  const done: SignInState = { phase: "done", account: A };
  const cancelled: SignInState = { phase: "cancelled" };
  const notApproved: SignInState = { phase: "appNotApproved" };
  const failedBrowser: SignInState = {
    phase: "failed",
    error: error("network"),
    method: "browser",
  };
  const idle: SignInState = { phase: "idle" };

  /** Terminal events from a running state, for the method the state belongs to. */
  const ends = (method: LoginMethod): Record<string, SignInState> => ({
    signedIn: done,
    "failed network": { phase: "failed", error: error("network"), method },
    "failed appNotApproved": notApproved,
    cancelled,
    reset: idle,
  });

  const table: { name: string; state: SignInState; changes: Record<string, SignInState> }[] = [
    {
      name: "idle",
      state: idle,
      changes: {
        "start browser": starting("browser", null),
        "start deviceCode": starting("deviceCode", null),
      },
    },
    {
      name: "starting browser, op not known yet",
      state: starting("browser", null),
      changes: {
        "started 5": starting("browser", 5),
        "started 6": starting("browser", 6),
        ...ends("browser"),
      },
    },
    {
      name: "starting browser, op 5",
      state: starting("browser", 5),
      changes: {
        browserOpened: waiting(),
        completing: completing("browser"),
        ...ends("browser"),
      },
    },
    {
      name: "starting deviceCode, op 5",
      state: starting("deviceCode", 5),
      changes: {
        deviceCode: showing,
        completing: completing("deviceCode"),
        ...ends("deviceCode"),
      },
    },
    {
      name: "starting deviceCode, op not known yet",
      state: starting("deviceCode", null),
      changes: {
        "started 5": starting("deviceCode", 5),
        "started 6": starting("deviceCode", 6),
        ...ends("deviceCode"),
      },
    },
    {
      name: "waitingBrowser",
      state: waiting(),
      changes: {
        completing: completing("browser"),
        reopenFailed: waiting(true),
        switchToCode: starting("deviceCode", null),
        ...ends("browser"),
      },
    },
    {
      name: "waitingBrowser after a failed reopen",
      state: waiting(true),
      changes: {
        completing: completing("browser"),
        reopened: waiting(false),
        switchToCode: starting("deviceCode", null),
        ...ends("browser"),
      },
    },
    {
      name: "showingCode",
      state: showing,
      changes: { completing: completing("deviceCode"), ...ends("deviceCode") },
    },
    {
      name: "completing after the browser",
      state: completing("browser"),
      changes: ends("browser"),
    },
    {
      name: "completing after the code",
      state: completing("deviceCode"),
      changes: ends("deviceCode"),
    },
    { name: "done", state: done, changes: { reset: idle } },
    { name: "cancelled", state: cancelled, changes: { reset: idle } },
    { name: "appNotApproved", state: notApproved, changes: { reset: idle } },
    {
      name: "failed (browser)",
      state: failedBrowser,
      changes: {
        "start browser": starting("browser", null),
        "start deviceCode": starting("deviceCode", null),
        reset: idle,
      },
    },
  ];

  for (const { name, state, changes } of table) {
    describe(name, () => {
      for (const [eventName, input] of Object.entries(events)) {
        const expected = changes[eventName];
        it(`${eventName}: ${expected ? "-> " + expected.phase : "is ignored"}`, () => {
          const next = signInReducer(state, input);
          if (expected) expect(next).toEqual(expected);
          else expect(next, "an ignored event returns the same state").toBe(state);
        });
      }
    });
  }

  it("covers every state of the machine and every event", () => {
    const phases = new Set(table.map((t) => t.state.phase));
    expect([...phases].sort()).toEqual(
      [
        "idle",
        "starting",
        "waitingBrowser",
        "showingCode",
        "completing",
        "done",
        "failed",
        "appNotApproved",
        "cancelled",
      ].sort(),
    );
    expect(Object.keys(events)).toHaveLength(15);
  });
});

describe("signInReducer: the backend's guarantees", () => {
  const run = (inputs: SignInInput[], from: SignInState = { phase: "idle" }) =>
    inputs.reduce(signInReducer, from);

  it("started then browserOpened -> waitingBrowser with the op from started", () => {
    const s = run([
      { event: "start", method: "browser" },
      { event: "started", data: { op: 9 } },
      { event: "browserOpened" },
    ]);
    expect(s).toEqual({ phase: "waitingBrowser", op: 9, reopenFailed: false });
  });

  it("started arrives before login_start resolves and the op is still known", () => {
    // The reducer never sees the return value of login_start: the op comes from the channel.
    const s = run([
      { event: "start", method: "browser" },
      { event: "started", data: { op: 4 } },
    ]);
    expect(s).toMatchObject({ phase: "starting", op: 4 });
    expect(isActive(s)).toBe(true);
  });

  it("an event before started is not taken for a step (the op is unknown)", () => {
    const s = run([{ event: "start", method: "browser" }, { event: "browserOpened" }]);
    expect(s).toEqual({ phase: "starting", method: "browser", op: null });
  });

  it("a second started with another op does not replace the op of this sign-in", () => {
    const s = run([
      { event: "start", method: "browser" },
      { event: "started", data: { op: 4 } },
      { event: "started", data: { op: 5 } },
    ]);
    expect(s).toMatchObject({ phase: "starting", op: 4 });
  });

  it("deviceCode event -> showingCode with code and expiry", () => {
    const s = run([
      { event: "start", method: "deviceCode" },
      { event: "started", data: { op: 3 } },
      {
        event: "deviceCode",
        data: {
          userCode: "WXYZ9876",
          verificationUri: "https://example.invalid/x",
          expiresAt: 1234,
        },
      },
    ]);
    expect(s).toEqual({
      phase: "showingCode",
      op: 3,
      userCode: "WXYZ9876",
      verificationUri: "https://example.invalid/x",
      expiresAt: 1234,
    });
  });

  it("failed keeps the error and the method so retry restarts the same method", () => {
    for (const method of ["browser", "deviceCode"] as const) {
      const failed = run([
        { event: "start", method },
        { event: "started", data: { op: 1 } },
        { event: "failed", data: { error: error("network") } },
      ]);
      expect(failed).toEqual({ phase: "failed", error: error("network"), method });
      const retry = signInReducer(failed, { event: "start", method });
      expect(retry).toEqual({ phase: "starting", method, op: null });
    }
  });

  it("the method survives the phases between: a failure while completing names the method that ran", () => {
    const viaCode = run([
      { event: "start", method: "deviceCode" },
      { event: "started", data: { op: 1 } },
      {
        event: "deviceCode",
        data: { userCode: "C", verificationUri: "https://example.invalid", expiresAt: 1 },
      },
      { event: "completing" },
      { event: "failed", data: { error: error("gameNotOwned") } },
    ]);
    expect(viaCode).toMatchObject({ phase: "failed", method: "deviceCode" });
    const viaBrowser = run([
      { event: "start", method: "browser" },
      { event: "started", data: { op: 1 } },
      { event: "browserOpened" },
      { event: "completing" },
      { event: "failed", data: { error: error("gameNotOwned") } },
    ]);
    expect(viaBrowser).toMatchObject({ phase: "failed", method: "browser" });
  });

  it("appNotApproved error leads to the appNotApproved phase, not failed", () => {
    const s = run([
      { event: "start", method: "browser" },
      { event: "started", data: { op: 1 } },
      { event: "failed", data: { error: error("appNotApproved") } },
    ]);
    expect(s).toEqual({ phase: "appNotApproved" });
  });

  it("reopenFailed in waitingBrowser sets the inline error and keeps the wait", () => {
    const s = run([
      { event: "start", method: "browser" },
      { event: "started", data: { op: 1 } },
      { event: "browserOpened" },
      { event: "reopenFailed" },
    ]);
    expect(s).toEqual({ phase: "waitingBrowser", op: 1, reopenFailed: true });
    expect(isActive(s)).toBe(true);
  });

  it("a late event after cancelled is ignored", () => {
    const s = run([
      { event: "start", method: "browser" },
      { event: "started", data: { op: 1 } },
      { event: "browserOpened" },
      { event: "cancelled" },
      { event: "signedIn", data: { account: A } },
      { event: "failed", data: { error: error("network") } },
      { event: "completing" },
    ]);
    expect(s).toEqual({ phase: "cancelled" });
  });

  it("exactly one terminal event counts: whatever follows the first one is ignored", () => {
    const first = run([
      { event: "start", method: "browser" },
      { event: "started", data: { op: 1 } },
      { event: "signedIn", data: { account: A } },
      { event: "failed", data: { error: error("network") } },
      { event: "cancelled" },
    ]);
    expect(first).toEqual({ phase: "done", account: A });
  });

  it("a cancel that came back false changes nothing: the sign-in goes on to its terminal event", () => {
    // loginCancel resolved false because the account was being saved. The dialog sends no event
    // for that; the state stays `completing` and the terminal event decides.
    const saving = run([
      { event: "start", method: "browser" },
      { event: "started", data: { op: 1 } },
      { event: "browserOpened" },
      { event: "completing" },
    ]);
    expect(saving).toMatchObject({ phase: "completing" });
    expect(signInReducer(saving, { event: "signedIn", data: { account: A } })).toEqual({
      phase: "done",
      account: A,
    });
    expect(
      signInReducer(saving, { event: "failed", data: { error: error("credentialStore") } }),
    ).toMatchObject({ phase: "failed", method: "browser" });
  });

  it("openFailed is a failure of the browser method; its action is the code", () => {
    const s = run([
      { event: "start", method: "browser" },
      { event: "started", data: { op: 1 } },
      { event: "failed", data: { error: error("openFailed") } },
    ]);
    expect(s).toMatchObject({ phase: "failed", method: "browser" });
    expect(signInErrorAction("openFailed")).toEqual({ kind: "useCode" });
  });

  it("isActive is true exactly while a sign-in runs", () => {
    expect(isActive({ phase: "idle" })).toBe(false);
    expect(isActive(starting("browser", null))).toBe(true);
    expect(isActive(waiting())).toBe(true);
    expect(isActive(showing)).toBe(true);
    expect(isActive(completing("browser"))).toBe(true);
    expect(isActive({ phase: "done", account: A })).toBe(false);
    expect(isActive({ phase: "failed", error: error("network"), method: "browser" })).toBe(false);
    expect(isActive({ phase: "appNotApproved" })).toBe(false);
    expect(isActive({ phase: "cancelled" })).toBe(false);
  });
});

describe("signInErrorAction", () => {
  it("follows the action column of DESIGN.md 11.5", () => {
    expect(signInErrorAction("loopbackUnavailable")).toEqual({ kind: "useCode" });
    expect(signInErrorAction("noXboxAccount")).toEqual({
      kind: "openUrl",
      target: "xboxSignup",
      labelKey: "errors.action.openUrl.xboxSignup",
    });
    expect(signInErrorAction("noProfile")).toEqual({
      kind: "openUrl",
      target: "minecraftProfile",
      labelKey: "errors.action.openUrl.minecraftProfile",
    });
    expect(signInErrorAction("appMisconfigured")).toEqual({ kind: "none" });
    expect(signInErrorAction("missingClientId")).toEqual({ kind: "none" });
    for (const code of [
      "network",
      "offline",
      "loginDeclined",
      "browserLoginTimedOut",
      "deviceCodeExpired",
      "browserLoginFailed",
      "gameNotOwned",
      "credentialStore",
    ] as const) {
      expect(signInErrorAction(code), code).toEqual({ kind: "retry" });
    }
  });

  it("a code the table does not list can be retried", () => {
    expect(signInErrorAction("serverError")).toEqual({ kind: "retry" });
    expect(signInErrorAction("internal")).toEqual({ kind: "retry" });
  });

  it("the action column of the table is what the function returns", () => {
    const rows = tableRows(
      "Error copy in the failed phase (title without full stop, body, action):",
    );
    let checked = 0;
    for (const [codeCell, , , actionCell] of rows) {
      const codes = [...codeCell.matchAll(/`([a-zA-Z]+)`/g)].map((m) => m[1]) as ErrorCode[];
      for (const code of codes) {
        const action = signInErrorAction(code).kind;
        if (/^Try again/.test(actionCell)) expect(action, code).toBe("retry");
        else if (/^"Code stattdessen/.test(actionCell)) expect(action, code).toBe("useCode");
        else if (/^"(Xbox-Profil|Profil) anlegen"/.test(actionCell)) {
          expect(action, code).toBe("openUrl");
        } else if (/^none/.test(actionCell)) expect(action, code).toBe("none");
        else continue;
        checked++;
      }
    }
    expect(checked).toBeGreaterThanOrEqual(13);
  });
});

describe("signedInMessage", () => {
  it("names both accounts when another account signed in again", () => {
    expect(signedInMessage(account("b", "SecondPlayer"), { id: "a", name: "E2EPlayer" })).toEqual({
      key: "accounts.signedInOther",
      params: { new: "SecondPlayer", old: "E2EPlayer" },
    });
  });

  it("is the plain message for the same id, or when nobody was re-signed", () => {
    expect(signedInMessage(account("a", "Mira_K"), { id: "a", name: "Mira_K" })).toEqual({
      key: "accounts.signedIn",
      params: { name: "Mira_K" },
    });
    expect(signedInMessage(account("a", "Mira_K"), null)).toEqual({
      key: "accounts.signedIn",
      params: { name: "Mira_K" },
    });
  });

  it("reads as DESIGN.md 11.5 says, in both languages", () => {
    const p = { new: "SecondPlayer", old: "E2EPlayer" };
    expect(en("accounts.signedInOther", p)).toBe(
      "Signed in as SecondPlayer. E2EPlayer is still signed out.",
    );
    expect(de("accounts.signedInOther", p)).toBe(
      "Angemeldet als SecondPlayer. E2EPlayer ist weiterhin abgemeldet.",
    );
    expect(en("accounts.signedIn", { name: "Mira_K" })).toBe("Signed in as Mira_K");
    expect(de("accounts.signedIn", { name: "Mira_K" })).toBe("Angemeldet als Mira_K");
  });
});

describe("the sign-in copy", () => {
  /** The table "Phase | Body | Footer" of DESIGN.md 11.5 is checked word for word below; this one is the error table. */
  const pair = (cell: string) => cell.match(/^"(.+)" \/ "(.+)"$/)?.slice(1);

  it("openFailed has its own sign-in text: the browser could not be opened, use a code", () => {
    expect(en_json["errors.openFailed.signIn.title"]).toBe("The browser could not be opened");
    expect(en_json["errors.openFailed.signIn.body"]).toBe("Use a code instead.");
    expect(de_json["errors.openFailed.signIn.title"]).toBe("Der Browser ließ sich nicht öffnen");
    expect(de_json["errors.openFailed.signIn.body"]).toBe("Verwende stattdessen den Code.");
  });

  it("the DESIGN.md 11.5 error table has the openFailed row with the same words", () => {
    const rows = tableRows(
      "Error copy in the failed phase (title without full stop, body, action):",
    );
    const row = rows.find((r) => r[0].includes("`openFailed`"));
    expect(row, "a row for openFailed").toBeDefined();
    expect(pair(row![1])).toEqual([
      de_json["errors.openFailed.signIn.title"],
      en_json["errors.openFailed.signIn.title"],
    ]);
    expect(pair(row![2])).toEqual([
      de_json["errors.openFailed.signIn.body"],
      en_json["errors.openFailed.signIn.body"],
    ]);
  });

  it("every sign-in code of the error table has its action label in both languages", () => {
    for (const t of [de, en]) {
      expect(t("signIn.useCode")).not.toBe("signIn.useCode");
    }
  });
});
