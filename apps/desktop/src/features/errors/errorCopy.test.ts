import { readFileSync } from "node:fs";
import path from "node:path";
import { describe, expect, it } from "vitest";
import { tableRows } from "@/styles/designDoc";
import i18n, { initI18n } from "@/i18n";
import type { ErrorDto } from "@/ipc/bindings/ErrorDto";
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
        // Task 5 adds its own three codes to ErrorCode; they are checked once they exist.
        if (!(code in errorKeys)) continue;
        const keys = errorKeys[code as ErrorDto["code"]];
        expect(de(keys.title), `${code} title de`).toBe(title[0]);
        expect(en(keys.title), `${code} title en`).toBe(title[1]);
        expect(de(keys.body), `${code} body de`).toBe(body[0]);
        expect(en(keys.body), `${code} body en`).toBe(body[1]);
        checked++;
      }
    }
    // network, offline, loginDeclined, deviceCodeExpired, appMisconfigured, noXboxAccount,
    // gameNotOwned, noProfile, credentialStore
    expect(checked).toBeGreaterThanOrEqual(9);
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
