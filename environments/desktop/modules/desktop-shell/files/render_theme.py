"""One palette produces all desktop styles and SVG colours."""

import json
from pathlib import Path
import re
import shlex
import xml.etree.ElementTree as ET
from backend import ROOT, write_output

def palette(source):
    if set(source) != {"background", "foreground", "accent", "warning", "success", "error"}:
        raise ValueError("Edit only the six base colours in config/theme.json")
    if any(not re.fullmatch(r"#[0-9a-fA-F]{6}", color) for color in source.values()):
        raise ValueError("Theme colors must be #RRGGBB")
    def mix(fraction):
        a, b = source["background"], source["foreground"]
        values = [round(int(a[i:i+2], 16) * (1 - fraction) + int(b[i:i+2], 16) * fraction) for i in (1, 3, 5)]
        return "#" + "".join(f"{value:02X}" for value in values)
    return {**source, "surface": mix(.045), "raised": mix(.085), "border": mix(.12), "muted": mix(.65)}


def render(destination, profile_destination=None, root=None):
    """Render derived files only at explicit apply, or into a test directory."""
    root = Path(root) if root is not None else ROOT
    theme = palette(json.loads((root / "config/theme.json").read_text()))
    destination = Path(destination)
    destination.mkdir(parents=True, exist_ok=True)
    settings = json.loads((root / "config/settings.json").read_text())
    layout = settings["keyboard_layout"]
    if not re.fullmatch(r"[a-zA-Z0-9_-]+", layout):
        raise ValueError("keyboard_layout must be a single XKB layout name")
    scratch = settings["scratchpad"]
    if not (.25 <= scratch["width"] <= 1 and .25 <= scratch["height"] <= 1 and
            0 <= scratch["top"] <= 160 and .5 <= scratch["opacity"] <= 1):
        raise ValueError("Invalid scratchpad geometry or opacity")
    reduced = "true" if settings.get("reduced_motion", False) else "false"
    write_output(destination / "settings.lua", "return { keyboard_layout = " + json.dumps(layout) +
                 f', scratch_width = {scratch["width"]}, scratch_height = {scratch["height"]}, ' +
                 f'scratch_top = {scratch["top"]}, scratch_opacity = {scratch["opacity"]}, reduced_motion = {reduced}' + " }\n")
    profile = (root / "config/fcitx-profile").read_text()
    profile = profile.replace("=us", "=" + layout).replace("=keyboard-us", "=keyboard-" + layout)
    write_output(destination / "fcitx-profile", profile)
    if profile_destination is not None:
        write_output(profile_destination, profile)
    for name in ("shell.css", "waybar.css", "kitty.conf", "hyprlock.conf", "theme.lua", "gtk.css", "tuigreet.txt", "wallpaper.svg"):
        content = (root / "theme/templates" / name).read_text()
        for key, color in theme.items():
            content = content.replace("{{" + key + "}}", color)
            content = content.replace("{{" + key + "_hex}}", color[1:])
        content = content.replace("@icons@", (destination.resolve() / "icons").as_uri())
        if "{{" in content:
            raise ValueError("Unresolved theme token in " + name)
        write_output(destination / name, content)
    ET.register_namespace("", "http://www.w3.org/2000/svg")
    for source in sorted((root / "assets/material").glob("*.svg")):
        svg = ET.fromstring(source.read_text())
        svg.set("fill", theme["warning"] if source.stem == "battery_alert" else theme["foreground"])
        svg.insert(0, ET.Comment("Moi: generated fill colour; original and Apache license are in assets/material."))
        write_output(destination / "icons" / source.name, ET.tostring(svg, encoding="unicode"))
    substitutions = {
        "@icons@": str(destination.resolve() / "icons"),
        "@desktop@": shlex.quote(str(root.resolve() / "desktop.py")),
        "@status@": shlex.quote(str(root.resolve() / "status.py")),
    }
    def expand(value):
        if isinstance(value, dict):
            return {key: expand(item) for key, item in value.items()}
        if isinstance(value, list):
            return [expand(item) for item in value]
        if isinstance(value, str):
            for token, replacement in substitutions.items():
                value = value.replace(token, replacement)
        return value
    bar = expand(json.loads((root / "config/waybar.json").read_text()))
    write_output(destination / "waybar.json", json.dumps(bar, ensure_ascii=False, indent=2) + "\n")
