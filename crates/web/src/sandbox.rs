//! Chromium's sandbox on Linux. Chromium confines its renderers either with
//! unprivileged user namespaces (plus seccomp-bpf) or, where a kernel or
//! AppArmor forbids those, with a setuid-root `chrome-sandbox` helper beside
//! the executable. When it can use neither it aborts ("No usable sandbox!"),
//! so we look first and run without the sandbox, loudly, rather than not at all.
//! `TRAILBOARD_NO_SANDBOX=1` turns it off on purpose (for debugging).

use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};

/// How Chromium's child processes are confined.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Sandbox {
    /// User, PID and network namespaces, the way Chromium prefers.
    Namespaces,
    /// The setuid-root helper at this path.
    Setuid(PathBuf),
    /// No sandbox.
    Off(Why),
}

/// Why the sandbox is off.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Why {
    /// `TRAILBOARD_NO_SANDBOX` asked for it.
    OptOut,
    /// No user namespaces, and no setuid helper (missing, or not root-owned 4755).
    Unavailable,
}

impl Sandbox {
    pub fn is_on(&self) -> bool {
        !matches!(self, Sandbox::Off(_))
    }

    /// One line for the log.
    pub fn describe(&self) -> String {
        match self {
            Sandbox::Namespaces => "sandbox: on (user namespaces)".into(),
            Sandbox::Setuid(p) => format!("sandbox: on (setuid helper {})", p.display()),
            Sandbox::Off(Why::OptOut) => "sandbox: off (TRAILBOARD_NO_SANDBOX)".into(),
            Sandbox::Off(Why::Unavailable) => "sandbox: OFF, pages run unconfined: this system allows no \
                 unprivileged user namespaces and there is no root-owned, setuid `chrome-sandbox` \
                 beside the executable"
                .into(),
        }
    }

    /// Its value in the settings panel.
    pub fn summary(&self) -> &'static str {
        match self {
            Sandbox::Namespaces => "On · user namespaces",
            Sandbox::Setuid(_) => "On · setuid helper",
            Sandbox::Off(Why::OptOut) => "Off · TRAILBOARD_NO_SANDBOX",
            Sandbox::Off(Why::Unavailable) => "Off · not supported here",
        }
    }

    /// What it means, shown when its row in the settings panel is picked.
    pub fn explain(&self) -> &'static str {
        match self {
            Sandbox::Namespaces => "Pages run in Chromium's sandbox, confined with user namespaces and seccomp",
            Sandbox::Setuid(_) => "Pages run in Chromium's sandbox, through the setuid chrome-sandbox helper",
            Sandbox::Off(Why::OptOut) => "Sandbox off for this run because TRAILBOARD_NO_SANDBOX is set",
            Sandbox::Off(Why::Unavailable) => {
                "Off: no unprivileged user namespaces here, and chrome-sandbox isn't setuid root · see the README"
            }
        }
    }

    /// The sandbox is off because the system can't run it (not by choice):
    /// worth telling the user about once.
    pub fn unsupported(&self) -> bool {
        *self == Sandbox::Off(Why::Unavailable)
    }
}

/// What a `chrome-sandbox` file looks like, as far as Chromium cares.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Helper {
    pub owner_root: bool,
    pub setuid: bool,
    pub executable: bool,
}

impl Helper {
    fn usable(self) -> bool {
        self.owner_root && self.setuid && self.executable
    }
}

/// Pure choice, in Chromium's own order: namespaces first, then the helper.
pub fn choose(opt_out: bool, namespaces: bool, helper: Option<(PathBuf, Helper)>) -> Sandbox {
    if opt_out {
        return Sandbox::Off(Why::OptOut);
    }
    if namespaces {
        return Sandbox::Namespaces;
    }
    match helper {
        Some((path, h)) if h.usable() => Sandbox::Setuid(path),
        _ => Sandbox::Off(Why::Unavailable),
    }
}

/// `TRAILBOARD_NO_SANDBOX` set to anything but empty or `0`.
pub fn opted_out(value: Option<&str>) -> bool {
    value.is_some_and(|v| !v.is_empty() && v != "0")
}

/// Looks at this system and picks. `exe_dir` is where Chromium looks for
/// `chrome-sandbox` (the browser executable's directory).
pub fn detect(exe_dir: &Path) -> Sandbox {
    let opt_out = opted_out(std::env::var("TRAILBOARD_NO_SANDBOX").ok().as_deref());
    if opt_out {
        return choose(true, false, None);
    }
    let path = exe_dir.join("chrome-sandbox");
    let helper = std::fs::metadata(&path).ok().map(|m| {
        let mode = m.mode();
        (path, Helper { owner_root: m.uid() == 0, setuid: mode & 0o4000 != 0, executable: mode & 0o111 != 0 })
    });
    choose(false, namespaces_work(), helper)
}

/// Chromium's own test: can a child make new user, PID and network namespaces?
/// The PID and network ones need capabilities inside the new user namespace,
/// so this also fails where AppArmor (Ubuntu 24.04+) strips them.
fn namespaces_work() -> bool {
    // SAFETY: the child only calls the async-signal-safe `unshare` and `_exit`.
    unsafe {
        match libc::fork() {
            -1 => false,
            0 => {
                let ok = libc::unshare(libc::CLONE_NEWUSER | libc::CLONE_NEWPID | libc::CLONE_NEWNET) == 0;
                libc::_exit(if ok { 0 } else { 1 })
            }
            pid => {
                let mut status = 0;
                loop {
                    let r = libc::waitpid(pid, &mut status, 0);
                    if r == pid {
                        break;
                    }
                    if r == -1 && *libc::__errno_location() != libc::EINTR {
                        return false;
                    }
                }
                libc::WIFEXITED(status) && libc::WEXITSTATUS(status) == 0
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const GOOD: Helper = Helper { owner_root: true, setuid: true, executable: true };

    fn helper(h: Helper) -> Option<(PathBuf, Helper)> {
        Some((PathBuf::from("/usr/lib/trailboard/chrome-sandbox"), h))
    }

    #[test]
    fn namespaces_preferred_over_helper() {
        assert_eq!(choose(false, true, helper(GOOD)), Sandbox::Namespaces);
        assert_eq!(choose(false, true, None), Sandbox::Namespaces);
    }

    #[test]
    fn helper_when_no_namespaces() {
        assert_eq!(
            choose(false, false, helper(GOOD)),
            Sandbox::Setuid(PathBuf::from("/usr/lib/trailboard/chrome-sandbox"))
        );
    }

    #[test]
    fn unusable_helper_means_off() {
        for h in [
            Helper { owner_root: false, ..GOOD },
            Helper { setuid: false, ..GOOD },
            Helper { executable: false, ..GOOD },
        ] {
            assert_eq!(choose(false, false, helper(h)), Sandbox::Off(Why::Unavailable));
        }
        assert_eq!(choose(false, false, None), Sandbox::Off(Why::Unavailable));
    }

    #[test]
    fn opt_out_wins() {
        assert_eq!(choose(true, true, helper(GOOD)), Sandbox::Off(Why::OptOut));
        assert!(!Sandbox::Off(Why::OptOut).is_on());
        assert!(Sandbox::Namespaces.is_on());
    }

    #[test]
    fn only_unavailable_is_unsupported() {
        assert!(Sandbox::Off(Why::Unavailable).unsupported());
        assert!(!Sandbox::Off(Why::OptOut).unsupported());
        assert!(!Sandbox::Namespaces.unsupported());
        assert!(!Sandbox::Setuid(PathBuf::from("/x")).unsupported());
        assert!(Sandbox::Off(Why::Unavailable).summary().starts_with("Off"));
        assert!(Sandbox::Setuid(PathBuf::from("/x")).summary().starts_with("On"));
    }

    #[test]
    fn opt_out_values() {
        assert!(!opted_out(None));
        assert!(!opted_out(Some("")));
        assert!(!opted_out(Some("0")));
        assert!(opted_out(Some("1")));
        assert!(opted_out(Some("yes")));
    }

    #[test]
    fn probe_runs() {
        // Whatever this machine allows, probing must return and not hang.
        let _ = namespaces_work();
    }
}
