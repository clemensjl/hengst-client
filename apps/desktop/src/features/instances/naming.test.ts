import { describe, expect, it } from "vitest";
import { suggestName } from "./naming";

describe("suggestName", () => {
  it("suggests '<game> <Loader>' from the display name", () => {
    expect(suggestName("1.20.1", { kind: "forge", displayName: "Forge" })).toBe("1.20.1 Forge");
    expect(suggestName("1.21.11", null)).toBe("1.21.11");
    expect(suggestName("1.8.9", { kind: "legacy-fabric", displayName: "Legacy Fabric" })).toBe(
      "1.8.9 Legacy Fabric",
    );
  });
});
