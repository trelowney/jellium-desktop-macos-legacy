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
        // `zip_url` is the release DMG here. Download and copy happen while we
        // keep running and show progress; we only quit for the final swap.
        if let Err(e) = start_macos_update(zip_url, size, version) {
            jfn_logging::log(
                jfn_logging::Category::Cef,
                jfn_logging::Level::Warn,
                &format!("Update: could not start the update: {e}"),
            );
            report_progress("error", 0, 0);
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

/// Tell the page's update dialog how far we are: `phase` is "download"
/// (`done` of `total` bytes), "install", "restart" or "error".
#[cfg(target_os = "macos")]
fn report_progress(phase: &str, done: u64, total: u64) {
    jfn_playback::exec_js::jfn_playback_exec_web_js(&format!(
        "window.__jfnUpdateProgress && window.__jfnUpdateProgress('{phase}', {done}, {total});"
    ));
}

/// The update runs `macos_update.sh` twice: `prepare` (download + copy the
/// new app next to the old one) while we keep running and report progress,
/// then `finish` (swap + relaunch), detached into its own process group so it
/// outlives us, right before we quit.
#[cfg(target_os = "macos")]
fn start_macos_update(dmg_url: &str, size: u64, version: &str) -> std::io::Result<()> {
    use std::io::Error;
    use std::os::unix::process::CommandExt;
    use std::process::{Command, Stdio};
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::time::Duration;

    const SCRIPT: &str = include_str!("macos_update.sh");
    check_update_url(dmg_url, version, MACOS_RELEASES, ".dmg")?;

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

    // One update at a time (the startup check and a manual one can both
    // offer it). A second request is ignored, not reported as an error: that
    // would close the progress dialog of the update that is running.
    static RUNNING: AtomicBool = AtomicBool::new(false);
    if RUNNING.swap(true, Ordering::SeqCst) {
        jfn_logging::log(
            jfn_logging::Category::Cef,
            jfn_logging::Level::Info,
            "Update: already running; ignoring a second request",
        );
        return Ok(());
    }

    // $TMPDIR on macOS is per user (mode 700), so nobody else can plant this.
    let work = std::env::temp_dir().join(format!("jellium-update-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&work);
    if let Err(e) = std::fs::create_dir(&work) {
        RUNNING.store(false, Ordering::SeqCst);
        return Err(e);
    }
    let script = |args: &[&std::ffi::OsStr]| {
        let mut cmd = Command::new("/bin/bash");
        cmd.arg("-c")
            .arg(SCRIPT)
            .arg("jellium-update")
            .args(args)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null());
        cmd
    };
    let mut prepare = script(&[
        "prepare".as_ref(),
        dmg_url.as_ref(),
        size.to_string().as_ref(),
        app.as_os_str(),
        work.as_os_str(),
    ])
    .spawn()
    .inspect_err(|_| RUNNING.store(false, Ordering::SeqCst))?;

    std::thread::spawn(move || {
        let dmg = work.join("update.dmg");
        // The script deletes the DMG once copied, so remember we got past it.
        let mut downloaded = false;
        let status = loop {
            match prepare.try_wait() {
                Ok(Some(status)) => break Some(status),
                Ok(None) => {}
                Err(_) => break None,
            }
            let got = std::fs::metadata(&dmg).map_or(0, |m| m.len());
            downloaded |= size > 0 && got >= size;
            if downloaded {
                report_progress("install", 0, 0);
            } else {
                report_progress("download", got, size);
            }
            std::thread::sleep(Duration::from_millis(250));
        };
        if !status.is_some_and(|s| s.success()) {
            jfn_logging::log(
                jfn_logging::Category::Cef,
                jfn_logging::Level::Warn,
                "Update: download or copy failed; see ~/Library/Logs/Jellium Desktop/update.log",
            );
            RUNNING.store(false, Ordering::SeqCst);
            report_progress("error", 0, 0);
            return;
        }
        report_progress("restart", 0, 0);
        let pid = std::process::id().to_string();
        match script(&[
            "finish".as_ref(),
            pid.as_ref(),
            app.as_os_str(),
            work.as_os_str(),
        ])
        .process_group(0)
        .spawn()
        {
            Ok(_) => {
                jfn_logging::log(
                    jfn_logging::Category::Cef,
                    jfn_logging::Level::Info,
                    "Update: new version ready; quitting to swap it in",
                );
                // The script waits for us to exit (and kills us after 30 s if
                // we don't), so a plain shutdown is enough.
                jfn_playback::shutdown::jfn_shutdown_initiate();
            }
            Err(e) => {
                jfn_logging::log(
                    jfn_logging::Category::Cef,
                    jfn_logging::Level::Warn,
                    &format!("Update: could not start the final step: {e}"),
                );
                let _ = std::fs::remove_dir_all(app.with_extension("app.new"));
                RUNNING.store(false, Ordering::SeqCst);
                report_progress("error", 0, 0);
            }
        }
    });
    Ok(())
}

#[cfg(target_os = "windows")]
fn spawn_updater(zip_url: &str, size: u64, version: &str) -> std::io::Result<()> {
    use std::io::{Error, ErrorKind};
    use std::os::windows::process::CommandExt;
    use std::process::Command;

    check_update_url(zip_url, version, WINDOWS_RELEASES, ".zip")?;

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

/// Where each build's releases are downloaded from.
#[cfg(target_os = "windows")]
const WINDOWS_RELEASES: &str =
    "https://github.com/trelowney/jellium-desktop-rtx/releases/download/";
#[cfg(target_os = "macos")]
const MACOS_RELEASES: &str =
    "https://github.com/trelowney/jellium-desktop-macos-legacy/releases/download/";

/// The URL comes from page JavaScript, i.e. from whatever the server serves.
/// Only ever install an asset of this build's own releases, and only a release
/// newer than the running one, so a page can neither make us install something
/// else nor downgrade us.
#[cfg(any(target_os = "windows", target_os = "macos"))]
fn check_update_url(url: &str, version: &str, releases: &str, ext: &str) -> std::io::Result<()> {
    use std::io::Error;
    let tag = release_asset_tag(url, releases, ext)
        .ok_or_else(|| Error::other(format!("refusing update from {url}")))?;
    if tag != version || !is_newer_release(tag, crate::APP_RELEASE_TAG) {
        return Err(Error::other(format!(
            "refusing update to {tag} (offered as {version}, running {})",
            crate::APP_RELEASE_TAG
        )));
    }
    Ok(())
}

/// The tag of a release download URL of exactly the form
/// `<releases><tag>/<name><ext>`, where the tag and file name use only plain
/// characters. Anything else (query strings, fragments, escapes, `..`,
/// extra path segments) is rejected rather than interpreted.
#[cfg(any(target_os = "windows", target_os = "macos", test))]
fn release_asset_tag<'a>(url: &'a str, releases: &str, ext: &str) -> Option<&'a str> {
    let (tag, name) = url.strip_prefix(releases)?.split_once('/')?;
    let plain = |s: &str, extra: &[char]| {
        !s.is_empty()
            && s.chars()
                .all(|c| c.is_ascii_alphanumeric() || extra.contains(&c))
    };
    // GitHub escapes the `+` of a build-metadata version (`0.1.0-dev+1fa8e8b`)
    // as `%2B`; that is the only escape allowed.
    let name = name.replace("%2B", "+");
    let ok = plain(tag, &['.'])
        && name.ends_with(ext)
        && plain(&name, &['.', '-', '_', '+'])
        && !name.starts_with('.');
    ok.then_some(tag)
}

/// `vYYYY.MM.DD[.N]` tags compared number by number (so `.10` follows `.9`).
/// A tag that doesn't parse is never newer, and nothing is newer than a build
/// without a release tag (test builds don't update).
#[cfg(any(target_os = "windows", target_os = "macos", test))]
fn is_newer_release(candidate: &str, current: &str) -> bool {
    fn parts(tag: &str) -> Option<Vec<u32>> {
        tag.strip_prefix('v')?
            .split('.')
            .map(|p| p.parse().ok())
            .collect()
    }
    match (parts(candidate), parts(current)) {
        (Some(c), Some(cur)) => c > cur,
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::{is_newer_release, release_asset_tag};

    #[test]
    fn release_tags_compare_by_number() {
        assert!(is_newer_release("v2026.10.13", "v2026.10.12"));
        assert!(is_newer_release("v2026.10.12.1", "v2026.10.12"));
        assert!(is_newer_release("v2026.10.12.10", "v2026.10.12.9"));
        assert!(!is_newer_release("v2026.10.12", "v2026.10.12"));
        assert!(!is_newer_release("v2026.09.30", "v2026.10.12"));
        assert!(!is_newer_release("v2026.10.13-rc1", "v2026.10.12"));
        assert!(!is_newer_release("latest", "v2026.10.12"));
        assert!(!is_newer_release("v2026.10.12", ""));
    }

    #[test]
    fn only_plain_release_asset_urls_are_accepted() {
        const R: &str = "https://github.com/o/r/releases/download/";
        fn tag(u: &str) -> Option<&str> {
            release_asset_tag(u, R, ".dmg")
        }
        assert_eq!(
            tag(&format!(
                "{R}v2026.10.13/JelliumDesktop-2026.10.13-macos-12-x86_64.dmg"
            )),
            Some("v2026.10.13")
        );
        assert_eq!(
            tag("https://github.com/o/r/releases/download.evil/v1/a.dmg"),
            None
        );
        assert_eq!(tag(&format!("{R}v1/../../x/a.dmg")), None);
        assert_eq!(tag(&format!("{R}v1/%2e%2e/a.dmg")), None);
        assert_eq!(tag(&format!("{R}v1/a.zip?x=.dmg")), None);
        assert_eq!(tag(&format!("{R}v1/a.dmg#x")), None);
        assert_eq!(tag(&format!("{R}v1/.dmg")), None);
        assert_eq!(tag(&format!("{R}/a.dmg")), None);
        assert_eq!(
            tag("http://github.com/o/r/releases/download/v1/a.dmg"),
            None
        );
    }
}
