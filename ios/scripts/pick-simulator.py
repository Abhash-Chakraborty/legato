#!/usr/bin/env python3
"""Prints the simulator to test on as `udid=…`, `name=…` and `os=…` lines:
an iPhone on the newest iOS this Xcode has, the plain model before the Pro
and Max ones. Reads `xcrun simctl list devices available -j`."""
import json
import re
import subprocess
import sys

listing = json.loads(
    subprocess.run(
        ["xcrun", "simctl", "list", "devices", "available", "-j"],
        check=True,
        capture_output=True,
        text=True,
    ).stdout
)

found = []
for runtime, devices in listing["devices"].items():
    version = re.search(r"SimRuntime\.iOS-(\d+)-(\d+)(?:-(\d+))?$", runtime)
    if not version:
        continue
    number = tuple(int(part or 0) for part in version.groups())
    for device in devices:
        name = device["name"]
        if not name.startswith("iPhone") or not device.get("isAvailable", True):
            continue
        # iPhone 17 before iPhone 17 Pro, iPhone Air and the rest.
        plain = bool(re.fullmatch(r"iPhone \d+", name))
        found.append((number, plain, name, device["udid"]))

if not found:
    sys.exit("No iPhone simulator is installed for this Xcode.")

number, _, name, udid = max(found)
print(f"udid={udid}")
print(f"name={name}")
print(f"os={'.'.join(str(part) for part in number).removesuffix('.0')}")
