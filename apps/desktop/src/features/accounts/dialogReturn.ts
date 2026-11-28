/**
 * The chip (or the sign-in button, or the unavailable button) the dialogs of the accounts feature
 * return focus to when they close.
 */
export const chipRef: { current: HTMLElement | null } = { current: null };

// The element that opened the sign-in (or no-client-id) dialog, if it was not the chip.
let opener: HTMLElement | null = null;

/** Set on every opening of the sign-in path; `null` means the chip opened it. */
export function setDialogOpener(el: HTMLElement | null) {
  opener = el;
}

/**
 * Where the sign-in and no-client-id dialogs return focus (DESIGN.md 11.5): the Play button when
 * it opened them and is still there, else the chip.
 */
export const dialogReturnRef = {
  get current(): HTMLElement | null {
    return opener?.isConnected ? opener : chipRef.current;
  },
};
