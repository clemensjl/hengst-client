import type { AccountDto } from "@/ipc/bindings/AccountDto";
import type { ErrorCode } from "@/ipc/bindings/ErrorCode";
import type { ErrorDto } from "@/ipc/bindings/ErrorDto";
import type { LoginEvent } from "@/ipc/bindings/LoginEvent";
import type { LoginMethod } from "@/ipc/bindings/LoginMethod";
import type { UrlTarget } from "@/ipc/bindings/UrlTarget";

/**
 * The state of the sign-in dialog (DESIGN.md 11.5), as one pure function so every (state, event)
 * pair can be tested.
 *
 * What the backend guarantees (accounts.rs): `started` is the first event of a sign-in's channel
 * and carries the operation id; then progress; then exactly one of `signedIn`, `failed` and
 * `cancelled`, and nothing after it. The reducer relies on no more than that, and ignores what does
 * not fit: an event the state cannot take is returned as the same object.
 *
 * Differences from the plan's sketch: `starting` also holds the op (null until `started` arrives),
 * and `completing` holds the method, so a failure while completing still knows which method to
 * retry; `completing` is the one phase that follows both methods.
 */
export type SignInState =
  | { phase: "idle" }
  | { phase: "starting"; method: LoginMethod; op: number | null }
  | { phase: "waitingBrowser"; op: number; reopenFailed: boolean }
  | {
      phase: "showingCode";
      op: number;
      userCode: string;
      verificationUri: string;
      /** Unix seconds. */
      expiresAt: number;
    }
  | { phase: "completing"; op: number; method: LoginMethod }
  | { phase: "done"; account: AccountDto }
  | { phase: "failed"; error: ErrorDto; method: LoginMethod }
  | { phase: "appNotApproved" }
  | { phase: "cancelled" };

/** What the dialog itself feeds in, besides the events of the backend. */
export type SignInInput =
  | LoginEvent
  | { event: "start"; method: LoginMethod }
  /** The user leaves the browser wait for the code; the dialog has cancelled the browser sign-in. */
  | { event: "switchToCode" }
  | { event: "reopenFailed" }
  | { event: "reopened" }
  /** The dialog opens again. */
  | { event: "reset" };

/** A sign-in is running: the dialog has something to cancel. */
export type ActiveState = Extract<
  SignInState,
  { phase: "starting" | "waitingBrowser" | "showingCode" | "completing" }
>;

export function isActive(state: SignInState): state is ActiveState {
  return (
    state.phase === "starting" ||
    state.phase === "waitingBrowser" ||
    state.phase === "showingCode" ||
    state.phase === "completing"
  );
}

/** The method a running state belongs to. */
function methodOf(state: SignInState): LoginMethod {
  switch (state.phase) {
    case "starting":
    case "completing":
      return state.method;
    case "showingCode":
      return "deviceCode";
    default:
      return "browser";
  }
}

export function signInReducer(state: SignInState, input: SignInInput): SignInState {
  if (input.event === "reset") return state.phase === "idle" ? state : { phase: "idle" };

  if (input.event === "start") {
    return state.phase === "idle" || state.phase === "failed"
      ? { phase: "starting", method: input.method, op: null }
      : state;
  }

  // Everything else concerns a running sign-in; the finished phases take nothing more.
  if (!isActive(state)) return state;

  switch (input.event) {
    case "started":
      // The op of this sign-in is the first one it sees; another one is some other operation.
      return state.phase === "starting" && state.op === null
        ? { ...state, op: input.data.op }
        : state;
    case "browserOpened":
      return state.phase === "starting" && state.method === "browser" && state.op !== null
        ? { phase: "waitingBrowser", op: state.op, reopenFailed: false }
        : state;
    case "deviceCode":
      return state.phase === "starting" && state.method === "deviceCode" && state.op !== null
        ? { phase: "showingCode", op: state.op, ...input.data }
        : state;
    case "completing": {
      if (state.phase === "completing") return state;
      const op = state.op;
      return op === null ? state : { phase: "completing", op, method: methodOf(state) };
    }
    case "signedIn":
      return { phase: "done", account: input.data.account };
    case "failed":
      return input.data.error.code === "appNotApproved"
        ? { phase: "appNotApproved" }
        : { phase: "failed", error: input.data.error, method: methodOf(state) };
    case "cancelled":
      return { phase: "cancelled" };
    case "reopenFailed":
      return state.phase === "waitingBrowser" && !state.reopenFailed
        ? { ...state, reopenFailed: true }
        : state;
    case "reopened":
      return state.phase === "waitingBrowser" && state.reopenFailed
        ? { ...state, reopenFailed: false }
        : state;
    case "switchToCode":
      return state.phase === "waitingBrowser"
        ? { phase: "starting", method: "deviceCode", op: null }
        : state;
  }
}

/** What a failed sign-in offers besides the panel's own "Try again" (DESIGN.md 11.5, column Action). */
export type SignInErrorAction =
  /** Start the same method again. */
  | { kind: "retry" }
  /** The browser cannot be used: start the code instead. */
  | { kind: "useCode" }
  | { kind: "openUrl"; target: UrlTarget; labelKey: string }
  /** Nothing to do but read the details. */
  | { kind: "none" };

export function signInErrorAction(code: ErrorCode): SignInErrorAction {
  switch (code) {
    case "loopbackUnavailable":
    // The system could not open the browser at all: the code needs none (backend `openFailed`).
    case "openFailed":
      return { kind: "useCode" };
    case "noXboxAccount":
      return {
        kind: "openUrl",
        target: "xboxSignup",
        labelKey: "errors.action.openUrl.xboxSignup",
      };
    case "noProfile":
      return {
        kind: "openUrl",
        target: "minecraftProfile",
        labelKey: "errors.action.openUrl.minecraftProfile",
      };
    case "appMisconfigured":
    case "missingClientId":
      return { kind: "none" };
    default:
      return { kind: "retry" };
  }
}

/**
 * The toast after a sign-in. Signing in again as another Microsoft account than the one asked for
 * adds and selects that account; the one asked for stays signed out (DESIGN.md 11.5).
 */
export function signedInMessage(
  account: AccountDto,
  resign: { id: string; name: string } | null,
): { key: string; params: Record<string, string> } {
  if (resign && resign.id !== account.id) {
    return {
      key: "accounts.signedInOther",
      params: { new: account.name, old: resign.name },
    };
  }
  return { key: "accounts.signedIn", params: { name: account.name } };
}
