import { useApp } from "@/stores/app";
import { NoClientIdDialog } from "./NoClientIdDialog";
import { RemoveAccountDialog } from "./RemoveAccountDialog";
import { SignInDialog } from "./SignInDialog";

/** The one dialog of the accounts feature that is open (the store holds which). */
export function AccountDialogs() {
  const dialog = useApp((s) => s.accountDialog);
  const close = useApp((s) => s.closeAccountDialog);
  if (!dialog) return null;
  switch (dialog.kind) {
    // A new `id` per opening is a new sign-in with a fresh state.
    case "signIn":
      return <SignInDialog key={dialog.id} resign={dialog.resign} onClosed={close} />;
    case "noClientId":
      return <NoClientIdDialog onClosed={close} />;
    case "remove":
      return <RemoveAccountDialog account={dialog.account} onClosed={close} />;
  }
}
