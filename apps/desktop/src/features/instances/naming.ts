import type { LoaderInfoDto } from "@/ipc/bindings/LoaderInfoDto";

/**
 * The name the create dialog suggests: the game version, then the loader's display name. Loader
 * names come only from the backend (`LoaderInfoDto.displayName`).
 */
export function suggestName(game: string, loader: LoaderInfoDto | null): string {
  return loader ? `${game} ${loader.displayName}` : game;
}

/** The backend's limit for a name, in characters (Unicode scalar values). */
export const MAX_NAME = 64;

/**
 * What Rust's `str::trim` removes: Unicode White_Space. JavaScript's `trim` differs (it removes
 * U+FEFF and keeps U+0085), so the dialog would judge a name otherwise than the backend. Written
 * as numbers: some of these characters end a line in source text.
 */
const WHITE_SPACE = [
  ...[0x09, 0x0a, 0x0b, 0x0c, 0x0d, 0x20, 0x85, 0xa0, 0x1680],
  ...[0x2000, 0x2001, 0x2002, 0x2003, 0x2004, 0x2005, 0x2006, 0x2007, 0x2008, 0x2009, 0x200a],
  ...[0x2028, 0x2029, 0x202f, 0x205f, 0x3000],
]
  .map((c) => "\\u" + c.toString(16).padStart(4, "0"))
  .join("");
const EDGES = new RegExp(`^[${WHITE_SPACE}]+|[${WHITE_SPACE}]+$`, "g");

/** The name as the backend keeps it (`validate_name` trims it). */
export const trimName = (name: string): string => name.replace(EDGES, "");

/** The length the check counts and the counter shows: characters of the trimmed name. */
export const nameLength = (name: string): number => Array.from(trimName(name)).length;

/**
 * `validate_name` of the core, in its order: empty, longer than 64 characters, a control
 * character (Unicode category Cc). The i18n key of the problem, or `null`.
 */
export function nameProblem(name: string): string | null {
  const kept = trimName(name);
  if (kept === "") return "instances.form.nameEmpty";
  if (Array.from(kept).length > MAX_NAME) return "instances.form.nameTooLong";
  if (/\p{Cc}/u.test(kept)) return "instances.form.nameControl";
  return null;
}
