// Test helper: a fake backend for the accounts commands, on top of `mockIPC`. It records every
// call, plays the channel of `login_start` (the events a test sends arrive as the backend's would)
// and lets a test decide what `login_cancel` and `login_reopen_browser` answer.
import { mockIPC } from "@tauri-apps/api/mocks";
import type { AccountDto } from "@/ipc/bindings/AccountDto";
import type { AccountsDto } from "@/ipc/bindings/AccountsDto";
import type { ErrorDto } from "@/ipc/bindings/ErrorDto";
import type { LoginEvent } from "@/ipc/bindings/LoginEvent";

export interface Call {
  cmd: string;
  args: Record<string, unknown>;
}

interface Channel {
  onmessage?: (e: LoginEvent) => void;
}

export const account = (id: string, name: string, extra: Partial<AccountDto> = {}): AccountDto => ({
  id,
  name,
  kind: "microsoft",
  selected: false,
  signInRequired: false,
  ...extra,
});

export function fakeBackend(initial: AccountsDto = { accounts: [], canSignIn: true }) {
  const calls: Call[] = [];
  const channels: Channel[] = [];
  let nextOp = 1;
  const b = {
    calls,
    accounts: initial,
    /** What `login_cancel` answers. */
    cancelAnswer: true,
    /** `login_reopen_browser` fails with this. */
    reopenFails: null as ErrorDto | null,
    /** `login_start` fails with this. */
    startFails: null as ErrorDto | null,
    /** `accounts_list` fails with this. */
    listFails: null as ErrorDto | null,
    /** `login_cancel` rejects with this. */
    cancelFails: null as ErrorDto | null,
    /** `account_select` fails with this. */
    selectFails: null as ErrorDto | null,
    /** `account_remove` fails with this. */
    removeFails: null as ErrorDto | null,
    count: (cmd: string) => calls.filter((c) => c.cmd === cmd).length,
    of: (cmd: string) => calls.filter((c) => c.cmd === cmd).map((c) => c.args),
    /** The channel of the n-th `login_start` (0 is the first). */
    channel: (n = channels.length - 1) => channels[n],
    /** Sends an event on the channel of the n-th sign-in. */
    emit: (event: LoginEvent, n = channels.length - 1) => channels[n].onmessage?.(event),
    /** `started` with the op that `login_start` handed out for that channel. */
    started: (n = channels.length - 1) => b.emit({ event: "started", data: { op: n + 1 } }, n),
  };
  mockIPC((cmd, payload) => {
    const args = (payload ?? {}) as Record<string, unknown>;
    calls.push({ cmd, args });
    switch (cmd) {
      case "accounts_list":
        if (b.listFails) throw b.listFails;
        return b.accounts;
      case "login_start":
        if (b.startFails) throw b.startFails;
        channels.push(args.channel as Channel);
        return nextOp++;
      case "login_cancel":
        if (b.cancelFails) throw b.cancelFails;
        return b.cancelAnswer;
      case "account_select":
        if (b.selectFails) throw b.selectFails;
        return;
      case "login_reopen_browser":
        if (b.reopenFails) throw b.reopenFails;
        return;
      case "account_remove":
        if (b.removeFails) throw b.removeFails;
        return;
      default:
        return;
    }
  });
  return b;
}
