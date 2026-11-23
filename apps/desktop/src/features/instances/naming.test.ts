import { describe, expect, it } from "vitest";
import { nameLength, nameProblem, suggestName } from "./naming";

describe("suggestName", () => {
  it("suggests '<game> <Loader>' from the display name", () => {
    expect(suggestName("1.20.1", { kind: "forge", displayName: "Forge" })).toBe("1.20.1 Forge");
    expect(suggestName("1.21.11", null)).toBe("1.21.11");
    expect(suggestName("1.8.9", { kind: "legacy-fabric", displayName: "Legacy Fabric" })).toBe(
      "1.8.9 Legacy Fabric",
    );
  });
});

describe("name rules equal the backend's validate_name", () => {
  const BOM = "﻿";
  const x = (n: number) => "x".repeat(n);
  // [input, problem, counted length]; Rust trims Unicode White_Space (not U+FEFF), counts chars,
  // refuses empty, more than 64 and control characters, in that order.
  const table: [string, string | null, number][] = [
    ["", "instances.form.nameEmpty", 0],
    ["   ", "instances.form.nameEmpty", 0],
    [`${x(64)} `, null, 64],
    [` ${x(64)}　`, null, 64],
    [x(65), "instances.form.nameTooLong", 65],
    [`${BOM}${x(63)}`, null, 64],
    [`${BOM}${x(64)}`, "instances.form.nameTooLong", 65],
    [`${BOM}`, null, 1],
    ["\u0085Mira\u0085", null, 4],
    ["a\tb", "instances.form.nameControl", 3],
    ["Mira\u0007", "instances.form.nameControl", 5],
    ["a\u007fb", "instances.form.nameControl", 3],
    ["Überleben \u{1D510}", null, 11],
    ["Wiśniewska", null, 10],
  ];
  it.each(table)("%j", (input, problem, length) => {
    expect(nameProblem(input)).toBe(problem);
    expect(nameLength(input)).toBe(length);
  });
});
