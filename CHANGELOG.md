# Changelog

All notable changes to the macOS 12 build. Newest first. Each release's notes
are published from the matching section below. Changes that come from the
Windows build are listed in the
[Jellium Desktop RTX changelog](https://github.com/trelowney/jellium-desktop-rtx/blob/main/CHANGELOG.md).

## 2026-10-12.1

### Fixed
- **Dropdowns in jellyfin-web's settings** (for example *Media segment actions*) didn't take the option you picked: *Skip* only applied after another click, and *Ask to skip* turned into *Skip* and could not be changed. Dropdowns now open as an in-page menu, the same one the Windows build uses, instead of a native macOS menu.

## 2026-10-12

First release for Intel Macs on macOS 12 Monterey, based on Jellium Desktop RTX
2026-09-21 and upstream jellium-desktop `14dc084`.

### Added
- **Self-update.** At startup, and from *Check for updates* on the About tab, the app looks for a newer release. *Update now* downloads the DMG, quits, replaces the app where it is installed and starts the new version. If anything fails, the old version is kept and started again, and a message says what went wrong. The app has to run from a folder you can write to, such as Applications.
- **Direct play only hardware-decodable video** (Settings → Transcode, on by default). The app asks macOS which of HEVC, VP9 and AV1 this Mac can decode in hardware and lets the server convert the others, instead of decoding them on the CPU. On a MacBook Air 2017 that means only H.264 plays directly; HEVC, VP9 and AV1 arrive converted. Smoother playback, less heat and longer battery life.
- **Own icon**: the green Jellium triangle with a *Legacy* badge, so it can't be confused with the official app.

### Changed
- The version in About and Playback Info reads *macOS 12 build* instead of *RTX build*.
- Everything is built for macOS 12: Chromium (CEF) is pinned to 150, the last release that supports Monterey, and ffmpeg, libplacebo, mpv and all libraries are built from source with a macOS 12 deployment target. The build fails if any binary in the app asks for a newer macOS.
