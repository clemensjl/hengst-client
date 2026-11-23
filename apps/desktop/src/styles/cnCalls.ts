// Test helper: reads the class lists that `cn(...)` calls put together, so the conflict check
// (classConflicts.ts) also sees two classes that sit in different arguments of one call:
// `cn("px-4", "px-2")`. `cn` only joins, so such a pair would depend on CSS order.
//
// A conditional argument counts as its alternatives, never as both at once: `cn("flex", a ? "px-4" :
// "px-2")` has two lists, "flex px-4" and "flex px-2", and no conflict. `a && "px-2"` is "px-2" or
// nothing. An argument that is not a literal (`className`, a function call) is nothing: what it
// holds is not visible here.

const MAX_LISTS = 512;

/** Index of the character after the string literal that starts at `start`. */
function skipString(s: string, start: number): number {
  const quote = s[start];
  let i = start + 1;
  while (i < s.length) {
    if (s[i] === "\\") i += 2;
    else if (s[i] === quote) return i + 1;
    else if (quote === "`" && s[i] === "$" && s[i + 1] === "{") i = skipBalanced(s, i + 1);
    else i++;
  }
  return s.length;
}

/** Index after the bracket group that opens at `open` (strings inside are skipped). */
function skipBalanced(s: string, open: number): number {
  let depth = 0;
  let i = open;
  while (i < s.length) {
    const c = s[i];
    if (c === '"' || c === "'" || c === "`") {
      i = skipString(s, i);
      continue;
    }
    if (c === "(" || c === "[" || c === "{") depth++;
    else if (c === ")" || c === "]" || c === "}") {
      depth--;
      if (depth === 0) return i + 1;
    }
    i++;
  }
  return s.length;
}

/** Splits at the top level (outside strings and brackets) at every `separator` character. */
function splitTop(s: string, separator: string): string[] {
  const parts: string[] = [];
  let from = 0;
  let i = 0;
  while (i < s.length) {
    const c = s[i];
    if (c === '"' || c === "'" || c === "`") i = skipString(s, i);
    else if (c === "(" || c === "[" || c === "{") i = skipBalanced(s, i);
    else {
      if (c === separator) {
        parts.push(s.slice(from, i));
        from = i + 1;
      }
      i++;
    }
  }
  parts.push(s.slice(from));
  return parts;
}

/** Splits at every top-level occurrence of `token` (one or more characters). */
function splitTopToken(s: string, token: string): string[] {
  const parts: string[] = [];
  let from = 0;
  let i = 0;
  while (i < s.length) {
    const c = s[i];
    if (c === '"' || c === "'" || c === "`") i = skipString(s, i);
    else if (c === "(" || c === "[" || c === "{") i = skipBalanced(s, i);
    else if (s.startsWith(token, i)) {
      parts.push(s.slice(from, i));
      i += token.length;
      from = i;
    } else i++;
  }
  parts.push(s.slice(from));
  return parts;
}

/** Splits a top-level `a ? b : c` (nested conditionals in `c` stay in `c`). */
function splitTernary(s: string): [string, string, string] | null {
  let i = 0;
  let question = -1;
  let depth = 0;
  while (i < s.length) {
    const c = s[i];
    if (c === '"' || c === "'" || c === "`") {
      i = skipString(s, i);
      continue;
    }
    if (c === "(" || c === "[" || c === "{") {
      i = skipBalanced(s, i);
      continue;
    }
    if (c === "?" && s[i + 1] !== "." && s[i + 1] !== "?" && s[i - 1] !== "?") {
      if (question < 0) question = i;
      else depth++;
    } else if (c === ":" && question >= 0) {
      if (depth === 0) return [s.slice(0, question), s.slice(question + 1, i), s.slice(i + 1)];
      depth--;
    }
    i++;
  }
  return null;
}

function literalText(token: string): string {
  const inner = token.slice(1, -1);
  return token[0] === "`" ? inner.replace(/\$\{[^}]*\}/g, "x") : inner;
}

const product = (lists: string[][]): string[][] => {
  let out: string[][] = [[]];
  for (const alternatives of lists) {
    out = out.flatMap((prefix) => alternatives.map((a) => [...prefix, a]));
    if (out.length > MAX_LISTS) return out.slice(0, MAX_LISTS);
  }
  return out;
};

/** The class texts an expression can stand for, `""` standing for "nothing". */
function alternatives(expression: string): string[] {
  const e = expression.trim();
  if (e === "") return [""];
  if (/^\(.*\)$/s.test(e) && skipBalanced(e, 0) === e.length) return alternatives(e.slice(1, -1));
  const ternary = splitTernary(e);
  if (ternary) return [...new Set([...alternatives(ternary[1]), ...alternatives(ternary[2])])];
  const and = splitTopToken(e, "&&");
  if (and.length > 1) {
    // The part after the last `&&` is what is added; the parts before it are conditions.
    return [...new Set(["", ...alternatives(and[and.length - 1])])];
  }
  const or = splitTopToken(e, "||").flatMap((part) => splitTopToken(part, "??"));
  if (or.length > 1) return [...new Set(or.flatMap(alternatives))];
  if (/^(["'`])/.test(e) && skipString(e, 0) === e.length) return [literalText(e)];
  const call = e.match(/^(?:cn|clsx)\(/);
  if (call && skipBalanced(e, call[0].length - 1) === e.length) {
    return callLists(e.slice(call[0].length, -1));
  }
  if (e.startsWith("[") && skipBalanced(e, 0) === e.length) {
    return joinedProduct(splitTop(e.slice(1, -1), ","));
  }
  if (e.startsWith("{") && skipBalanced(e, 0) === e.length) {
    // `{ "px-2": cond, "p-1": other }`: each key is on or off.
    const keys = splitTop(e.slice(1, -1), ",")
      .map((entry) => splitTop(entry, ":")[0].trim())
      .filter((key) => /^(["'`])/.test(key))
      .map((key) => [literalText(key), ""]);
    return product(keys).map((l) => l.join(" ").trim());
  }
  return [""];
}

function joinedProduct(args: string[]): string[] {
  return product(args.map(alternatives)).map((l) => l.join(" ").replace(/\s+/g, " ").trim());
}

/** Every class list the arguments of one call can produce. */
function callLists(args: string): string[] {
  return [...new Set(joinedProduct(splitTop(args, ",").filter((a) => a.trim() !== "")))];
}

/** The class lists of every `cn(...)` and `clsx(...)` call in a source text (each outer call once). */
export function cnClassLists(source: string): string[] {
  const out: string[] = [];
  let i = 0;
  while (i < source.length) {
    const c = source[i];
    if (c === '"' || c === "'" || c === "`") {
      // Do not look for calls in string literals (a call inside `${}` is rare and not a class list).
      i = skipString(source, i);
      continue;
    }
    const m = /^(?:cn|clsx)\(/.exec(source.slice(i, i + 5));
    if (m && !/[\w$.]/.test(source[i - 1] ?? "")) {
      const open = i + m[0].length - 1;
      const end = skipBalanced(source, open);
      out.push(...callLists(source.slice(open + 1, end - 1)));
      i = end;
      continue;
    }
    i++;
  }
  return out.filter((l) => l.split(" ").length > 1);
}
