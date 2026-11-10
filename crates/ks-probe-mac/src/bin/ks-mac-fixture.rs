//! AppKit test fixture for the M-2a probe. Not part of the product.
//!
//! Usage: `ks-mac-fixture --focus field|view|secure [--empty] [--long]`
//!        `ks-mac-fixture --hotkey-register <keycode> <carbon-mods>`  (prints status, keeps running)
//!        `ks-mac-fixture --hotkey-listen <keycode> <carbon-mods>`    (prints HOTKEY_FIRED on press)
//!
//! Window: NSTextField (synthetic text), NSTextView (multi-line, undo enabled),
//! NSSecureTextField (synthetic secret). An Edit menu provides Undo/Redo/Select All so
//! that Cmd+Z reaches `undo:`. Stdout carries only control lines (READY, status codes).

#[cfg(not(target_os = "macos"))]
fn main() {}

#[cfg(target_os = "macos")]
fn main() {
    mac::run();
}

#[cfg(target_os = "macos")]
#[allow(clippy::print_stdout)] // control protocol with the probe; never prints field text
mod mac {
    use std::io::Write as _;

    use ks_probe_mac::{carbon, synthetic};
    use objc2::rc::Retained;
    use objc2::{MainThreadMarker, MainThreadOnly, sel};
    use objc2_app_kit::{
        NSApplication, NSApplicationActivationPolicy, NSBackingStoreType, NSMenu, NSMenuItem,
        NSSecureTextField, NSTextField, NSTextView, NSWindow, NSWindowStyleMask,
    };
    use objc2_foundation::{NSPoint, NSRect, NSSize, NSString};

    fn say(line: &str) {
        let mut out = std::io::stdout().lock();
        let _ = writeln!(out, "{line}");
        let _ = out.flush();
    }

    extern "C" fn on_hotkey() {
        say("HOTKEY_FIRED");
    }

    fn rect(x: f64, y: f64, w: f64, h: f64) -> NSRect {
        NSRect::new(NSPoint::new(x, y), NSSize::new(w, h))
    }

    fn menu_item(
        mtm: MainThreadMarker,
        title: &str,
        action: objc2::runtime::Sel,
        key: &str,
    ) -> Retained<NSMenuItem> {
        // SAFETY: `action` is a standard responder-chain selector (undo:, redo:, selectAll:).
        unsafe {
            NSMenuItem::initWithTitle_action_keyEquivalent(
                NSMenuItem::alloc(mtm),
                &NSString::from_str(title),
                Some(action),
                &NSString::from_str(key),
            )
        }
    }

    fn build_menu(mtm: MainThreadMarker, app: &NSApplication) {
        let main = NSMenu::new(mtm);
        let app_item = NSMenuItem::new(mtm);
        let app_menu = NSMenu::new(mtm);
        app_menu.addItem(&menu_item(mtm, "Quit", sel!(terminate:), "q"));
        app_item.setSubmenu(Some(&app_menu));
        main.addItem(&app_item);

        let edit_item = NSMenuItem::new(mtm);
        let edit = NSMenu::new(mtm);
        edit.setTitle(&NSString::from_str("Edit"));
        edit.addItem(&menu_item(mtm, "Undo", sel!(undo:), "z"));
        edit.addItem(&menu_item(mtm, "Redo", sel!(redo:), "Z"));
        edit.addItem(&menu_item(mtm, "Select All", sel!(selectAll:), "a"));
        edit_item.setSubmenu(Some(&edit));
        main.addItem(&edit_item);
        app.setMainMenu(Some(&main));
    }

    pub fn run() {
        let args: Vec<String> = std::env::args().skip(1).collect();
        let Some(mtm) = MainThreadMarker::new() else {
            say("ERROR not-main-thread");
            return;
        };
        let app = NSApplication::sharedApplication(mtm);
        app.setActivationPolicy(NSApplicationActivationPolicy::Regular);

        if let Some(mode) = args.first().map(String::as_str)
            && (mode == "--hotkey-register" || mode == "--hotkey-listen")
        {
            let code: u32 = args.get(1).and_then(|s| s.parse().ok()).unwrap_or(0);
            let m: u32 = args.get(2).and_then(|s| s.parse().ok()).unwrap_or(0);
            if mode == "--hotkey-listen" {
                let st = carbon::install_hotkey_handler(on_hotkey);
                say(&format!("HANDLER_STATUS {st}"));
            }
            let hk = carbon::register_hotkey(code, m, 1);
            match &hk {
                Ok(_) => say("HOTKEY_STATUS 0"),
                Err(st) => say(&format!("HOTKEY_STATUS {st}")),
            }
            say("READY");
            app.run();
            drop(hk);
            return;
        }

        let focus = args
            .iter()
            .position(|a| a == "--focus")
            .and_then(|i| args.get(i + 1))
            .map(String::as_str)
            .unwrap_or("field");
        let empty = args.iter().any(|a| a == "--empty");
        let long = args.iter().any(|a| a == "--long");

        build_menu(mtm, &app);

        let style = NSWindowStyleMask::Titled | NSWindowStyleMask::Closable;
        // SAFETY: standard designated initializer; window released manually below.
        let window = unsafe {
            NSWindow::initWithContentRect_styleMask_backing_defer(
                NSWindow::alloc(mtm),
                rect(200.0, 200.0, 520.0, 320.0),
                style,
                NSBackingStoreType::Buffered,
                false,
            )
        };
        // SAFETY: we keep `window` alive for the whole process; avoids double release.
        unsafe { window.setReleasedWhenClosed(false) };
        window.setTitle(&NSString::from_str("KeySwitch M-2a fixture"));
        let Some(content) = window.contentView() else {
            say("ERROR no-content-view");
            return;
        };

        let field_text = if empty { "" } else { synthetic::FIELD_TEXT };
        let field = NSTextField::textFieldWithString(&NSString::from_str(field_text), mtm);
        field.setFrame(rect(20.0, 270.0, 480.0, 24.0));
        content.addSubview(&field);

        let secure = NSSecureTextField::initWithFrame(
            NSSecureTextField::alloc(mtm),
            rect(20.0, 235.0, 480.0, 24.0),
        );
        secure.setStringValue(&NSString::from_str(synthetic::SECURE_TEXT));
        content.addSubview(&secure);

        let view =
            NSTextView::initWithFrame(NSTextView::alloc(mtm), rect(20.0, 20.0, 480.0, 200.0));
        view.setRichText(false);
        view.setAllowsUndo(true);
        let view_text = if long {
            synthetic::long_text()
        } else if empty {
            String::new()
        } else {
            synthetic::VIEW_TEXT.to_owned()
        };
        view.setString(&NSString::from_str(&view_text));
        content.addSubview(&view);

        window.makeKeyAndOrderFront(None);
        let made = match focus {
            "view" => window.makeFirstResponder(Some(&view)),
            "secure" => window.makeFirstResponder(Some(&secure)),
            _ => window.makeFirstResponder(Some(&field)),
        };
        #[allow(deprecated)] // still the only non-cooperative path on macOS 14+; fact recorded
        app.activateIgnoringOtherApps(true);
        app.activate();
        say(&format!("FIRST_RESPONDER {}", made as u8));
        say("READY");
        app.run();
    }
}
