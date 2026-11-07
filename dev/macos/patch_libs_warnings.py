#!/usr/bin/env python3
"""Fix what makes ffmpeg and libplacebo warn when built for macOS 12.

Usage: patch_libs_warnings.py ffmpeg|libplacebo <source dir>
Fails if a change no longer applies, so a version bump can't silently bring
the warnings back.
"""
import pathlib
import sys

PATCHES = {
    "ffmpeg": {
        # The linker has ignored -single_module for years and warns about it.
        "configure": [
            (" -Wl,-single_module -Wl,-install_name,", " -Wl,-install_name,"),
        ],
        # Backport of 46d899d7a0 from release/8.1: ret was returned uninitialized.
        "libavformat/mov.c": [
            (
                "    ff_mov_read_chnl(c->fc, pb, st);\n",
                "    ret = ff_mov_read_chnl(c->fc, pb, st);\n"
                "    if (ret < 0) {\n"
                "        avio_seek(pb, end, SEEK_SET);\n"
                "        return 0;\n"
                "    }\n",
            ),
        ],
        # Dummy writer of size 0 (nothing is written); the cast states that.
        "libavcodec/bsf/extract_extradata.c": [
            (
                "bytestream2_init_writer(&pb_extradata, nal->data, 0);",
                "bytestream2_init_writer(&pb_extradata, (uint8_t *)nal->data, 0);",
            ),
        ],
    },
    "libplacebo": {
        # The 'lib' prefix is only for Windows; elsewhere meson warns that the
        # pkg-config file's -lplacebo might not match.
        "src/meson.build": [
            (
                "  name_prefix: 'lib'\n",
                "  kwargs: host_machine.system() == 'windows' ? {'name_prefix': 'lib'} : {},\n",
            ),
        ],
    },
}

target, root = sys.argv[1], pathlib.Path(sys.argv[2])
for name, changes in PATCHES[target].items():
    path = root / name
    text = path.read_text()
    for old, new in changes:
        if text.count(old) != 1:
            raise SystemExit(f"{name}: expected exactly one match for {old!r}")
        text = text.replace(old, new)
    path.write_text(text)
