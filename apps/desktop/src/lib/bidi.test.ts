import { describe, expect, it } from "vitest";
import { isolate, stripBidi } from "./bidi";

const controls = [
  "\u200E",
  "\u200F",
  "\u061C",
  "\u202A",
  "\u202B",
  "\u202C",
  "\u202D",
  "\u202E",
  "\u2066",
  "\u2067",
  "\u2068",
  "\u2069",
];

describe("bidi", () => {
  it("stripBidi drops every mark, embedding, override and isolate control, as Rust refuses them", () => {
    for (const c of controls) expect(stripBidi(`C:\\x${c}exe.avaj`)).toBe("C:\\xexe.avaj");
  });

  it("isolate wraps the cleaned name in one first-strong isolate", () => {
    expect(isolate("Mira\u200F_K\u2069")).toBe("\u2068Mira_K\u2069");
  });
});
