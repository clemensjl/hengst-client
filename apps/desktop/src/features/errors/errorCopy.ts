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
  browserLoginTimedOut: keys("browserLoginTimedOut"),
  loopbackUnavailable: keys("loopbackUnavailable"),
  browserLoginFailed: keys("browserLoginFailed"),
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
  instanceNotFound: keys("instanceNotFound"),
  instanceExists: keys("instanceExists"),
  instanceFormatTooNew: keys("instanceFormatTooNew"),
  invalidInstanceName: keys("invalidInstanceName"),
  instanceUnreadable: keys("instanceUnreadable"),
  instanceLinked: keys("instanceLinked"),
  recycleBinPartial: keys("recycleBinPartial"),
  recycleBinFailed: keys("recycleBinFailed"),
  recycleBinUnconfirmed: keys("recycleBinUnconfirmed"),
  instanceRunning: keys("instanceRunning"),
  busy: keys("busy"),
};

const REASONS = ["tooLarge", "binOff", "noBin", "inUse", "other", "unchecked", "unknown"];

/** The params the copy interpolates; everything else in `params` is technical and stays in the details. */
const COPY_PARAMS = ["loader", "game", "version", "name"] as const;

/**
 * Where an error shows up, for codes whose wording depends on it: `errors.<code>.<context>.title`
 * and `.body` are looked up first, the generic `errors.<code>.*` is the fallback. `signIn` carries
 * the wording of DESIGN.md 11.5, `launch` the offline text of the launch panel (10.2), `folder`
 * the failure of "Open folder" (`openFailed` is about a link in the browser otherwise).
 */
export type ErrorContext = "signIn" | "launch" | "folder";

/*
 * `missingClientId` is the "build without client id" state of DESIGN.md 11.6. It cannot occur in a
 * build of this repository (the id is built into the core and an empty override falls back to it);
 * it exists for forks that remove the id. The copy is the single source for the 11.6 block.
 */

/** Title, body and the label of the error's own action (none for `retry`: that is the panel's retry button). */
export function describeError(
  dto: ErrorDto,
  t: TFunction,
  context?: ErrorContext,
): { title: string; body: string; actionLabel?: string } {
  const values: Record<string, string> = {};
  for (const key of COPY_PARAMS) {
    const value = dto.params[key];
    if (value !== undefined) values[key] = value;
  }
  // A code this bundle does not know (a newer backend) shows the internal copy.
  const code = dto.code in errorKeys ? dto.code : "internal";
  const lookup = (field: "title" | "body") => {
    const generic = errorKeys[code][field];
    return context ? [`errors.${code}.${context}.${field}`, generic] : generic;
  };
  let body = t(lookup("body"), values);
  const hint = code === "noLoaderVersions" ? dto.params.hint : undefined;
  if (hint) body = `${body} ${t(`errors.noLoaderVersions.hint.${hint}`)}`;
  // Why a folder could not go to the Recycle Bin comes first; an unknown reason is left out.
  const reason = code === "recycleBinFailed" ? dto.params.reason : undefined;
  if (reason && REASONS.includes(reason)) {
    body = `${t(`errors.recycleBinFailed.reason.${reason}`)} ${body}`;
  }
  return { title: t(lookup("title"), values), body, actionLabel: actionLabel(dto.action, t) };
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
