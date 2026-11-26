/** The embedding, override and isolate controls (U+202A to U+202E, U+2066 to U+2069). */
const CONTROLS = /[‪-‮⁦-⁩]/g;

/**
 * A name (instance, account) for interpolation into a sentence: its bidi controls are dropped
 * and it is wrapped in a first-strong isolate (U+2068 .. U+2069), so it cannot reorder the text
 * around it. Without dropping them first, a U+2069 in the name would end the isolate early.
 */
export function isolate(name: string): string {
  return `⁨${name.replace(CONTROLS, "")}⁩`;
}
