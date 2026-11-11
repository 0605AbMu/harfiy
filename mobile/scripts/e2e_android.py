#!/usr/bin/env python3
"""End-to-end typing test for the Harfiy accessibility service on a real device.

Types sentences by tapping the on-screen keyboard (so the real keyboard composes,
suggests and commits exactly as for a user) into the debug-only TestbedActivity, then
compares the field's text with the expected conversion.

Requirements:
- a debug build installed (./gradlew installDebug) and the Harfiy accessibility
  service enabled on the device (this script never changes system settings);
- a keyboard layout file with key coordinates for the device's keyboard
  (see keyboard_layouts/; measure with `adb exec-out screencap -p > s.png`).

Usage:
    mobile/scripts/e2e_android.py [--layout FILE] [--fields edit,nooffset,web]
                                  [--delays 0,0.12] [--serial SERIAL]

`<` in a sentence is Backspace. PASS = converted, SAFE = left exactly as typed (editors
that don't serve surrounding text, e.g. WebView), FAIL = anything else. Exit status is
non-zero when any case fails.
"""

import argparse
import json
import os
import shutil
import subprocess
import sys
import time

HERE = os.path.dirname(os.path.abspath(__file__))
DEFAULT_LAYOUT = os.path.join(HERE, "keyboard_layouts", "samsung_sm-a556e.json")
PKG = "uz.harfiy.keyboard"
ACTIVITY = f"{PKG}/.TestbedActivity"
SERVICE = f"{PKG}/{PKG}.HarfiyAccessibilityService"

CASES = [
    "salom dunyo shahar choy ",
    "bugun shahar chetida choyxonada ishladik ",
    "o'zbek tilida g'isht o'rik ",
    "shaxar<<har choy ",
]

# Extra cases for the `plain` field (keyboard commits key by key, no composing region):
# (typed, expected). Conversion is live, mid-word, and Backspace right after a
# conversion restores the pair, which then stays unconverted.
PLAIN_CASES = [
    ("qish", "qiş"),            # converted before the word ends (real time)
    ("ish<oq ", "ishoq "),      # instant undo keeps "sh"
    ("choy ish", "çoy iş"),
]

APOSTROPHES = "'`‘’ʻʼ´"


def convert(text):
    """Mirror of the engine's live-typing rules (no smart apostrophe, no ng)."""
    pairs = {("s", "h"): "ş", ("c", "h"): "ç"}
    out, i = [], 0
    while i < len(text):
        a = text[i]
        b = text[i + 1] if i + 1 < len(text) else ""
        low = (a.lower(), b.lower())
        if low in pairs:
            r = pairs[low]
            out.append(r.upper() if a.isupper() else r)
            i += 2
        elif a.lower() in "og" and b and b in APOSTROPHES:
            r = "ö" if a.lower() == "o" else "ğ"
            out.append(r.upper() if a.isupper() else r)
            i += 2
        else:
            out.append(a)
            i += 1
    return "".join(out)


def apply_backspaces(text):
    out = []
    for c in text:
        if c == "<":
            if out:
                out.pop()
        else:
            out.append(c)
    return "".join(out)


class Device:
    def __init__(self, serial):
        adb = shutil.which("adb") or os.path.expanduser("~/Library/Android/sdk/platform-tools/adb")
        self.base = [adb] + (["-s", serial] if serial else [])

    def sh(self, cmd, check=True):
        r = subprocess.run(self.base + ["shell", cmd], capture_output=True, text=True)
        if check and r.returncode != 0:
            raise RuntimeError(f"adb shell {cmd!r}: {r.stderr.strip()}")
        return r.stdout

    def logcat(self, *args):
        return subprocess.run(self.base + ["logcat", *args], capture_output=True, text=True).stdout


def load_layout(path):
    with open(path, encoding="utf-8") as f:
        spec = json.load(f)
    keys = {}
    for row in spec["rows"]:
        for i, c in enumerate(row["keys"]):
            keys[c] = [[row["x0"] + row["dx"] * i, row["y"]]]
    for c, taps in spec["keys"].items():
        keys[c] = taps
    return keys


def type_text(dev, layout, text, delay):
    cmds = []
    for c in text:
        if c not in layout:
            raise ValueError(f"no key for {c!r} in layout")
        taps = layout[c]
        for i, (x, y) in enumerate(taps):
            if i > 0:
                cmds.append("sleep 0.25")  # let the keyboard switch layers
            cmds.append(f"input tap {x} {y}")
        if delay > 0:
            cmds.append(f"sleep {delay}")
    dev.sh(";".join(cmds))


def last_text(dev):
    lines = [l for l in dev.logcat("-d", "-s", "HarfiyTestbed:I").splitlines() if "TEXT=" in l]
    if not lines:
        return None
    return lines[-1].split("TEXT=", 1)[1].replace("\\n", "\n")


def dump_diagnostics(dev):
    dev.logcat("-c")
    dev.sh(f"am start -W -n {ACTIVITY} --ez dump true -f 0x10008000")
    time.sleep(1.0)
    return [l.split("DIAG ", 1)[1] for l in dev.logcat("-d", "-s", "HarfiyTestbed:I").splitlines() if "DIAG " in l]


def settled_text(dev, quiet=1.0, timeout=6.0):
    """The field's text once it stopped changing (WebView reports through JS, with lag)."""
    deadline = time.time() + timeout
    last, since = None, time.time()
    while time.time() < deadline:
        time.sleep(0.25)
        cur = last_text(dev)
        if cur != last:
            last, since = cur, time.time()
        elif time.time() - since >= quiet:
            break
    return last


def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--layout", default=DEFAULT_LAYOUT)
    ap.add_argument("--fields", default="edit,plain,nooffset,web")
    ap.add_argument("--delays", default="0,0.12", help="seconds between taps, comma separated")
    ap.add_argument("--serial")
    ap.add_argument("--field-tap", default="540,420", help="x,y inside the testbed field, to open the keyboard")
    ap.add_argument("--diag", action="store_true", help="print the session diagnostics after each failure")
    args = ap.parse_args()

    dev = Device(args.serial)
    layout = load_layout(args.layout)
    if SERVICE not in dev.sh("settings get secure enabled_accessibility_services"):
        sys.exit("Harfiy accessibility service is not enabled on the device; enable it in Settings first.")

    failures = 0
    print(f"{'field':9} {'delay':>5}  result")
    for field in args.fields.split(","):
        for delay in (float(d) for d in args.delays.split(",")):
            cases = [(c, None) for c in CASES]
            if field == "plain":
                cases += PLAIN_CASES
            for case, expected in cases:
                dev.sh(f"am start -W -n {ACTIVITY} --es field {field} -f 0x10008000")
                time.sleep(1.2)
                fx, fy = args.field_tap.split(",")
                dev.sh(f"input tap {fx} {fy}")  # WebView needs a tap to show the keyboard
                time.sleep(0.8)
                dev.logcat("-c")
                type_text(dev, layout, case, delay)
                got = settled_text(dev)
                typed = apply_backspaces(case)
                want = expected if expected is not None else convert(typed)
                ok = got == want
                # Editors that don't serve surrounding text (WebView) are left alone on
                # purpose: unconverted is safe, anything else is corruption.
                safe = got == typed
                failures += not (ok or safe)
                mark = "PASS" if ok else ("SAFE" if safe else "FAIL")
                print(f"{field:9} {delay:5.2f}  {mark}  {case!r}" + ("" if ok else f"\n{'':18}want {want!r}\n{'':18}got  {got!r}"))
                if not (ok or safe) and args.diag:
                    print("\n".join(" " * 18 + l for l in dump_diagnostics(dev)[-60:]))
    dev.sh("input keyevent KEYCODE_HOME", check=False)
    print(f"\n{failures} failure(s)")
    sys.exit(1 if failures else 0)


if __name__ == "__main__":
    main()
