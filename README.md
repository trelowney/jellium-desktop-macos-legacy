# Jellium Desktop for legacy Intel Macs

A build of [**Jellium Desktop RTX**](https://github.com/trelowney/jellium-desktop-rtx) for **old Intel Macs stuck on macOS 12 Monterey** (for example a MacBook Air 2017), which the official client no longer runs on. It is based on [jellium-desktop](https://github.com/andrewrabert/jellium-desktop), the CEF + mpv desktop client for Jellyfin.

> [!NOTE]
> Unofficial build for personal use. On a Mac that runs macOS 13 or newer, use the official
> [andrewrabert/jellium-desktop](https://github.com/andrewrabert/jellium-desktop) instead.

## Who this is for

- **Intel Macs on macOS 12 Monterey.** Apple Silicon Macs all run macOS 13+ and don't need this.
- Everything in the app bundle is built for macOS 12: ffmpeg, libplacebo, mpv and every library they use are compiled from source with a macOS 12 deployment target, and the build fails if any binary in the bundle asks for a newer macOS.

## Why it stays on CEF 150

The embedded browser is CEF (Chromium). **Chromium 150 is the last release that supports macOS 12**; Chromium 151 and later require macOS 13 Ventura. This build is therefore pinned to **CEF 150** and will stay there. It no longer receives Chromium security fixes, which is acceptable for an app that only talks to your own Jellyfin server, but don't use it to browse anything else.

## Features

It has everything the RTX build has, except the parts that can't exist on an old Intel Mac.

| Feature | Here |
|---|---|
| Logo in the player (series/movie clear logo under the OSD title) | ✅ |
| Configurable buffer size (Settings → Playback) | ✅ |
| Live buffer in Playback Info | ✅ |
| Subtitle offset works | ✅ |
| Hard Reload / Clear Cache and Reload in the right-click menu | ✅ |
| Save and close in Settings | ✅ |
| Hardware decoding defaults to `auto` (VideoToolbox on macOS) | ✅ |
| Direct play only video this Mac decodes in hardware; the server converts the rest (Settings → Transcode) | ✅ Mac only |
| Dolby Vision profile 5 is transcoded by the server (to SDR), so it never shows green or purple | ✅ |
| Version shows build date and upstream base | ✅ |
| Self-updating (downloads the new DMG and replaces the app) | ✅ |
| Own icon (green, with a *Legacy* badge) | ✅ |
| Remote control mode (arrows, Enter, Escape; switch in the top bar) | ✅ |
| Jellyfin Enhanced subtitle style and pause screen in the player (only when the server has the plugin) | ✅ |
| NVIDIA RTX Video Super Resolution / RTX Video HDR | ❌ needs an NVIDIA RTX GPU on Windows |
| NVIDIA guard, GPU load (NVML) in Playback Info | ❌ NVIDIA-only |
| Styled dropdowns, the two startup/overlay fixes | ❌ Windows-only fixes; macOS doesn't need them |

## Download

Download the `.dmg` from the [latest release](https://github.com/trelowney/jellium-desktop-macos-legacy/releases/latest) and drag the app into Applications. It is not signed with an Apple Developer ID, so on first launch **right-click the app → Open** and confirm. After that it updates itself.

The `test-build` pre-release is the latest `main` and does not update itself.

## Building

GitHub Actions builds everything on an Intel macOS runner: `.github/workflows/build-macos-legacy.yml`. The libraries come prebuilt from `.github/workflows/build-macos-libs.yml`, which takes them from MacPorts with `macosx_deployment_target 12.0` (Homebrew bottles target macOS 14+ and won't load on Monterey) and builds ffmpeg and libplacebo from source against them. The release workflow pins that build by tag and checksum.

Before anything is published, a smoke test (`dev/macos/smoke_test.sh`) starts the app against a stand-in Jellyfin server and plays a short clip over HTTPS, with the build machine's libraries moved away. `.github/workflows/smoke-macos.yml` runs the same test against any published release in a few minutes. It can't catch problems specific to one Mac's graphics or a slow cold start, which still need a real Mac.

## Credits / license

Based on [andrewrabert/jellium-desktop](https://github.com/andrewrabert/jellium-desktop) and [trelowney/jellium-desktop-rtx](https://github.com/trelowney/jellium-desktop-rtx), licensed under the same terms (GPLv2). All credit for the client itself goes to the Jellyfin project and the jellium-desktop authors.
