import { ChevronDownIcon, LogInIcon, TriangleAlertIcon, UserRoundIcon } from "lucide-react";
import { useEffect, useRef } from "react";
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
import { chipRef } from "./dialogReturn";

export { chipRef } from "./dialogReturn";
const setChip = (el: HTMLElement | null) => {
  chipRef.current = el;
};

/** The first letter of the name as a whole character (a name may start with a surrogate pair). */
const initialOf = (name: string) => (Array.from(name)[0] ?? "?").toUpperCase();

/** Accounts exist but none is selected: the chip says so instead of naming some other account. */
function ChooseAccountButton({ accounts }: { accounts: AccountDto[] }) {
  const { t } = useTranslation();
  // Controlled through the store: the Play button's "Choose account" opens this same menu.
  const open = useApp((s) => s.accountMenuOpen);
  const setOpen = useApp((s) => s.setAccountMenuOpen);
  // An account chosen elsewhere replaces this button: the menu must not reopen with the next one.
  useEffect(() => () => setOpen(false), [setOpen]);
  return (
    <DropdownMenu open={open} onOpenChange={setOpen}>
      <DropdownMenuTrigger ref={setChip} render={<Button />}>
        <UserRoundIcon aria-hidden="true" />
        {t("accounts.choose")}
        <ChevronDownIcon aria-hidden="true" />
      </DropdownMenuTrigger>
      <AccountMenuContent accounts={accounts} />
    </DropdownMenu>
  );
}

function AccountChipButton({ accounts, account }: { accounts: AccountDto[]; account: AccountDto }) {
  const { t } = useTranslation();
  const nameRef = useRef<HTMLSpanElement>(null);
  const truncated = useTruncated(nameRef, account.name);
  const needsSignIn = account.signInRequired;
  const stateId = "account-chip-state";
  const hintId = "account-chip-hint";
  return (
    <DropdownMenu>
      <Tooltip disabled={!truncated}>
        <TooltipTrigger
          render={
            <DropdownMenuTrigger
              ref={setChip}
              aria-label={t("accounts.chip", { name: account.name })}
              aria-describedby={needsSignIn ? `${hintId} ${stateId}` : hintId}
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
          {/* 8 px from the circle to the name, 4 px from the name to the chevron (DESIGN.md 10.3). */}
          <ChevronDownIcon className="-ml-1 size-4 shrink-0" aria-hidden="true" />
        </TooltipTrigger>
        <TooltipContent side="bottom">{account.name}</TooltipContent>
      </Tooltip>
      <span id={hintId} className="sr-only">
        {t("accounts.chipHint")}
      </span>
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
          <TriangleAlertIcon className="text-warning" aria-hidden="true" />
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

  const list = accounts.data.accounts;
  const selected = list.find((a) => a.selected);
  return selected ? (
    <AccountChipButton accounts={list} account={selected} />
  ) : (
    <ChooseAccountButton accounts={list} />
  );
}
