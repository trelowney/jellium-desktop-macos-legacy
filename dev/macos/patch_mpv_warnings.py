#!/usr/bin/env python3
"""Remove what makes mpv's Swift sources warn, for a build without OpenGL.

The app renders through Vulkan (metal_layer.swift); the OpenGL backend that uses
most of these APIs is turned off with -Dmacos-cocoa-cb=disabled. Fails if a
change no longer applies, so a submodule update can't silently bring them back.
"""
import pathlib

PATCHES = {
    # Never mutated.
    "osdep/mac/app_hub.swift": [
        ("            var path = $0.absoluteString\n", "            let path = $0.absoluteString\n"),
    ],
    # OpenGL surface properties, unused with Vulkan.
    "video/out/mac/view.swift": [
        ("        wantsBestResolutionOpenGLSurface = true\n", ""),
        ("        wantsExtendedDynamicRangeOpenGLSurface = true\n", ""),
    ],
    # Materials deprecated since macOS 10.14. The app doesn't set them, and
    # --macos-title-bar-material now falls back to the default for these values.
    "video/out/mac/title_bar.swift": [
        (
            "            // fix for broken deprecated materials\n"
            "            if material == .light || material == .dark || material == .mediumLight || material == .ultraDark {\n"
            "                state = .active\n"
            "            } else {\n"
            "                state = .followsWindowActiveState\n"
            "            }\n",
            "            state = .followsWindowActiveState\n",
        ),
        ("            case MAC_MAT_DARK: return .dark\n", ""),
        ("            case MAC_MAT_LIGHT: return .light\n", ""),
        ("            case MAC_MAT_MEDIUM_LIGHT: return .mediumLight\n", ""),
        ("            case MAC_MAT_ULTRA_DARK: return .ultraDark\n", ""),
    ],
}

for name, changes in PATCHES.items():
    path = pathlib.Path("third_party/mpv") / name
    text = path.read_text()
    for old, new in changes:
        if text.count(old) != 1:
            raise SystemExit(f"{name}: expected exactly one match for {old!r}")
        text = text.replace(old, new)
    path.write_text(text)
