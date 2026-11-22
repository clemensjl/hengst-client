import { readFileSync } from "node:fs";
import path from "node:path";
import { describe, expect, it } from "vitest";
import { DESIGN, tableRows } from "@/styles/designDoc";
import i18n, { initI18n } from "@/i18n";
import type { ErrorDto } from "@/ipc/bindings/ErrorDto";
import de_json from "@/i18n/de.json";
import en_json from "@/i18n/en.json";
import { describeError, errorKeys } from "./errorCopy";

initI18n("en", "en-US");
const de = i18n.getFixedT("de");
const en = i18n.getFixedT("en");

const dto = (code: ErrorDto["code"], params: Record<string, string> = {}): ErrorDto => ({
  code,
  params,
  action: null,
  detail: "",
});

const codes = Object.keys(errorKeys) as ErrorDto["code"][];

describe("errorKeys", () => {
  it("covers exactly the codes of the generated ErrorCode type", () => {
    const text = readFileSync(
      path.resolve(import.meta.dirname, "../../ipc/bindings/ErrorCode.ts"),
      "utf8",
    );
    const union = text.slice(text.indexOf("export type ErrorCode"));
    const generated = [...union.matchAll(/"([a-zA-Z]+)"/g)].map((m) => m[1]);
    expect(generated.length).toBeGreaterThan(30);
    expect([...codes].sort()).toEqual([...generated].sort());
  });

  it("every error code has German and English title and body", () => {
    for (const code of codes) {
      for (const [lang, t] of [
        ["de", de],
        ["en", en],
      ] as const) {
        const { title, body } = errorKeys[code];
        for (const key of [title, body]) {
          const text = t(key, { loader: "L", game: "G", version: "V", name: "N" });
          expect(text, `${lang} ${key}`).not.toBe(key);
          expect(text.trim(), `${lang} ${key}`).not.toBe("");
        }
      }
    }
  });

  it("titles end without a full stop and bodies end with one, in de and en", () => {
    for (const code of codes) {
      const { title, body } = errorKeys[code];
      for (const t of [de, en]) {
        expect(t(title).endsWith("."), `${title}`).toBe(false);
        expect(t(body).endsWith("."), `${body}`).toBe(true);
      }
    }
  });

  it("context titles end without a full stop and context bodies end with one, in de and en", () => {
    // errors.<code>.<context>.title|body: the wording that differs by place (sign-in, launch, folder).
    const contextKeys = Object.keys(en_json).filter((k) =>
      /^errors\.[a-zA-Z]+\.(signIn|launch|folder)\.(title|body)$/.test(k),
    );
    expect(contextKeys.length).toBeGreaterThanOrEqual(10);
    for (const json of [de_json, en_json] as Record<string, string>[]) {
      for (const key of contextKeys) {
        const text = json[key];
        expect(text, key).toBeDefined();
        expect(text.endsWith("."), key).toBe(key.endsWith(".body"));
      }
    }
  });

  it("the hint sentences are complete sentences in both languages", () => {
    for (const hint of ["neoForgeStartsAt1201", "fabricStartsAt114", "legacyFabricServesOld"]) {
      for (const t of [de, en]) {
        const text = t(`errors.noLoaderVersions.hint.${hint}`);
        expect(text, hint).not.toBe(`errors.noLoaderVersions.hint.${hint}`);
        expect(text.endsWith("."), hint).toBe(true);
      }
    }
  });
});

const ALL_CODES = [
  "internal",
  "invalidInput",
  "network",
  "offline",
  "serverError",
  "checksumMismatch",
  "unsafeData",
  "unknownVersion",
  "noJavaRuntime",
  "offlineAccountBlocked",
  "noLoaderVersions",
  "unknownLoaderVersion",
  "loaderUnsupported",
  "installerInvalid",
  "installFailed",
  "installStepFailed",
  "nativesInUse",
  "lockTimeout",
  "credentialStore",
  "missingClientId",
  "appMisconfigured",
  "appNotApproved",
  "loginDeclined",
  "loginExpired",
  "deviceCodeExpired",
  "browserLoginTimedOut",
  "loopbackUnavailable",
  "browserLoginFailed",
  "noXboxAccount",
  "xboxRegionBlocked",
  "xboxAdultVerification",
  "xboxChildAccount",
  "xboxRefused",
  "gameNotOwned",
  "noProfile",
  "accountNotFound",
  "noAccount",
  "cancelled",
  "fileSystem",
  "openFailed",
  "instanceNotFound",
  "instanceExists",
  "instanceFormatTooNew",
  "invalidInstanceName",
  "instanceUnreadable",
  "instanceLinked",
  "recycleBinPartial",
  "recycleBinFailed",
  "instanceRunning",
  "busy",
];

describe("wire codes and copy groups", () => {
  it("pins every code string", () => {
    expect([...codes].sort()).toEqual([...ALL_CODES].sort());
  });

  // Groups under `errors.` that are not codes: the panel's own labels, the settings load error and
  // the About panel's inline texts.
  const NON_CODE_GROUPS = [
    "action",
    "details",
    "copy",
    "copied",
    "copyFailed",
    "settingsLoadFailed",
    "versionsOffline",
    "versionsFailed",
    "versionsRefreshFailed",
  ];
  const CONTEXTS = ["signIn", "launch", "folder"];

  it("no errors.<group> exists without a matching code or a listed non-code group", () => {
    for (const json of [de_json, en_json] as Record<string, string>[]) {
      for (const key of Object.keys(json).filter((k) => k.startsWith("errors."))) {
        const [, group, second] = key.split(".");
        const known = ALL_CODES.includes(group) || NON_CODE_GROUPS.includes(group);
        expect(known, key).toBe(true);
        // A context group sits directly under a code: errors.<code>.<context>.title|body.
        if (ALL_CODES.includes(group) && !["title", "body", "hint"].includes(second)) {
          expect(CONTEXTS, key).toContain(second);
        }
      }
    }
  });
});

/** The offline panel of DESIGN.md 10.12: title and body, each as the pair [de, en]. */
function readLaunchOffline() {
  const line = DESIGN.split("\n").find((l) => l.startsWith("- Offline: if all files are cached"));
  if (!line) throw new Error("DESIGN.md 10.12 has no offline line any more");
  const pairs = [...line.matchAll(/"([^"]+)" \/ "([^"]+)"/g)].map((m) => [m[1], m[2]]);
  // ErrorPanel "Keine Verbindung" / "No connection", body "Für diese ..." / "Files for ...".
  if (pairs.length < 2) throw new Error(`cannot read the offline texts from: ${line}`);
  return { title: pairs[0], body: pairs[1] };
}
const launchOffline = readLaunchOffline();

describe("error contexts", () => {
  it("reads the launch wording from DESIGN.md", () => {
    expect(launchOffline.title).toEqual(["Keine Verbindung", "No connection"]);
    expect(launchOffline.body[1]).toMatch(/^Files for this version are missing\./);
  });

  it("network and offline are generic by default, the sign-in wording only in the sign-in context", () => {
    for (const code of ["network", "offline"] as const) {
      expect(describeError(dto(code), en).title).toBe("No connection");
      expect(describeError(dto(code), de).title).toBe("Keine Verbindung");
      expect(describeError(dto(code), en, "signIn").title).toBe("Cannot reach Microsoft");
      expect(describeError(dto(code), de, "signIn").title).toBe("Keine Verbindung zu Microsoft");
      // The launch wording is DESIGN.md 10.12's offline panel, read from there.
      expect(describeError(dto(code), de, "launch")).toMatchObject({
        title: launchOffline.title[0],
        body: launchOffline.body[0],
      });
      expect(describeError(dto(code), en, "launch")).toMatchObject({
        title: launchOffline.title[1],
        body: launchOffline.body[1],
      });
      // The generic copy names no service.
      expect(JSON.stringify(describeError(dto(code), en))).not.toMatch(/Microsoft/);
    }
  });

  it("credentialStore is generic by default and says the sign-in was not saved in the sign-in context", () => {
    expect(describeError(dto("credentialStore"), en).title).not.toMatch(/sign-in/i);
    expect(describeError(dto("credentialStore"), en, "signIn").title).toBe(
      "The sign-in could not be saved",
    );
    expect(describeError(dto("credentialStore"), de, "signIn").title).toBe(
      "Die Anmeldung konnte nicht gespeichert werden",
    );
  });

  it("a context without its own text for a code falls back to the generic text", () => {
    expect(describeError(dto("serverError"), en, "launch")).toEqual(
      describeError(dto("serverError"), en),
    );
  });

  it("a code this bundle does not know shows the internal copy instead of throwing", () => {
    const unknown = dto("fromTheFuture" as ErrorDto["code"]);
    expect(describeError(unknown, en, "signIn")).toEqual(describeError(dto("internal"), en));
  });
});

describe("describeError", () => {
  it("noLoaderVersions with hint shows the hint sentence", () => {
    const e = dto("noLoaderVersions", {
      loader: "Fabric",
      loaderSlug: "fabric",
      game: "1.8.9",
      hint: "fabricStartsAt114",
    });
    expect(describeError(e, en).body).toContain("1.14");
    expect(describeError(e, de).body).toContain("1.14");
    const without = describeError(dto("noLoaderVersions", { loader: "Fabric", game: "1.21" }), en);
    expect(without.body).not.toContain("1.14");
  });

  it("noLoaderVersions shows the display name, never the slug", () => {
    const e = dto("noLoaderVersions", {
      loader: "Legacy Fabric",
      loaderSlug: "legacy-fabric",
      game: "1.21.11",
      hint: "legacyFabricServesOld",
    });
    for (const t of [de, en]) {
      const { title, body } = describeError(e, t);
      expect(title).toContain("Legacy Fabric");
      expect(title).toContain("1.21.11");
      expect(`${title} ${body}`).not.toContain("legacy-fabric");
    }
  });

  it("names the action of the error in the UI language", () => {
    const withAction = (action: ErrorDto["action"]) => ({ ...dto("network"), action });
    expect(describeError(withAction({ kind: "signIn" }), en).actionLabel).toBe("Sign in again");
    expect(describeError(withAction({ kind: "signIn" }), de).actionLabel).toBe("Erneut anmelden");
    expect(describeError(withAction({ kind: "openSettings" }), en).actionLabel).toBe(
      "Open settings",
    );
    expect(
      describeError(withAction({ kind: "openUrl", target: "xboxSignup" }), de).actionLabel,
    ).toBe("Xbox-Profil anlegen");
    expect(
      describeError(withAction({ kind: "openUrl", target: "minecraftProfile" }), en).actionLabel,
    ).toBe("Create profile");
    expect(
      describeError(withAction({ kind: "openUrl", target: "buildHelp" }), en).actionLabel,
    ).toBe("Build instructions");
    // Retry is the panel's own button, not a second one.
    expect(describeError(withAction({ kind: "retry" }), en).actionLabel).toBeUndefined();
    expect(describeError(dto("network"), en).actionLabel).toBeUndefined();
  });

  it("every openUrl target has a label in both languages", () => {
    const targets = [
      "repository",
      "signInHelp",
      "xboxSignup",
      "minecraftProfile",
      "deviceLogin",
      "buildHelp",
    ] as const;
    for (const target of targets) {
      for (const t of [de, en]) {
        const label = describeError(
          { ...dto("network"), action: { kind: "openUrl", target } },
          t,
        ).actionLabel;
        expect(label, target).toBeTruthy();
        expect(label, target).not.toMatch(/^errors\./);
      }
    }
  });

  it("ignores params that are not interpolation values", () => {
    const e = dto("unknownVersion", { version: "9.9", lng: "de", count: "5", defaultValue: "x" });
    expect(describeError(e, en).title).toBe("Game version 9.9 is unknown");
  });
});

describe("copy of DESIGN.md 11.5 and 11.6", () => {
  const rows = tableRows("Error copy in the failed phase (title without full stop, body, action):");
  const pair = (cell: string) => cell.match(/^"(.+)" \/ "(.+)"$/)?.slice(1);

  it("reads the table", () => {
    expect(rows.length).toBeGreaterThanOrEqual(10);
  });

  it("the sign-in errors use the wording of the table, one text per code", () => {
    let checked = 0;
    for (const [codeCell, titleCell, bodyCell] of rows) {
      const title = pair(titleCell);
      const body = pair(bodyCell);
      if (!title || !body) continue;
      for (const code of [...codeCell.matchAll(/`([a-zA-Z]+)`/g)].map((m) => m[1])) {
        const d = describeError(dto(code as ErrorDto["code"]), de, "signIn");
        const e = describeError(dto(code as ErrorDto["code"]), en, "signIn");
        expect(d.title, `${code} title de`).toBe(title[0]);
        expect(e.title, `${code} title en`).toBe(title[1]);
        expect(d.body, `${code} body de`).toBe(body[0]);
        expect(e.body, `${code} body en`).toBe(body[1]);
        checked++;
      }
    }
    // network, offline, loginDeclined, browserLoginTimedOut, deviceCodeExpired,
    // loopbackUnavailable, browserLoginFailed, appMisconfigured, noXboxAccount, gameNotOwned,
    // noProfile, credentialStore
    expect(checked).toBeGreaterThanOrEqual(12);
  });

  it("missingClientId and appNotApproved carry the texts of 11.6", () => {
    expect(de(errorKeys.missingClientId.title)).toBe("Dieser Build kann sich nicht anmelden");
    expect(en(errorKeys.missingClientId.title)).toBe("This build cannot sign in");
    expect(en(errorKeys.missingClientId.body)).toBe(
      "It was built without a Microsoft client id. Use an official release, or set the client id when building it yourself.",
    );
    expect(de(errorKeys.appNotApproved.title)).toBe("Anmeldung noch nicht verfügbar");
    expect(en(errorKeys.appNotApproved.title)).toBe("Sign-in is not available yet");
    expect(en(errorKeys.appNotApproved.body)).toBe(
      "Mojang has not approved Hengst Client's Microsoft sign-in yet. Until then, accounts cannot be added. Your account is fine, nothing needs to change on your side.",
    );
  });
});
