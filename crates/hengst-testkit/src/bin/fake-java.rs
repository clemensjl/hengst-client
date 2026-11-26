//! Stands in for `java.exe` in tests of the game process (dev only). Started with the game's
//! arguments, it follows the script `fake-java.txt` in its working directory (the instance folder)
//! line by line; without a script it prints its arguments and exits with 0. It never shows a
//! window. Commands:
//!
//! - `args`: prints each argument as `ARG <argument>` on stdout.
//! - `variants`: prints the value after `--accessToken` in the shapes a log might hold it: inside a
//!   JSON string, percent-encoded in a URL, split between two writes, split over two lines, on
//!   stderr, and straddling the 8 KiB cut of a very long stderr line.
//! - `out <text>` / `err <text>`: one line on stdout / stderr.
//! - `xml <LEVEL>|<thread>|<message>`: one log4j XML event on stdout, as the game prints it.
//! - `sleep <ms>`.
//! - `wait <file>`: waits until `<file>` exists in the working directory, so a test ends the game
//!   when it wants to (and a guard can end it when the test panics).
//! - `window` / `window ignore-close`: creates a hidden top-level window (never shown) and prints
//!   `WINDOW READY`. A close request (`WM_SYSCOMMAND`/`SC_CLOSE`) prints `CLOSE REQUESTED` and
//!   closes it, after which the script goes on; with `ignore-close` it prints `CLOSE IGNORED` and
//!   keeps running until it is killed.
//! - `exit <code>`.
//!
//! Whatever the script does, fake-java exits with 124 after 120 s.

use std::io::Write;
use std::time::Duration;

/// fake-java ends itself after this long in every mode, so a test binary that was killed before
/// it could stop its game leaves no (invisible) window process behind for good.
const LIFETIME: Duration = Duration::from_secs(120);

fn main() {
    std::thread::spawn(|| {
        std::thread::sleep(LIFETIME);
        std::process::exit(124);
    });
    let args: Vec<String> = std::env::args().skip(1).collect();
    let script = std::fs::read_to_string("fake-java.txt").unwrap_or_else(|_| "args".into());
    for line in script.lines() {
        let (cmd, rest) = line.split_once(' ').unwrap_or((line, ""));
        match cmd {
            "args" => {
                for a in &args {
                    out(&format!("ARG {a}"));
                }
            }
            "variants" => variants(&args),
            "out" => out(rest),
            "err" => err(rest),
            "xml" => {
                let mut parts = rest.splitn(3, '|');
                let (level, thread, message) = (
                    parts.next().unwrap_or("INFO"),
                    parts.next().unwrap_or("main"),
                    parts.next().unwrap_or(""),
                );
                out(&format!(
                    "  <log4j:Event logger=\"fake\" timestamp=\"1795445575774\" level=\"{level}\" thread=\"{}\">\n    <log4j:Message><![CDATA[{message}]]></log4j:Message>\n  </log4j:Event>",
                    thread.replace('&', "&amp;").replace('"', "&quot;").replace('<', "&lt;")
                ));
            }
            "sleep" => std::thread::sleep(Duration::from_millis(rest.parse().unwrap_or(0))),
            "wait" => {
                while !std::path::Path::new(rest).exists() {
                    std::thread::sleep(Duration::from_millis(20));
                }
            }
            "window" => window(rest == "ignore-close"),
            "exit" => std::process::exit(rest.parse().unwrap_or(0)),
            "" => {}
            other => err(&format!("fake-java: unknown command {other:?}")),
        }
    }
}

fn out(text: &str) {
    let mut o = std::io::stdout().lock();
    let _ = writeln!(o, "{text}");
    let _ = o.flush();
}

fn err(text: &str) {
    let mut e = std::io::stderr().lock();
    let _ = writeln!(e, "{text}");
    let _ = e.flush();
}

fn variants(args: &[String]) {
    let Some(token) = args
        .iter()
        .position(|a| a == "--accessToken")
        .and_then(|i| args.get(i + 1))
    else {
        err("fake-java: no --accessToken");
        return;
    };
    let json: String = token
        .chars()
        .flat_map(|c| match c {
            '"' => vec!['\\', '"'],
            '\\' => vec!['\\', '\\'],
            c => vec![c],
        })
        .collect();
    out(&format!("{{\"accessToken\":\"{json}\",\"expires\":86400}}"));
    let pct: String = token
        .bytes()
        .map(|b| {
            if b.is_ascii_alphanumeric() || b"-._~".contains(&b) {
                (b as char).to_string()
            } else {
                format!("%{b:02X}")
            }
        })
        .collect();
    out(&format!(
        "GET https://example.invalid/session?token={pct}&x=1"
    ));
    let half = token.len() / 2;
    {
        let mut o = std::io::stdout().lock();
        let _ = write!(o, "split between writes {}", &token[..half]);
        let _ = o.flush();
    }
    std::thread::sleep(Duration::from_millis(100));
    out(&format!("{} end", &token[half..]));
    out(&format!("wrapped {}", &token[..half]));
    out(&format!("{} continues", &token[half..]));
    err(&format!("Exception: bad token {token}"));
    err(&format!(
        "{}{token}{}",
        "x".repeat(8 * 1024 - half),
        "y".repeat(20_000)
    ));
}

#[cfg(windows)]
fn window(ignore_close: bool) {
    use std::sync::atomic::{AtomicBool, Ordering};
    use windows_sys::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
    use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        CreateWindowExW, DefWindowProcW, DestroyWindow, DispatchMessageW, GetMessageW,
        PostQuitMessage, RegisterClassW, TranslateMessage, CW_USEDEFAULT, MSG, SC_CLOSE, WM_CLOSE,
        WM_DESTROY, WM_SYSCOMMAND, WNDCLASSW, WS_OVERLAPPEDWINDOW,
    };

    static IGNORE: AtomicBool = AtomicBool::new(false);

    unsafe extern "system" fn proc(h: HWND, msg: u32, w: WPARAM, l: LPARAM) -> LRESULT {
        let ignore = IGNORE.load(Ordering::SeqCst);
        match msg {
            WM_SYSCOMMAND if (w & 0xFFF0) == SC_CLOSE as usize => {
                out("CLOSE REQUESTED");
                if ignore {
                    out("CLOSE IGNORED");
                    return 0;
                }
                // SAFETY: forwarding the message this procedure received.
                unsafe { DefWindowProcW(h, msg, w, l) }
            }
            WM_CLOSE if ignore => 0,
            WM_CLOSE => {
                // SAFETY: `h` is this thread's own window.
                unsafe { DestroyWindow(h) };
                0
            }
            WM_DESTROY => {
                // SAFETY: plain call.
                unsafe { PostQuitMessage(0) };
                0
            }
            // SAFETY: forwarding the message this procedure received.
            _ => unsafe { DefWindowProcW(h, msg, w, l) },
        }
    }

    IGNORE.store(ignore_close, Ordering::SeqCst);
    let wide = |s: &str| s.encode_utf16().chain([0]).collect::<Vec<u16>>();
    let class = wide("HengstFakeJavaWindow");
    let title = wide("fake-java");
    // SAFETY: plain calls with valid, nul-terminated wide strings that outlive them. The window
    // is created without WS_VISIBLE and never shown.
    unsafe {
        let instance = GetModuleHandleW(std::ptr::null());
        let mut wc: WNDCLASSW = std::mem::zeroed();
        wc.lpfnWndProc = Some(proc);
        wc.hInstance = instance;
        wc.lpszClassName = class.as_ptr();
        RegisterClassW(&wc);
        let hwnd = CreateWindowExW(
            0,
            class.as_ptr(),
            title.as_ptr(),
            WS_OVERLAPPEDWINDOW,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            instance,
            std::ptr::null(),
        );
        if hwnd.is_null() {
            err("fake-java: no window");
            std::process::exit(9);
        }
        out("WINDOW READY");
        let mut msg: MSG = std::mem::zeroed();
        while GetMessageW(&mut msg, std::ptr::null_mut(), 0, 0) > 0 {
            TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
    }
}

#[cfg(not(windows))]
fn window(_: bool) {
    err("fake-java: windows only");
}
