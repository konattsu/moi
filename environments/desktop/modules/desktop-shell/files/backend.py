"""Desktop actions and search data. No GUI imports, shell interpolation or host setup."""

import fcntl
import json
from pathlib import Path
import re
import subprocess
import time
import urllib.parse
import uuid


ROOT = Path(__file__).resolve().parent


def run(*args, data=None):
    return subprocess.run(
        args, input=data, stdout=subprocess.PIPE, stderr=subprocess.PIPE,
        check=True, timeout=5,
    ).stdout


def spawn(*args):
    # Applications belong to independent UWSM units, not the panel's cgroup.
    return subprocess.Popen(("uwsm", "app", "-t", "service", "--", *args), start_new_session=True)


def hypr_json(what):
    return json.loads(run("hyprctl", "-j", what))


def dispatch(expression):
    result = run("hyprctl", "dispatch", expression).decode()
    if result.strip() != "ok":
        raise RuntimeError(result.strip())


def selector(address):
    if not re.fullmatch(r"0x[0-9a-fA-F]+", address):
        raise ValueError("Invalid window address")
    return json.dumps("address:" + address)


def application_name(class_name):
    if class_name == "kitty":
        return "Terminal"
    if class_name == "moi-scratchpad" or class_name.startswith("moi-scratchpad-"):
        return "Scratchpad"
    if class_name in ("zen", "zen-browser"):
        return "Zen"
    return class_name or "Application"


def window_detail(window):
    name = application_name(window.get("class", ""))
    workspace = window.get("workspace", {}).get("name", "")
    return name if not workspace or workspace.startswith("special:") else name + " · " + workspace


def search_rows(text, apps, windows, clips, search_url):
    text = text.lstrip()
    prefixes = {"@": "app", "%": "window", "$": "clip", "&": "web"}
    kind = prefixes.get(text[:1])
    query = (text[1:] if kind else text).strip()
    needle = query.casefold()
    if not query and kind is None:
        return []
    rows = []
    for app in apps:
        if kind in (None, "app") and needle in app["search"].casefold():
            rows.append({"kind": "app", "label": app["name"], "detail": "", "value": app["id"]})
    for window in windows:
        title = window.get("title", "")
        app_name = application_name(window.get("class", ""))
        if kind in (None, "window") and needle in (title + " " + window.get("class", "") + " " + app_name).casefold():
            rows.append({"kind": "window", "label": title or window.get("class", "Window"),
                         "detail": window_detail(window),
                         "value": window["address"]})
    for clip in clips:
        if kind in (None, "clip") and needle in clip.casefold():
            rows.append({"kind": "clip", "label": clip.partition("\t")[2], "detail": "", "value": clip})
    if kind == "web" or (query and kind is None):
        url = search_url.replace("{query}", urllib.parse.quote(query, safe=""))
        rows.append({"kind": "web", "label": query or urllib.parse.urlparse(search_url).hostname or "Web", "detail": "", "value": url})
    # Limit each category so a large application list cannot hide windows or history.
    result = []
    for group in ("window", "app", "clip", "web"):
        result.extend([row for row in rows if row["kind"] == group][:25])
    return result


def activate(row, app_lookup):
    if row["kind"] == "app":
        if row["value"] not in app_lookup:
            raise ValueError("Application is no longer available")
        spawn(row["value"])
    elif row["kind"] == "window":
        hide_scratchpad()
        dispatch("hl.dsp.focus({window=" + selector(row["value"]) + "})")
    elif row["kind"] == "clip":
        data = run("cliphist", "decode", data=row["value"].encode())
        run("wl-copy", data=data)
        # Explicitly promote it even if the watcher has not received the event yet.
        run("cliphist", "store", data=data)
    elif row["kind"] == "web":
        spawn("xdg-open", row["value"])


class BarState:
    """A pinned bar and a temporary OSD share visibility without stealing focus."""

    def __init__(self):
        self.open = False
        self.bar = False
        self.generation = 0

    def show(self):
        self.open = self.bar = True
        self.generation += 1

    def toggle(self):
        if self.open:
            self.close()
        else:
            self.show()

    def close(self):
        self.open = self.bar = False
        self.generation += 1

    def flash(self):
        self.bar = True
        self.generation += 1
        return self.generation

    def expire(self, generation):
        if generation == self.generation and not self.open:
            self.bar = False


class Scratchpad:
    """A single runtime-owned window; tiling releases ownership, adoption replaces it."""

    def __init__(self, runtime, settings=None):
        self.runtime = Path(runtime)
        self.path = self.runtime / "scratch.json"
        self.settings = settings or {"width": .8, "height": .8, "top": 24}

    def current(self):
        try:
            saved = json.loads(self.path.read_text())
        except (FileNotFoundError, ValueError):
            return None
        return next((w for w in hypr_json("clients")
                     if w["address"] == saved["address"] and w["pid"] == saved["pid"]), None)

    def remember(self, window):
        self.path.write_text(json.dumps({"address": window["address"], "pid": window["pid"]}))

    def forget(self):
        self.path.unlink(missing_ok=True)

    def floating(self, window):
        target = selector(window["address"])
        monitor = next(m for m in hypr_json("monitors") if m["focused"])
        x, y, width, height = scratch_geometry(monitor, self.settings)
        normalize_window(target)
        dispatch("hl.dsp.window.float({window=" + target + ',action="enable"})')
        dispatch("hl.dsp.window.resize({window=" + target + f",x={width},y={height}" + "})")
        dispatch("hl.dsp.window.move({window=" + target + f",x={x},y={y}" + "})")
        dispatch("hl.dsp.window.tag({window=" + target + ',tag="+moi-scratch"})')
        dispatch("hl.dsp.focus({window=" + target + "})")
        dispatch("hl.dsp.window.alter_zorder({window=" + target + ',mode="top"})')

    def execute(self, action):
        # Also serialize separate command invocations and double taps during startup.
        with (self.runtime / "scratch.lock").open("w") as lock:
            fcntl.flock(lock, fcntl.LOCK_EX)
            current = self.current()
            if action == "tile":
                if not current:
                    return
                target = selector(current["address"])
                normalize_window(target)
                workspace = hypr_json("activeworkspace")["id"]
                dispatch("hl.dsp.window.move({window=" + target + f",workspace={workspace}" + "})")
                dispatch("hl.dsp.window.tag({window=" + target + ',tag="-moi-scratch"})')
                dispatch("hl.dsp.window.float({window=" + target + ',action="disable"})')
                self.forget()
                return
            if action == "adopt":
                active = hypr_json("activewindow")
                if active.get("class") not in ("kitty", "moi-scratchpad") and not active.get("class", "").startswith("moi-scratchpad-"):
                    raise RuntimeError("Focus a kitty window before moving it to the scratchpad.")
                if current and current["address"] != active["address"]:
                    dispatch("hl.dsp.window.close({window=" + selector(current["address"]) + "})")
                    # Do not orphan an old scratchpad when a close request is refused.
                    for _ in range(30):
                        if not self.current():
                            break
                        time.sleep(.05)
                    else:
                        raise RuntimeError("The current scratchpad did not close. Adoption was cancelled.")
                current = active
                self.remember(current)
                dispatch("hl.dsp.window.move({window=" + selector(current["address"]) +
                         ',workspace="special:moi-scratch"})')
                self.show(current)
                return
            if current:
                if current["workspace"]["name"] != "special:moi-scratch":
                    dispatch("hl.dsp.window.move({window=" + selector(current["address"]) +
                             ',workspace="special:moi-scratch"})')
                monitor = next(m for m in hypr_json("monitors") if m["focused"])
                if monitor.get("specialWorkspace", {}).get("name") == "special:moi-scratch":
                    dispatch('hl.dsp.workspace.toggle_special("moi-scratch")')
                else:
                    self.show(current)
                return
            self.forget()
            identity = "moi-scratchpad-" + uuid.uuid4().hex
            spawn("kitty", "--start-as", "normal", "--class", identity, "--title", "Scratchpad")
            for _ in range(60):
                window = next((w for w in hypr_json("clients")
                               if w.get("class") == identity), None)
                if window:
                    self.remember(window)
                    self.show(window)
                    return
                time.sleep(.05)
            raise RuntimeError("The scratchpad terminal did not start.")

    def show(self, window):
        monitor = next(m for m in hypr_json("monitors") if m["focused"])
        if monitor.get("specialWorkspace", {}).get("name") != "special:moi-scratch":
            dispatch('hl.dsp.workspace.toggle_special("moi-scratch")')
        # Apply logical geometry only after the special workspace is visible.
        self.floating(window)


def scratch_geometry(monitor, settings):
    width, height = monitor["width"], monitor["height"]
    if monitor.get("transform", 0) % 2:
        width, height = height, width
    width, height = width / monitor["scale"], height / monitor["scale"]
    left, top, right, bottom = monitor.get("reserved", [0, 0, 0, 0])
    usable_w, usable_h = width - left - right, height - top - bottom
    w = round(usable_w * settings["width"])
    h = round(min(usable_h * settings["height"], usable_h - settings["top"]))
    x = round(monitor.get("x", 0) + left + (usable_w - w) / 2)
    y = round(monitor.get("y", 0) + top + settings["top"])
    return x, y, w, h


def hide_scratchpad():
    monitor = next(m for m in hypr_json("monitors") if m["focused"])
    if monitor.get("specialWorkspace", {}).get("name") == "special:moi-scratch":
        dispatch('hl.dsp.workspace.toggle_special("moi-scratch")')


def normalize_window(target):
    # Hyprland rejects float/resize while either fullscreen state is active.
    # Maximized (1) is a fullscreen state too, not only fullscreen (2).
    dispatch("hl.dsp.window.fullscreen_state({window=" + target +
             ',internal=0,client=0,action="set",layout_aware=false})')


def focus_terminal(address):
    """A newly mapped regular kitty must not remain behind a scratchpad."""
    target = selector(address)
    window = next((w for w in hypr_json("clients") if w["address"] == address), None)
    if not window or window.get("class") != "kitty":
        return
    hide_scratchpad()
    normalize_window(target)
    dispatch("hl.dsp.window.float({window=" + target + ',action="disable"})')
    dispatch("hl.dsp.focus({window=" + target + "})")


def capture_preview(window):
    """Capture the toplevel, never a screen crop that could show another app."""
    identifier = str(window.get("stableId", ""))
    if not re.fullmatch(r"[A-Za-z0-9][A-Za-z0-9_-]{0,127}", identifier):
        return None
    try:
        result = subprocess.run(("grim", "-T", identifier, "-t", "png", "-l", "0", "-"),
                                stdout=subprocess.PIPE, stderr=subprocess.DEVNULL, timeout=2)
    except (OSError, subprocess.TimeoutExpired):
        return None
    if result.returncode == 0 and result.stdout.startswith(b"\x89PNG\r\n\x1a\n"):
        return result.stdout
    return None


def control(action):
    commands = {
        "volume-up": ("wpctl", "set-volume", "-l", "1", "@DEFAULT_AUDIO_SINK@", "5%+"),
        "volume-down": ("wpctl", "set-volume", "@DEFAULT_AUDIO_SINK@", "5%-"),
        "mute": ("wpctl", "set-mute", "@DEFAULT_AUDIO_SINK@", "toggle"),
        "brightness-up": ("brightnessctl", "set", "5%+"),
        "brightness-down": ("brightnessctl", "set", "5%-"),
    }
    run(*commands[action])


def get_level(kind):
    if kind == "audio":
        match = re.search(r"Volume:\s*([0-9.]+)", run("wpctl", "get-volume", "@DEFAULT_AUDIO_SINK@").decode())
        if not match:
            raise ValueError("The audio device did not report its volume.")
        return min(100, round(float(match[1]) * 100))
    current = int(run("brightnessctl", "get"))
    maximum = int(run("brightnessctl", "max"))
    return round(current / maximum * 100)


def set_level(kind, value):
    value = max(0 if kind == "audio" else 1, min(100, round(value)))
    if kind == "audio":
        run("wpctl", "set-volume", "-l", "1", "@DEFAULT_AUDIO_SINK@", str(value) + "%")
    else:
        run("brightnessctl", "set", str(value) + "%")


def write_output(path, content):
    path = Path(path)
    if path.is_symlink():
        raise ValueError("Refusing to replace generated symlink: " + str(path))
    path.parent.mkdir(parents=True, exist_ok=True)
    if not path.exists() or path.read_text() != content:
        # mkstemp avoids following a pre-existing temporary symlink.
        import tempfile
        with tempfile.NamedTemporaryFile(mode="w", dir=path.parent, delete=False) as temporary:
            temporary.write(content)
        Path(temporary.name).chmod(0o644)
        Path(temporary.name).replace(path)


def render(destination, profile_destination=None):
    from render_theme import render as render_theme
    render_theme(destination, profile_destination, root=ROOT)


if __name__ == "__main__":
    import sys
    if len(sys.argv) in (3, 4) and sys.argv[1] == "render":
        render(sys.argv[2], sys.argv[3] if len(sys.argv) == 4 else None)
    else:
        raise SystemExit("Usage: backend.py render OUTPUT_DIRECTORY [FCITX_PROFILE]")
