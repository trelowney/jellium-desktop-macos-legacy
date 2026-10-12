//! User-initiated self-update. The web UI checks GitHub Releases and, when the
//! user clicks "Update now", calls `applyUpdate(url, sizeBytes, versionTag)`.
//! We can't overwrite the running app in place, so we hand off to a helper
//! that waits for us to exit, replaces the install and relaunches us — then we
//! quit. On Windows that is the bundled `jellium-desktop-rtx-updater.exe`
//! side-car (release zip); on macOS it is `macos_update.sh` (release DMG).

/// Launch the update helper for `zip_url`, then begin app shutdown. Windows and
/// macOS; a no-op elsewhere.
pub(crate) fn apply_update(zip_url: &str, size: u64, version: &str) {
    #[cfg(target_os = "windows")]
    {
        match spawn_updater(zip_url, size, version) {
            Ok(()) => {
                jfn_logging::log(
                    jfn_logging::Category::Cef,
                    jfn_logging::Level::Info,
                    "Update: side-car launched; exiting to apply",
                );
                // GUARANTEE the process dies — this is the whole ballgame. The
                // app being replaced doesn't need a clean teardown, but it MUST
                // actually exit, or the side-car waits forever and the UI falls
                // back to "download manually" (the exact bug this file kept
                // failing to fix).
                //
                // Two rules learned the hard way:
                //  1. Arm the guaranteed kill BEFORE any graceful teardown, so
                //     nothing that runs in between can stop us dying.
                //  2. Kill with TerminateProcess(GetCurrentProcess()), NOT
                //     ExitProcess. ExitProcess still runs DLL_PROCESS_DETACH for
                //     every DLL under the loader lock and terminates other
                //     threads while they hold locks — with CEF's GPU/renderer
                //     processes and the NVIDIA driver that detach DEADLOCKS, so
                //     the process never actually exits. TerminateProcess is a
                //     kernel-level kill that runs zero user-mode cleanup and
                //     cannot deadlock — the same primitive Chromium uses for
                //     immediate shutdown.
                std::thread::spawn(|| {
                    use windows::Win32::System::Threading::{
                        ExitProcess, GetCurrentProcess, TerminateProcess,
                    };
                    // Short grace so the settings save worker (woken below) can
                    // flush; then hard-kill no matter what state teardown is in.
                    std::thread::sleep(std::time::Duration::from_millis(1500));
                    unsafe {
                        let _ = TerminateProcess(GetCurrentProcess(), 0);
                        // Only reached if TerminateProcess ever no-ops; still die.
                        ExitProcess(0);
                    }
                });
                // Best-effort graceful flush (settings save worker). Safe even
                // if it blocks or hangs — the watchdog above already owns our
                // death.
                jfn_playback::shutdown::jfn_shutdown_initiate();
            }
            Err(e) => {
                jfn_logging::log(
                    jfn_logging::Category::Cef,
                    jfn_logging::Level::Warn,
                    // Don't shut down — keep the working app running.
                    &format!("Update: failed to launch updater side-car: {e}"),
                );
            }
        }
    }
    #[cfg(target_os = "macos")]
    {
        // `zip_url` is the release DMG here. The script waits for us to exit
        // (and kills us after 30 s if we don't), so a plain shutdown is enough.
        match spawn_macos_updater(zip_url, size, version) {
            Ok(()) => {
                jfn_logging::log(
                    jfn_logging::Category::Cef,
                    jfn_logging::Level::Info,
                    "Update: update script started; quitting to apply",
                );
                jfn_playback::shutdown::jfn_shutdown_initiate();
            }
            Err(e) => {
                jfn_logging::log(
                    jfn_logging::Category::Cef,
                    jfn_logging::Level::Warn,
                    &format!("Update: could not start the update: {e}"),
                );
            }
        }
    }
    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    {
        let _ = (zip_url, size, version);
        jfn_logging::log(
            jfn_logging::Category::Cef,
            jfn_logging::Level::Warn,
            "Update: self-update is only supported on the Windows and macOS builds",
        );
    }
}

/// Download, swap and relaunch run in `macos_update.sh`, detached into its own
/// process group so it outlives the app.
#[cfg(target_os = "macos")]
fn spawn_macos_updater(dmg_url: &str, size: u64, version: &str) -> std::io::Result<()> {
    use std::io::Error;
    use std::os::unix::process::CommandExt;
    use std::process::{Command, Stdio};

    const SCRIPT: &str = include_str!("macos_update.sh");
    // The URL comes from page JavaScript. Only ever download a release of this
    // repository, so a page can't make us install something else.
    const RELEASES: &str =
        "https://github.com/trelowney/jellium-desktop-macos-legacy/releases/download/";

    if !dmg_url.starts_with(RELEASES) || !dmg_url.ends_with(".dmg") || dmg_url.contains("/../") {
        return Err(Error::other(format!("refusing update from {dmg_url}")));
    }
    // ...and only a release newer than this one, so it can't be downgraded.
    let tag = dmg_url[RELEASES.len()..]
        .split('/')
        .next()
        .unwrap_or_default();
    if tag != version || !is_newer_release(tag, crate::APP_RELEASE_TAG) {
        return Err(Error::other(format!(
            "refusing update to {tag} (offered as {version}, running {})",
            crate::APP_RELEASE_TAG
        )));
    }

    // .../Jellium Desktop.app/Contents/MacOS/jellium-desktop -> the .app
    let exe = std::env::current_exe()?;
    let app = exe
        .ancestors()
        .nth(3)
        .filter(|p| p.extension().is_some_and(|e| e == "app"))
        .ok_or_else(|| Error::other("not running from an app bundle"))?
        .to_path_buf();
    // Gatekeeper runs a quarantined app from a read-only random location
    // until it has been moved (e.g. into Applications).
    if app.to_string_lossy().contains("/AppTranslocation/") {
        return Err(Error::other(
            "app runs from a translocated copy; move it into Applications first",
        ));
    }
    // The bundle is replaced in place, so its folder must be writable.
    let dir = app
        .parent()
        .ok_or_else(|| Error::other("app has no parent folder"))?;
    let probe = dir.join(".jellium-update-probe");
    std::fs::write(&probe, b"")?;
    let _ = std::fs::remove_file(&probe);

    Command::new("/bin/bash")
        .arg("-c")
        .arg(SCRIPT)
        .arg("jellium-update")
        .arg(dmg_url)
        .arg(size.to_string())
        .arg(std::process::id().to_string())
        .arg(&app)
        .arg(version)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .process_group(0)
        .spawn()
        .map(|_| ())
}

#[cfg(target_os = "windows")]
fn spawn_updater(zip_url: &str, size: u64, version: &str) -> std::io::Result<()> {
    use std::io::{Error, ErrorKind};
    use std::os::windows::process::CommandExt;
    use std::process::Command;

    let exe = std::env::current_exe()?;
    let dir = exe
        .parent()
        .ok_or_else(|| Error::other("exe has no parent dir"))?
        .to_path_buf();
    let updater = dir.join("jellium-desktop-rtx-updater.exe");
    if !updater.exists() {
        return Err(Error::new(
            ErrorKind::NotFound,
            "updater side-car not found next to the app",
        ));
    }
    let pid = std::process::id();

    // The updater shows its own window, so we don't detach/hide it — but we do
    // ask it to break away from any job object the app lives in, so it outlives
    // our shutdown. Fall back to a plain spawn if the job forbids breakaway.
    const CREATE_BREAKAWAY_FROM_JOB: u32 = 0x0100_0000;
    let spawn = |flags: u32| {
        Command::new(&updater)
            .arg("--url")
            .arg(zip_url)
            .arg("--dir")
            .arg(&dir)
            .arg("--pid")
            .arg(pid.to_string())
            .arg("--size")
            .arg(size.to_string())
            .arg("--relaunch")
            .arg(&exe)
            .arg("--version")
            .arg(version)
            .creation_flags(flags)
            .spawn()
    };

    match spawn(CREATE_BREAKAWAY_FROM_JOB) {
        Ok(_) => Ok(()),
        Err(_) => spawn(0).map(|_| ()),
    }
}

/// `vYYYY.MM.DD[.N]` tags compared number by number (so `.10` follows `.9`).
/// A tag that doesn't parse is never newer.
#[cfg(any(target_os = "macos", test))]
fn is_newer_release(candidate: &str, current: &str) -> bool {
    fn parts(tag: &str) -> Option<Vec<u32>> {
        tag.strip_prefix('v')?
            .split('.')
            .map(|p| p.parse().ok())
            .collect()
    }
    match (parts(candidate), parts(current)) {
        (Some(c), Some(cur)) => c > cur,
        (Some(_), None) => current.is_empty(),
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::is_newer_release;

    #[test]
    fn release_tags_compare_by_number() {
        assert!(is_newer_release("v2026.10.13", "v2026.10.12"));
        assert!(is_newer_release("v2026.10.12.1", "v2026.10.12"));
        assert!(is_newer_release("v2026.10.12.10", "v2026.10.12.9"));
        assert!(!is_newer_release("v2026.10.12", "v2026.10.12"));
        assert!(!is_newer_release("v2026.09.30", "v2026.10.12"));
        assert!(!is_newer_release("v2026.10.13-rc1", "v2026.10.12"));
        assert!(!is_newer_release("latest", "v2026.10.12"));
        assert!(is_newer_release("v2026.10.12", ""));
    }
}
