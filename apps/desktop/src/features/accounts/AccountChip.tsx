import { ChevronDownIcon, LogInIcon, TriangleAlertIcon } from "lucide-react";
import { useRef } from "react";
import { useTranslation } from "react-i18next";
import { Badge } from "@/components/Badge";
import { Delayed } from "@/components/Delayed";
import { Button } from "@/components/ui/button";
import { DropdownMenu, DropdownMenuTrigger } from "@/components/ui/dropdown-menu";
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip";
import { useTruncated } from "@/lib/useTruncated";
import type { AccountDto } from "@/ipc/bindings/AccountDto";
import { useApp } from "@/stores/app";
import { AccountErrorMenuContent, AccountMenuContent } from "./AccountMenu";

/**
 * The chip (or the sign-in button, or the unavailable button) the dialogs of the accounts feature
 * return focus to when they close.
 */
export const chipRef: { current: HTMLElement | null } = { current: null };
const setChip = (el: HTMLElement | null) => {
  chipRef.current = el;
};

/** The account the chip shows: the selected one, else the first. */
function shownAccount(accounts: AccountDto[]): AccountDto {
  return accounts.find((a) => a.selected) ?? accounts[0];
}

/** The first letter of the name as a whole character (a name may start with a surrogate pair). */
const initialOf = (name: string) => (Array.from(name)[0] ?? "?").toUpperCase();

function AccountChipButton({ accounts }: { accounts: AccountDto[] }) {
  const { t } = useTranslation();
  const account = shownAccount(accounts);
  const nameRef = useRef<HTMLSpanElement>(null);
  const truncated = useTruncated(nameRef, account.name);
  const needsSignIn = account.signInRequired;
  const stateId = "account-chip-state";
  return (
    <DropdownMenu>
      <Tooltip disabled={!truncated}>
        <TooltipTrigger
          render={
            <DropdownMenuTrigger
              ref={setChip}
              aria-label={t("accounts.chip", { name: account.name })}
              aria-describedby={needsSignIn ? stateId : undefined}
              className="flex h-10 shrink-0 items-center gap-2 rounded-pill border border-input bg-secondary py-1 pr-3 pl-1 text-small text-foreground transition-[background-color] duration-(--dur-press) ease-out select-none hover:bg-hover active:bg-selected active:duration-0 data-popup-open:bg-selected"
            />
          }
        >
          <span
            aria-hidden="true"
            className="flex size-8 shrink-0 items-center justify-center rounded-pill bg-hover text-label"
          >
            {initialOf(account.name)}
          </span>
          {/* Below 880 CSS px only the circle and the chevron stay (DESIGN.md 7.3). */}
          <span ref={nameRef} className="max-w-52 min-w-0 truncate max-medium:hidden">
            {account.name}
          </span>
          {needsSignIn && (
            <Badge className="shrink-0 max-medium:hidden">{t("accounts.needsSignIn")}</Badge>
          )}
          <ChevronDownIcon className="size-4 shrink-0" aria-hidden="true" />
        </TooltipTrigger>
        <TooltipContent side="bottom">{account.name}</TooltipContent>
      </Tooltip>
      {needsSignIn && (
        <span id={stateId} className="sr-only">
          {t("accounts.needsSignIn")}
        </span>
      )}
      <AccountMenuContent accounts={accounts} />
    </DropdownMenu>
  );
}

/** DESIGN.md 10.3 and 11.4: the account in the header, in all its states. */
export function AccountChip() {
  const { t } = useTranslation();
  const accounts = useApp((s) => s.accounts);
  const openSignIn = useApp((s) => s.openSignIn);

  if (accounts.status === "error" && accounts.error) {
    return (
      <DropdownMenu>
        <DropdownMenuTrigger ref={setChip} render={<Button />}>
          <TriangleAlertIcon aria-hidden="true" />
          {t("accounts.unavailable")}
        </DropdownMenuTrigger>
        <AccountErrorMenuContent error={accounts.error} />
      </DropdownMenu>
    );
  }

  if (!accounts.data) {
    // Loading: a static block after 200 ms (DESIGN.md 11.4), the size of the chip.
    return (
      <Delayed>
        <div aria-busy="true" className="h-10 w-50 shrink-0 rounded-pill bg-hover" />
      </Delayed>
    );
  }

  if (accounts.data.accounts.length === 0) {
    // Without a client id the store shows the no-client-id dialog instead of the sign-in dialog.
    return (
      <Button ref={setChip} onClick={() => openSignIn(null)}>
        <LogInIcon aria-hidden="true" />
        {t("accounts.signInButton")}
      </Button>
    );
  }

  return <AccountChipButton accounts={accounts.data.accounts} />;
}
