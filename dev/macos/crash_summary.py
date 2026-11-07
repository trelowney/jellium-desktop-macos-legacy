#!/usr/bin/env python3
"""Print the reason and the crashed thread's frames from a macOS .ips report.

Usage: crash_summary.py <report.ips>
"""
import json
import sys

with open(sys.argv[1]) as f:
    f.readline()  # the first line is a separate JSON header
    report = json.load(f)
print("exception:", json.dumps(report.get("exception", {})))
for line in report.get("asi", {}).values():
    print("message:", line)
images = report.get("usedImages", [])
for thread in report.get("threads", []):
    if not thread.get("triggered"):
        continue
    for frame in thread.get("frames", [])[:30]:
        image = images[frame["imageIndex"]].get("name", "?") if frame.get("imageIndex", -1) < len(images) else "?"
        print(f"  {image}  {frame.get('symbol', '?')}")
