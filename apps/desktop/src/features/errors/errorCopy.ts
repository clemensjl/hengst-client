import type { TFunction } from "i18next";
import type { ErrorCode } from "@/ipc/bindings/ErrorCode";
import type { ErrorDto } from "@/ipc/bindings/ErrorDto";

const keys = (code: ErrorCode) => ({ title: `errors.${code}.title`, body: `errors.${code}.body` });

/** The i18n keys of every error code. A `Record` makes a code without copy a type error. */
export const errorKeys: Record<ErrorCode, { title: string; body: string }> = {
  internal: keys("internal"),
  invalidInput: keys("invalidInput"),
  network: keys("network"),
  offline: keys("offline"),
  serverError: keys("serverError"),
  checksumMismatch: keys("checksumMismatch"),
  unsafeData: keys("unsafeData"),
  unknownVersion: keys("unknownVersion"),
  noJavaRuntime: keys("noJavaRuntime"),
  offlineAccountBlocked: keys("offlineAccountBlocked"),
  noLoaderVersions: keys("noLoaderVersions"),
  unknownLoaderVersion: keys("unknownLoaderVersion"),
  loaderUnsupported: keys("loaderUnsupported"),
  installerInvalid: keys("installerInvalid"),
  installFailed: keys("installFailed"),
  installStepFailed: keys("installStepFailed"),
  nativesInUse: keys("nativesInUse"),
  lockTimeout: keys("lockTimeout"),
  credentialStore: keys("credentialStore"),
  missingClientId: keys("missingClientId"),
  appMisconfigured: keys("appMisconfigured"),
  appNotApproved: keys("appNotApproved"),
  loginDeclined: keys("loginDeclined"),
  loginExpired: keys("loginExpired"),
  deviceCodeExpired: keys("deviceCodeExpired"),
  noXboxAccount: keys("noXboxAccount"),
  xboxRegionBlocked: keys("xboxRegionBlocked"),
  xboxAdultVerification: keys("xboxAdultVerification"),
  xboxChildAccount: keys("xboxChildAccount"),
  xboxRefused: keys("xboxRefused"),
  gameNotOwned: keys("gameNotOwned"),
  noProfile: keys("noProfile"),
  accountNotFound: keys("accountNotFound"),
  noAccount: keys("noAccount"),
  cancelled: keys("cancelled"),
  fileSystem: keys("fileSystem"),
  openFailed: keys("openFailed"),
};

/** The params the copy interpolates; everything else in `params` is technical and stays in the details. */
const COPY_PARAMS = ["loader", "game", "version", "name"] as const;

/** Title, body and the label of the error's own action (none for `retry`: that is the panel's retry button). */
export function describeError(
  dto: ErrorDto,
  t: TFunction,
): { title: string; body: string; actionLabel?: string } {
  const values: Record<string, string> = {};
  for (const key of COPY_PARAMS) {
    const value = dto.params[key];
    if (value !== undefined) values[key] = value;
  }
  const { title, body } = errorKeys[dto.code];
  let bodyText = t(body, values);
  const hint = dto.code === "noLoaderVersions" ? dto.params.hint : undefined;
  if (hint) bodyText = `${bodyText} ${t(`errors.noLoaderVersions.hint.${hint}`)}`;
  return { title: t(title, values), body: bodyText, actionLabel: actionLabel(dto.action, t) };
}

function actionLabel(action: ErrorDto["action"], t: TFunction): string | undefined {
  switch (action?.kind) {
    case "signIn":
      return t("errors.action.signIn");
    case "openSettings":
      return t("errors.action.openSettings");
    case "openUrl":
      return t(`errors.action.openUrl.${action.target}`);
    default:
      return undefined;
  }
}
