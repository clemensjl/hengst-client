# Brand mark

`mark.svg` is a placeholder (a rounded square with an "H", see DESIGN.md section 14). The real logo replaces it.

To replace the mark:

1. Swap `brand/mark.svg` for the new file (keep it square).
2. Run `pnpm tauri icon brand/mark.svg` in `apps/desktop`. It rewrites `src-tauri/icons/`.
3. Delete the `android` and `ios` folders it adds there (the app is Windows only).

The frontend imports the mark only from `brand/mark.svg`. The window, taskbar and tray icons come from `src-tauri/icons/`.
