import { LogOutIcon, RefreshCwIcon, UserPlusIcon } from "lucide-react";
import { useRef } from "react";
import { useTranslation } from "react-i18next";
import { Badge } from "@/components/Badge";
import {
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuRadioGroup,
  DropdownMenuRadioItem,
  DropdownMenuSeparator,
  DropdownMenuSub,
  DropdownMenuSubContent,
  DropdownMenuSubTrigger,
} from "@/components/ui/dropdown-menu";
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip";
import { useTruncated } from "@/lib/useTruncated";
import type { AccountDto } from "@/ipc/bindings/AccountDto";
import type { ErrorDto } from "@/ipc/bindings/ErrorDto";
import { useApp } from "@/stores/app";
import { describeError } from "@/features/errors/errorCopy";
import { toast } from "sonner";

/** One account in the radio group: check slot, the name (the only part that shrinks), the badges. */
function AccountItem({ account }: { account: AccountDto }) {
  const { t } = useTranslation();
  const nameRef = useRef<HTMLSpanElement>(null);
  const truncated = useTruncated(nameRef, account.name);
  const openSignIn = useApp((s) => s.openSignIn);
  const selectAccount = useApp((s) => s.selectAccount);

  const choose = () => {
    // An account that needs to sign in again starts that sign-in; any other one is selected.
    if (account.signInRequired) {
      openSignIn({ id: account.id, name: account.name });
      return;
    }
    if (account.selected) return;
    void selectAccount(account.id).then((failure) => {
      if (failure) toast(describeError(failure, t).title);
    });
  };

  // The tooltip shows the full name of a truncated one, and opens on keyboard focus as well as on
  // hover: the highlighted item holds the focus.
  return (
    <Tooltip disabled={!truncated}>
      <TooltipTrigger
        render={<DropdownMenuRadioItem value={account.id} onClick={choose} label={account.name} />}
      >
        <span ref={nameRef} className="min-w-0 flex-1 truncate">
          {account.name}
        </span>
        {account.signInRequired && <Badge className="shrink-0">{t("accounts.needsSignIn")}</Badge>}
        {account.kind === "offlineDev" && (
          <Badge className="shrink-0">{t("common.devBuild")}</Badge>
        )}
      </TooltipTrigger>
      <TooltipContent side="left">{account.name}</TooltipContent>
    </Tooltip>
  );
}

/** One account in the sign-out submenu. */
function SignOutItem({ account }: { account: AccountDto }) {
  const nameRef = useRef<HTMLSpanElement>(null);
  const truncated = useTruncated(nameRef, account.name);
  const openRemove = useApp((s) => s.openRemoveAccount);
  return (
    <Tooltip disabled={!truncated}>
      <TooltipTrigger
        render={<DropdownMenuItem onClick={() => openRemove(account)} label={account.name} />}
      >
        <span ref={nameRef} className="min-w-0 flex-1 truncate">
          {account.name}
        </span>
      </TooltipTrigger>
      <TooltipContent side="left">{account.name}</TooltipContent>
    </Tooltip>
  );
}

/** DESIGN.md 10.3: the accounts as a radio group, "Add account", and the sign-out submenu. */
export function AccountMenuContent({ accounts }: { accounts: AccountDto[] }) {
  const { t } = useTranslation();
  const openSignIn = useApp((s) => s.openSignIn);
  const selected = accounts.find((a) => a.selected)?.id ?? "";
  // The development account has nothing to sign out of.
  const signOutable = accounts.filter((a) => a.kind === "microsoft");
  return (
    <DropdownMenuContent align="end" aria-label={t("accounts.menu")}>
      <DropdownMenuRadioGroup value={selected}>
        {accounts.map((a) => (
          <AccountItem key={a.id} account={a} />
        ))}
      </DropdownMenuRadioGroup>
      <DropdownMenuSeparator />
      {/* Stays enabled without a client id: the store then shows the no-client-id dialog. */}
      <DropdownMenuItem onClick={() => openSignIn(null)}>
        <UserPlusIcon aria-hidden="true" />
        {t("accounts.add")}
      </DropdownMenuItem>
      {signOutable.length > 0 && (
        <DropdownMenuSub>
          <DropdownMenuSubTrigger>
            <LogOutIcon aria-hidden="true" />
            {t("accounts.signOut")}
          </DropdownMenuSubTrigger>
          <DropdownMenuSubContent>
            {signOutable.map((a) => (
              <SignOutItem key={a.id} account={a} />
            ))}
          </DropdownMenuSubContent>
        </DropdownMenuSub>
      )}
    </DropdownMenuContent>
  );
}

/** The menu of the "Accounts unavailable" button: what went wrong and a way to try again. */
export function AccountErrorMenuContent({ error }: { error: ErrorDto }) {
  const { t } = useTranslation();
  const reload = useApp((s) => s.loadAccounts);
  const { title, body } = describeError(error, t);
  return (
    <DropdownMenuContent align="end" aria-label={t("accounts.menu")}>
      <div className="flex flex-col px-3 py-2">
        <p className="text-label">{title}</p>
        <p className="text-small text-muted-foreground">{body}</p>
      </div>
      <DropdownMenuSeparator />
      <DropdownMenuItem onClick={() => void reload()}>
        <RefreshCwIcon aria-hidden="true" />
        {t("common.retry")}
      </DropdownMenuItem>
    </DropdownMenuContent>
  );
}
