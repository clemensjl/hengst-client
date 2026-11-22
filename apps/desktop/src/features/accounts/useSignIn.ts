import { useCallback, useEffect, useReducer, useRef } from "react";
import { api, IpcError } from "@/ipc/client";
import type { ErrorDto } from "@/ipc/bindings/ErrorDto";
import type { Language } from "@/ipc/bindings/Language";
import type { LoginEvent } from "@/ipc/bindings/LoginEvent";
import type { LoginMethod } from "@/ipc/bindings/LoginMethod";
import { isActive, signInReducer, type SignInState } from "./signIn";

const errorOf = (e: unknown): ErrorDto =>
  e instanceof IpcError ? e.dto : { code: "internal", params: {}, action: null, detail: String(e) };

export interface SignIn {
  state: SignInState;
  /** Starts a sign-in from `idle` or `failed`. A second call while one runs does nothing. */
  start: (method: LoginMethod) => void;
  /**
   * Cancels what runs. `true`: nothing is left to wait for, the dialog may close. `false`: it
   * cannot be cancelled now (the account is being saved, or the sign-in just ended); the dialog
   * stays and the terminal event decides. Never assumes `cancelled`.
   */
  cancel: () => Promise<boolean>;
  /** "Use a code instead" while the browser is awaited. */
  switchToCode: () => void;
  /** "Open browser again". A failure shows inline and the wait goes on. */
  reopen: () => void;
}

/**
 * Drives one sign-in dialog: the reducer of `signIn.ts` fed by one channel per attempt.
 *
 * An attempt that was replaced (retry, switch to the code, the dialog left) is ignored from then
 * on: its late events must not touch the state of the one that follows. Within an attempt the
 * reducer ignores what no longer fits (events after the terminal one, another operation id).
 */
export function useSignIn(language: Language): SignIn {
  const [state, dispatch] = useReducer(signInReducer, { phase: "idle" } as SignInState);
  const stateRef = useRef(state);
  stateRef.current = state;

  const attempt = useRef(0);
  /** The op of the current attempt: from its `started` event, else from `loginStart`'s answer. */
  const op = useRef<number | null>(null);
  /** From the first click until the sign-in has ended: a double click starts nothing twice. */
  const busy = useRef(false);
  /** The browser sign-in is being cancelled to start the code: its `cancelled` is expected. */
  const switching = useRef(false);
  const reopening = useRef(false);
  /** `cancel` succeeded: nothing is left running that the dialog started. */
  const cancelled = useRef(false);

  // A failure, or any other end, makes the next start possible.
  useEffect(() => {
    if (!isActive(state)) busy.current = false;
  }, [state]);

  const launch = useCallback(
    (method: LoginMethod) => {
      const mine = ++attempt.current;
      op.current = null;
      switching.current = false;
      cancelled.current = false;
      const live = () => attempt.current === mine;
      api
        .loginStart(method, language, (event: LoginEvent) => {
          if (!live()) return;
          // The cancel of a switch to the code ends the old sign-in with `cancelled`: expected.
          if (switching.current && event.event === "cancelled") return;
          if (event.event === "started") op.current = event.data.op;
          dispatch(event);
        })
        .then((started) => {
          if (live()) op.current ??= started;
          // The attempt was left before its id was known: cancel it now, nobody watches it.
          else void api.loginCancel(started).catch(() => {});
        })
        .catch((e: unknown) => {
          if (live()) dispatch({ event: "failed", data: { error: errorOf(e) } });
        });
    },
    [language],
  );

  const start = useCallback(
    (method: LoginMethod) => {
      // Before the first await and not a prop of the next render: the second click of a double
      // click arrives before the button has re-rendered.
      if (busy.current) return;
      const s = stateRef.current;
      if (s.phase !== "idle" && s.phase !== "failed") return;
      busy.current = true;
      dispatch({ event: "start", method });
      launch(method);
    },
    [launch],
  );

  const currentOp = (): number | null => {
    const s = stateRef.current;
    return op.current ?? ("op" in s ? s.op : null);
  };

  const cancel = useCallback(async (): Promise<boolean> => {
    const s = stateRef.current;
    if (!isActive(s)) return true;
    // Nothing to cancel while the account is being saved or an earlier cancel is under way.
    if (s.phase === "completing") return false;
    const id = currentOp();
    if (id === null) {
      // Not known yet: leave this attempt; `loginStart`'s answer cancels it.
      attempt.current++;
      cancelled.current = true;
      return true;
    }
    try {
      cancelled.current = await api.loginCancel(id);
      return cancelled.current;
    } catch {
      return false;
    }
  }, []);

  const switchToCode = useCallback(() => {
    const s = stateRef.current;
    if (s.phase !== "waitingBrowser" || switching.current) return;
    switching.current = true;
    void api
      .loginCancel(s.op)
      .then((cancelled) => {
        if (!cancelled) {
          // Saving or already over: no new sign-in; the events of this one go on.
          switching.current = false;
          return;
        }
        dispatch({ event: "switchToCode" });
        launch("deviceCode");
      })
      .catch(() => {
        switching.current = false;
      });
  }, [launch]);

  const reopen = useCallback(() => {
    const s = stateRef.current;
    if (s.phase !== "waitingBrowser" || reopening.current) return;
    reopening.current = true;
    api
      .loginReopenBrowser(s.op)
      .then(() => dispatch({ event: "reopened" }))
      .catch(() => dispatch({ event: "reopenFailed" }))
      .finally(() => {
        reopening.current = false;
      });
  }, []);

  // The dialog went away: nothing it started may speak to it any more, and a sign-in nobody
  // watches is cancelled (the close paths cancel first; this is the net under them).
  useEffect(
    () => () => {
      attempt.current++;
      const left = stateRef.current;
      const id = op.current ?? ("op" in left ? left.op : null);
      if (!cancelled.current && isActive(left) && left.phase !== "completing" && id !== null) {
        void api.loginCancel(id).catch(() => {});
      }
    },
    [],
  );

  return { state, start, cancel, switchToCode, reopen };
}
