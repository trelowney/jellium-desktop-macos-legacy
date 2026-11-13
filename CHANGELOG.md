# Changelog

All notable changes to the macOS 12 build. Newest first. Each release's notes
are published from the matching section below. Changes that come from the
Windows build are listed in the
[Jellium Desktop RTX changelog](https://github.com/trelowney/jellium-desktop-rtx/blob/main/CHANGELOG.md).

## 2026-11-13

### Added
- **Jellyfin Enhanced subtitle style applies in the player.** If the server has the Jellyfin Enhanced plugin, the subtitle style each user picks there (text color, background, size, font, text effect, vertical position) is now drawn by the player too, at the same size as in the browser. Changes made in the plugin's panel show up within a second, even during playback. Without the plugin, or with its custom subtitle styles turned off, subtitles look exactly as before, including anything set in your own `mpv.conf`. Only plain-text subtitles (SRT, WebVTT, ...) are styled, as in jellyfin-web; ASS and image subtitles keep their own look.
- **Jellyfin Enhanced pause screen works in the app.** With the plugin's pause screen turned on, pausing for a few seconds shows the item's backdrop, logo, plot and progress. A click on the screen, Space or Enter resumes the video, the same as in the browser. Without the plugin, or with the pause screen turned off in it, pausing works as before.

## 2026-11-07.3

### Fixed
- **Video flashed in random colors.** The bundled MoltenVK, which draws the video through Metal, had been updated to 1.4.2, which breaks drawing on older Intel graphics such as the Iris 6000. The app uses 1.4.1 again, as in 2026-10-16.
- **The first launch after starting the Mac, or after an update, still froze and then quit.** At startup the app read a few values from the player on the main thread while the player, still setting up its window, waited for that same thread. A slow cold start made the two wait for each other. These reads now run in the background while the main thread keeps working.

## 2026-11-07.2

### Fixed
- **The first launch after starting the Mac, or after an update, froze and then quit.** At startup the app asks macOS which video formats this Mac decodes in hardware, and on a cold start that question could hang. It is now asked in the background and given 3 seconds; if macOS hasn't answered by then, the server converts HEVC, VP9 and AV1 video for that session, and the next launch asks again.

## 2026-11-07.1

### Fixed
- **Nothing played in 2026-11-07 over HTTPS.** The player's OpenSSL found no certificate authorities on the Mac, so every HTTPS stream failed with "certificate verify failed" and the title logo flashed a few times before playback gave up. The player now trusts the same certificate authorities as macOS, including any you added in Keychain Access. To use your own file instead, set `tls-ca-file` in `~/.config/jellium-desktop-rtx/mpv/mpv.conf`.

## 2026-11-07

### Fixed
- **Dolby Vision profile 5 plays through the server.** These files have no HDR10 base layer and can show green or purple when played directly, so the server now transcodes them, with the correct colors. This picture is SDR. Other Dolby Vision files (profiles 7 and 8) follow the same rules as before.

### Changed
- **ffmpeg 8.1.3.** Bug-fix update of the bundled ffmpeg. HTTPS streams now go through OpenSSL instead of Apple's Secure Transport, which Apple deprecated.

## 2026-10-16

### Added
- **Remote control mode**, from the Windows build. The remote button in the top bar (next to Search) switches the app between mouse and keyboard and a remote or keyboard that sends arrows, Enter and Escape (or Backspace, the Mac's delete key). A highlight moves row by row through posters, episodes and buttons; Enter opens the highlighted item, plays an episode from a season's episode list and opens dropdowns such as Audio and Subtitles; Escape goes back to the item you came from and, during playback, stops it. The update dialog works with it too. The player's own keys are unchanged, the choice is remembered, and in mouse and keyboard mode no key behaves differently.

### Fixed
- **Choosing an item in a dropdown with Enter no longer reopens it.**
- **Disconnecting AirPods or other audio devices during playback could crash the app.** The bundled mpv now registers for audio device changes only after its audio output has started, and cleans up if that start fails (upstream fix, andrewrabert/jellium-desktop#719).

## 2026-10-14

### Fixed
- **The first launch after starting the Mac, or after an update, froze and then quit.** On a cold start an older Mac needs more than the 5 seconds the app gave its window and renderer to come up, so it gave up. It now waits up to a minute; the first launch can still take a little longer than the next ones.

## 2026-10-12.3

### Changed
- **The update shows its progress.** After *Update now* the dialog stays open with a progress bar (*Downloading … MB*, *Installing…*, *Restarting…*); the app downloads and copies the new version while it keeps running and only quits for the final swap, which takes a moment. Before, it quit right away and worked in the background with nothing on screen. If the update fails, the dialog closes and the app keeps running.

### Added
- **The app reopens the way you closed it**: fullscreen if you quit in fullscreen, otherwise in a window with its last size and position (which it already remembered).

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
