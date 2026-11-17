//! Keeping a venv's name in its LabVIEW's window titles.
//!
//! A venv LabVIEW is the same exe as the user's own and its window titles read the
//! same, so `lvpm launch` starts this watcher beside it, hidden, and it tags
//! every window title with the venv's name until that LabVIEW exits.
//!
//! What it relies on, measured on LabVIEW 2026 (Windows):
//!
//! - LabVIEW's own windows (Getting Started, Project Explorer, front panels,
//!   block diagrams, class and library windows) are class `LVDChild` with
//!   `WS_EX_APPWINDOW`; its dialogs and floating palettes lack that bit. The
//!   `LVFrame` window owns them all, and its title is the one Alt+Tab shows.
//!   Add-ons keep hidden app windows of their own: only visible ones are
//!   touched, so a tool that finds its own window by title keeps finding it.
//! - LabVIEW rebuilds a title from its own state on every edit, save or
//!   rename, which drops the tag, and never doubles it: putting the tag back
//!   after each change is all it takes.
//! - Where the tag goes matters. In front, or at the end but before LabVIEW's
//!   ` *` for unsaved changes, the `*` keeps working. Written after the `*`,
//!   LabVIEW lost track of its dirty marker and fell back to short titles.
//! - Reading another process's titles does not involve that process; only a
//!   rename reaches LabVIEW's UI thread, one per window per change.

use crate::config::TitlePosition;

/// The title `title` should have, tagged with `tag` at `pos`, or `None` when
/// it already has it. The tag is first removed from wherever an earlier
/// position left it, so changing the setting moves it.
///
/// `None` for an unchanged title is what ends the loop of rename, title
/// change event, rename.
pub fn retitle(title: &str, tag: &str, pos: TitlePosition) -> Option<String> {
    if title.is_empty() || tag.is_empty() || pos == TitlePosition::Off {
        return None;
    }
    let base = untagged(title, tag);
    if base.is_empty() {
        return None;
    }
    let want = match pos {
        TitlePosition::Start => format!("{tag} {base}"),
        TitlePosition::End => match base.strip_suffix(" *") {
            Some(clean) => format!("{clean} {tag} *"),
            None => format!("{base} {tag}"),
        },
        TitlePosition::Off => return None,
    };
    (want != title).then_some(want)
}

/// `title` without the tag in any of the places `retitle` puts it — in
/// front, at the end, or before a closing ` *` — and nowhere else, so a VI
/// whose own name contains the tag's text keeps it.
fn untagged(title: &str, tag: &str) -> String {
    let mut t = title;
    let front = format!("{tag} ");
    while let Some(rest) = t.strip_prefix(front.as_str()) {
        t = rest;
    }
    let mut t = t.to_string();
    let end = format!(" {tag}");
    let before_star = format!(" {tag} *");
    loop {
        if let Some(rest) = t.strip_suffix(before_star.as_str()) {
            t = format!("{rest} *");
        } else if let Some(rest) = t.strip_suffix(end.as_str()) {
            t = rest.to_string();
        } else {
            break;
        }
    }
    if t == tag { String::new() } else { t }
}

/// Tag the windows of the LabVIEW with process id `pid` until it exits.
#[cfg(windows)]
pub fn watch(pid: u32, tag: &str, pos: TitlePosition) -> anyhow::Result<()> {
    win::watch(pid, tag, pos)
}

#[cfg(windows)]
mod win {
    use super::retitle;
    use crate::config::TitlePosition;
    use anyhow::{Result, bail};
    use std::cell::RefCell;
    use std::ffi::c_void;

    type Hwnd = *mut c_void;
    type Handle = *mut c_void;
    type Hook = *mut c_void;
    type WinEventProc = unsafe extern "system" fn(Hook, u32, Hwnd, i32, i32, u32, u32);
    type EnumProc = unsafe extern "system" fn(Hwnd, isize) -> i32;

    #[repr(C)]
    struct Msg {
        hwnd: Hwnd,
        message: u32,
        wparam: usize,
        lparam: isize,
        time: u32,
        pt_x: i32,
        pt_y: i32,
        private: u32,
    }

    #[link(name = "user32")]
    unsafe extern "system" {
        fn SetWinEventHook(min: u32, max: u32, module: *mut c_void, proc_: WinEventProc, pid: u32, tid: u32, flags: u32) -> Hook;
        fn UnhookWinEvent(hook: Hook) -> i32;
        fn EnumWindows(proc_: EnumProc, lparam: isize) -> i32;
        fn GetWindowThreadProcessId(hwnd: Hwnd, pid: *mut u32) -> u32;
        fn IsWindowVisible(hwnd: Hwnd) -> i32;
        fn GetClassNameW(hwnd: Hwnd, buf: *mut u16, len: i32) -> i32;
        fn GetWindowTextW(hwnd: Hwnd, buf: *mut u16, len: i32) -> i32;
        fn GetWindowTextLengthW(hwnd: Hwnd) -> i32;
        fn GetWindowLongW(hwnd: Hwnd, index: i32) -> i32;
        fn SendMessageTimeoutW(hwnd: Hwnd, msg: u32, wparam: usize, lparam: isize, flags: u32, ms: u32, result: *mut usize) -> isize;
        fn MsgWaitForMultipleObjects(count: u32, handles: *const Handle, wait_all: i32, ms: u32, wake: u32) -> u32;
        fn PeekMessageW(msg: *mut Msg, hwnd: Hwnd, min: u32, max: u32, remove: u32) -> i32;
        fn TranslateMessage(msg: *const Msg) -> i32;
        fn DispatchMessageW(msg: *const Msg) -> isize;
    }

    unsafe extern "system" {
        fn OpenProcess(access: u32, inherit: i32, pid: u32) -> Handle;
        fn CloseHandle(handle: Handle) -> i32;
    }

    const EVENT_OBJECT_SHOW: u32 = 0x8002;
    const EVENT_OBJECT_NAMECHANGE: u32 = 0x800C;
    const WINEVENT_OUTOFCONTEXT: u32 = 0;
    const OBJID_WINDOW: i32 = 0;
    const CHILDID_SELF: i32 = 0;
    const GWL_EXSTYLE: i32 = -20;
    const WS_EX_APPWINDOW: u32 = 0x0004_0000;
    const WM_SETTEXT: u32 = 0x000C;
    const SMTO_ABORTIFHUNG: u32 = 0x0002;
    const SET_TIMEOUT_MS: u32 = 500;
    const SYNCHRONIZE: u32 = 0x0010_0000;
    const PROCESS_QUERY_LIMITED_INFORMATION: u32 = 0x1000;
    const QS_ALLINPUT: u32 = 0x04FF;
    const WAIT_OBJECT_0: u32 = 0;
    const WAIT_FAILED: u32 = 0xFFFF_FFFF;
    const INFINITE: u32 = 0xFFFF_FFFF;
    const PM_REMOVE: u32 = 0x0001;

    /// What the callbacks need. Out-of-context hooks call back on the thread
    /// that set them, while it pumps messages, so a thread-local is enough.
    struct State {
        pid: u32,
        tag: String,
        pos: TitlePosition,
    }

    thread_local! {
        static STATE: RefCell<Option<State>> = const { RefCell::new(None) };
    }

    pub fn watch(pid: u32, tag: &str, pos: TitlePosition) -> Result<()> {
        if pos == TitlePosition::Off {
            return Ok(());
        }
        let Some(process) = Process::open(pid) else { bail!("no process {pid} to watch") };
        STATE.with(|s| *s.borrow_mut() = Some(State { pid, tag: tag.to_string(), pos }));
        let (Some(_shown), Some(_renamed)) = (EventHook::new(EVENT_OBJECT_SHOW, pid), EventHook::new(EVENT_OBJECT_NAMECHANGE, pid))
        else {
            bail!("could not hook window events of process {pid}");
        };
        log::debug!("watching the window titles of LabVIEW {pid}, tag {tag:?} at the {}", pos.as_str());
        // Windows from before the hooks; later ones arrive as events.
        enum_windows();
        // The hooks call back from inside `pump`; it returns when LabVIEW exits.
        process.pump_until_exit();
        log::debug!("LabVIEW {pid} exited; title watcher done");
        Ok(())
    }

    /// The watched LabVIEW's process, closed when dropped.
    struct Process(Handle);

    impl Process {
        fn open(pid: u32) -> Option<Process> {
            // SAFETY: no pointers in; the handle is checked and owned from here.
            let h = unsafe { OpenProcess(SYNCHRONIZE | PROCESS_QUERY_LIMITED_INFORMATION, 0, pid) };
            (!h.is_null()).then_some(Process(h))
        }

        /// Dispatch this thread's messages (and with them the hook
        /// callbacks) until the process exits.
        fn pump_until_exit(&self) {
            // SAFETY: an all-zero MSG is a valid value of a plain C struct.
            let mut msg: Msg = unsafe { std::mem::zeroed() };
            loop {
                // SAFETY: one handle, owned by `self`, passed with count 1.
                let woke = unsafe { MsgWaitForMultipleObjects(1, &self.0, 0, INFINITE, QS_ALLINPUT) };
                if woke == WAIT_OBJECT_0 || woke == WAIT_FAILED {
                    return;
                }
                // SAFETY: `msg` is a valid, writable MSG for each call.
                while unsafe { PeekMessageW(&mut msg, std::ptr::null_mut(), 0, 0, PM_REMOVE) } != 0 {
                    unsafe {
                        TranslateMessage(&msg);
                        DispatchMessageW(&msg);
                    }
                }
            }
        }
    }

    impl Drop for Process {
        fn drop(&mut self) {
            // SAFETY: a handle this value owns, closed once.
            unsafe { CloseHandle(self.0) };
        }
    }

    /// An out-of-context hook for one event of one process, removed when dropped.
    struct EventHook(Hook);

    impl EventHook {
        fn new(event: u32, pid: u32) -> Option<EventHook> {
            // SAFETY: `on_event` matches WINEVENTPROC; no module for an
            // out-of-context hook.
            let h = unsafe { SetWinEventHook(event, event, std::ptr::null_mut(), on_event, pid, 0, WINEVENT_OUTOFCONTEXT) };
            (!h.is_null()).then_some(EventHook(h))
        }
    }

    impl Drop for EventHook {
        fn drop(&mut self) {
            // SAFETY: a hook this value owns, removed once.
            unsafe { UnhookWinEvent(self.0) };
        }
    }

    fn enum_windows() {
        // SAFETY: `on_window` matches WNDENUMPROC and uses no lparam.
        unsafe { EnumWindows(on_window, 0) };
    }

    unsafe extern "system" fn on_event(_: Hook, _event: u32, hwnd: Hwnd, object: i32, child: i32, _: u32, _: u32) {
        if object == OBJID_WINDOW && child == CHILDID_SELF && !hwnd.is_null() {
            tag_window(hwnd);
        }
    }

    unsafe extern "system" fn on_window(hwnd: Hwnd, _: isize) -> i32 {
        tag_window(hwnd);
        1
    }

    /// Tag `hwnd` if it is one of the watched LabVIEW's own visible windows.
    fn tag_window(hwnd: Hwnd) {
        STATE.with(|s| {
            let s = s.borrow();
            let Some(state) = s.as_ref() else { return };
            if window_pid(hwnd) != state.pid || !is_visible(hwnd) {
                return;
            }
            let class = class_name(hwnd);
            let app = ex_style(hwnd) & WS_EX_APPWINDOW != 0;
            if !(class == "LVFrame" || (class == "LVDChild" && app)) {
                return;
            }
            let title = window_text(hwnd);
            let Some(want) = retitle(&title, &state.tag, state.pos) else { return };
            if !set_window_text(hwnd, &want) {
                // Busy or hung: the next title change brings it back here.
                log::debug!("LabVIEW did not take a new title within {SET_TIMEOUT_MS} ms: {title:?}");
            }
        });
    }

    // Window queries. A window handle is an opaque value that Windows checks
    // on every call: a stale or foreign one makes the call fail, never touch
    // memory, so these are safe to call with any `hwnd`.

    fn window_pid(hwnd: Hwnd) -> u32 {
        let mut pid = 0u32;
        // SAFETY: `pid` is a valid u32 to write to.
        unsafe { GetWindowThreadProcessId(hwnd, &mut pid) };
        pid
    }

    fn is_visible(hwnd: Hwnd) -> bool {
        // SAFETY: no pointers.
        unsafe { IsWindowVisible(hwnd) != 0 }
    }

    fn ex_style(hwnd: Hwnd) -> u32 {
        // SAFETY: no pointers.
        unsafe { GetWindowLongW(hwnd, GWL_EXSTYLE) as u32 }
    }

    fn class_name(hwnd: Hwnd) -> String {
        let mut buf = [0u16; 64];
        // SAFETY: the buffer's length travels with it; the result is at most that.
        let n = unsafe { GetClassNameW(hwnd, buf.as_mut_ptr(), buf.len() as i32) };
        String::from_utf16_lossy(&buf[..n.clamp(0, buf.len() as i32) as usize])
    }

    /// The whole title, however long: a cut-off read would hide a tag at the
    /// end, and retitling that would cut LabVIEW's title and loop.
    fn window_text(hwnd: Hwnd) -> String {
        // SAFETY: no pointers.
        let len = unsafe { GetWindowTextLengthW(hwnd) }.max(0) as usize;
        let mut buf = vec![0u16; len + 1];
        // SAFETY: as in `class_name`.
        let n = unsafe { GetWindowTextW(hwnd, buf.as_mut_ptr(), buf.len() as i32) };
        String::from_utf16_lossy(&buf[..n.clamp(0, buf.len() as i32) as usize])
    }

    /// Retitle `hwnd`, waiting at most `SET_TIMEOUT_MS` for its owner, so a
    /// busy LabVIEW never blocks the watcher. False when it did not answer.
    fn set_window_text(hwnd: Hwnd, text: &str) -> bool {
        let wide: Vec<u16> = text.encode_utf16().chain(std::iter::once(0)).collect();
        let mut result = 0usize;
        // SAFETY: `wide` is NUL-terminated and outlives the call, which
        // returns before the timeout ends; `result` is a valid usize.
        let ok = unsafe {
            SendMessageTimeoutW(hwnd, WM_SETTEXT, 0, wide.as_ptr() as isize, SMTO_ABORTIFHUNG, SET_TIMEOUT_MS, &mut result)
        };
        ok != 0
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use TitlePosition::{End, Off, Start};

    const TAG: &str = "[venv-test4]";

    #[test]
    fn end_goes_after_the_title_and_before_labviews_star() {
        assert_eq!(
            retitle("x.vi Front Panel on p.lvproj/My Computer", TAG, End).as_deref(),
            Some("x.vi Front Panel on p.lvproj/My Computer [venv-test4]")
        );
        assert_eq!(
            retitle("x.vi Front Panel on p.lvproj/My Computer *", TAG, End).as_deref(),
            Some("x.vi Front Panel on p.lvproj/My Computer [venv-test4] *")
        );
        // The project's `*` sits mid-title: nothing special about the end.
        assert_eq!(
            retitle("p.lvproj * - Project Explorer", TAG, End).as_deref(),
            Some("p.lvproj * - Project Explorer [venv-test4]")
        );
    }

    #[test]
    fn start_goes_in_front() {
        assert_eq!(retitle("LabVIEW", TAG, Start).as_deref(), Some("[venv-test4] LabVIEW"));
        assert_eq!(retitle("x.vi Block Diagram *", TAG, Start).as_deref(), Some("[venv-test4] x.vi Block Diagram *"));
    }

    #[test]
    fn a_tagged_title_is_left_alone_which_ends_the_rename_loop() {
        assert_eq!(retitle("LabVIEW [venv-test4]", TAG, End), None);
        assert_eq!(retitle("x.vi [venv-test4] *", TAG, End), None);
        assert_eq!(retitle("[venv-test4] LabVIEW", TAG, Start), None);
    }

    #[test]
    fn a_tag_in_the_wrong_place_moves() {
        assert_eq!(retitle("[venv-test4] x.vi *", TAG, End).as_deref(), Some("x.vi [venv-test4] *"));
        assert_eq!(retitle("x.vi [venv-test4] *", TAG, Start).as_deref(), Some("[venv-test4] x.vi *"));
        // The form that breaks LabVIEW's `*` is repaired, not kept.
        assert_eq!(retitle("x.vi * [venv-test4]", TAG, End).as_deref(), Some("x.vi [venv-test4] *"));
    }

    #[test]
    fn the_tags_text_inside_labviews_own_title_is_kept() {
        assert_eq!(
            retitle("[venv-test4] notes.vi Front Panel", TAG, End).as_deref(),
            Some("notes.vi Front Panel [venv-test4]"),
            "a leading tag is taken for ours, the rest stays"
        );
        assert_eq!(
            retitle("a [venv-test4] b.vi Front Panel *", TAG, End).as_deref(),
            Some("a [venv-test4] b.vi Front Panel [venv-test4] *")
        );
        assert_eq!(retitle("a [venv-test4] b.vi [venv-test4]", TAG, End), None);
    }

    #[test]
    fn long_titles_are_tagged_whole() {
        let long = format!("{}.vi Front Panel on p.lvproj/My Computer *", "x".repeat(2000));
        let want = retitle(&long, TAG, End).unwrap();
        assert!(want.ends_with(" [venv-test4] *") && want.len() == long.len() + TAG.len() + 1);
        assert_eq!(retitle(&want, TAG, End), None);
    }

    #[test]
    fn nothing_to_do_for_empty_titles_or_off() {
        assert_eq!(retitle("", TAG, End), None);
        assert_eq!(retitle(TAG, TAG, End), None);
        assert_eq!(retitle("LabVIEW", TAG, Off), None);
    }
}
