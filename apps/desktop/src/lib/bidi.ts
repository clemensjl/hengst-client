/** The embedding, override and isolate controls (U+202A to U+202E, U+2066 to U+2069). */
const CONTROLS = /[\u202A-\u202E\u2066-\u2069]/g;

/** The text without bidi embedding, override and isolate controls (paths shown as they are). */
export function stripBidi(text: string): string {
  return text.replace(CONTROLS, "");
}

/**
 * A name (instance, account) for interpolation into a sentence: its bidi controls are dropped
 * and it is wrapped in a first-strong isolate (U+2068 .. U+2069), so it cannot reorder the text
 * around it. Without dropping them first, a U+2069 in the name would end the isolate early.
 */
export function isolate(name: string): string {
  return `\u2068${name.replace(CONTROLS, "")}\u2069`;
}
