import { describe, expect, it } from "vitest";
import i18n, { initI18n } from "@/i18n";
import type { InstanceDto } from "@/ipc/bindings/InstanceDto";
import { lastPlayedText, loaderText } from "./format";

initI18n("en", "en-US");
const t = i18n.t.bind(i18n);
const NOW = Date.UTC(2026, 10, 22, 12, 0, 0);
const ago = (seconds: number) => NOW / 1000 - seconds;

const withLoader = (loader: InstanceDto["loader"]) => ({ loader });

describe("loaderText", () => {
  it("names the loader with its version, or says there is none", () => {
    expect(
      loaderText(withLoader({ kind: "fabric", displayName: "Fabric", version: "0.17.3" }), t),
    ).toBe("Fabric 0.17.3");
    // A folder adopted from the CLI has no saved loader version yet.
    expect(loaderText(withLoader({ kind: "forge", displayName: "Forge", version: null }), t)).toBe(
      "Forge",
    );
    expect(loaderText(withLoader(null), t)).toBe("No loader");
  });
});

describe("lastPlayedText", () => {
  it("is relative up to 6 days, a medium date from 7 days, and 'Never played' without a launch", () => {
    expect(lastPlayedText(null, "en", t, NOW)).toBe("Never played");
    expect(lastPlayedText(ago(20), "en", t, NOW)).toBe("now");
    expect(lastPlayedText(ago(5 * 60), "en", t, NOW)).toBe("5 minutes ago");
    expect(lastPlayedText(ago(86400 + 60), "de", t, NOW)).toBe("vor 1 Tag");
    expect(lastPlayedText(ago(3 * 3600), "en", t, NOW)).toBe("3 hours ago");
    expect(lastPlayedText(ago(2 * 86400), "en", t, NOW)).toBe("2 days ago");
    expect(lastPlayedText(ago(2 * 86400), "de", t, NOW)).toBe("vor 2 Tagen");
    expect(lastPlayedText(ago(6 * 86400), "en", t, NOW)).toBe("6 days ago");
    expect(lastPlayedText(ago(7 * 86400), "en", t, NOW)).toBe("Nov 15, 2026");
    // A clock that is behind the saved time does not say "in 3 minutes".
    expect(lastPlayedText(ago(-180), "en", t, NOW)).toBe("now");
  });
});
