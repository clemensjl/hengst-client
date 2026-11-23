import { describe, expect, it } from "vitest";
import { classConflicts } from "./classConflicts";
import { cnClassLists } from "./cnCalls";

const conflictsIn = (source: string) => cnClassLists(source).flatMap(classConflicts);

describe("class lists of cn(...) calls", () => {
  it("joins the arguments of one call into one list", () => {
    expect(cnClassLists('cn("flex gap-2", "p-4")')).toEqual(["flex gap-2 p-4"]);
    expect(cnClassLists('<i className={cn("flex", "gap-2")} />')).toEqual(["flex gap-2"]);
  });

  it("treats the branches of a conditional as alternatives, never as both", () => {
    expect(cnClassLists('cn("flex", on ? "px-4" : "px-2")').sort()).toEqual([
      "flex px-2",
      "flex px-4",
    ]);
    expect(conflictsIn('cn("flex", on ? "px-4" : "px-2")')).toEqual([]);
    expect(cnClassLists('cn("flex", a ? "p-1" : b ? "p-2" : "p-3")')).toHaveLength(3);
  });

  it("treats `cond && class` as the class or nothing", () => {
    // "flex" alone is a list of one class and cannot conflict, so it is not reported.
    expect(cnClassLists('cn("flex", on && "gap-2")')).toEqual(["flex gap-2"]);
    expect(cnClassLists('cn("flex p-1", on && "gap-2")').sort()).toEqual([
      "flex p-1",
      "flex p-1 gap-2",
    ]);
    expect(cnClassLists('cn("flex", a && b && "gap-2")')).toContain("flex gap-2");
  });

  it("reads objects and arrays and nested calls", () => {
    expect(cnClassLists('cn("flex", { "gap-2": on, "p-4": other })')).toContain("flex gap-2 p-4");
    expect(cnClassLists('cn("flex", ["gap-2", "p-4"])')).toEqual(["flex gap-2 p-4"]);
    expect(cnClassLists('cn("flex", cn("gap-2", "p-4"))')).toContain("flex gap-2 p-4");
  });

  it("takes what is visible and ignores the rest", () => {
    expect(cnClassLists('cn("flex gap-2", className)')).toEqual(["flex gap-2"]);
    expect(cnClassLists('cn("flex gap-2", buttonVariants({ variant }), props.class)')).toEqual([
      "flex gap-2",
    ]);
    expect(cnClassLists('cn(`flex ${gap} p-4`, "gap-2")')).toEqual(["flex x p-4 gap-2"]);
  });

  it("is not thrown off by brackets and quotes inside strings", () => {
    expect(cnClassLists(`cn("a)b", 'c"d', "px-4")`)).toEqual(['a)b c"d px-4']);
    expect(cnClassLists('const x = "cn(a b)"; cn("p-1 m-1", "w-1")')).toEqual(["p-1 m-1 w-1"]);
  });

  it("does not take a longer name for the call", () => {
    expect(cnClassLists('mycn("px-4", "px-2") + btn.cn("px-4", "px-2")')).toEqual([]);
  });
});

describe("conflicts across the arguments of one cn call", () => {
  it("finds two classes of the same property in different arguments", () => {
    for (const source of [
      'cn("px-4", "px-2")',
      'cn("flex px-4", "px-2")',
      'cn("p-4", on && "p-2")',
      'cn("rounded-md", big ? "rounded-lg" : "rounded-sm")',
      'cn("text-label", { "text-small": on })',
      'cn("px-4", cn("gap-2", "px-2"))',
      'cn(["h-10"], "h-8")',
      'cn("bg-surface", `bg-hover ${x}`)',
    ]) {
      expect(conflictsIn(source), source).not.toEqual([]);
    }
  });

  it("lets alternatives, general-then-specific pairs and unknown arguments through", () => {
    for (const source of [
      'cn("px-4", on ? "py-2" : "py-4")',
      'cn("flex", on ? "px-4" : "px-2")',
      'cn("p-4", "px-2")',
      'cn("rounded-md", "rounded-t-lg")',
      'cn("text-body", "font-semibold")',
      'cn("px-4", className)',
      'cn("hidden", on && "wide:flex")',
    ]) {
      expect(conflictsIn(source), source).toEqual([]);
    }
  });
});
