import type { TFunction } from "i18next";
import type { LoginPagesDto } from "@/ipc/bindings/LoginPagesDto";

/**
 * The texts of the page the browser shows after the Microsoft redirect, in the UI language. The
 * success text says only that the user can return to the launcher: the Xbox and game steps
 * follow and can still fail, so it must not claim "you are signed in".
 */
export function loginPages(t: TFunction): LoginPagesDto {
  return {
    successTitle: t("accounts.loginPage.successTitle"),
    successBody: t("accounts.loginPage.successBody"),
    failureTitle: t("accounts.loginPage.failureTitle"),
    failureBody: t("accounts.loginPage.failureBody"),
  };
}
