"""Headless layer-shell/DBus smoke test; run inside dbus-run-session in devcontainer.

Uses a private compositor and temporary XDG directories. Hardware actions are faked.
"""

import importlib.util
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile
import time


FILES = Path(__file__).resolve().parents[1] / "files"


def main():
    with tempfile.TemporaryDirectory(prefix="moi-desktop-smoke-") as directory:
        root = Path(directory)
        for name in ("runtime", "config", "cache", "data", "bin"):
            (root / name).mkdir(mode=0o700)
        os.environ.update({
            "XDG_RUNTIME_DIR": str(root / "runtime"), "XDG_CONFIG_HOME": str(root / "config"),
            "XDG_CACHE_HOME": str(root / "cache"), "XDG_DATA_HOME": str(root / "data"),
            "WLR_BACKENDS": "headless", "WLR_RENDERER": "pixman", "WLR_HEADLESS_OUTPUTS": "1",
            "GDK_BACKEND": "wayland", "NO_AT_BRIDGE": "1",
        })
        config = root / "sway.conf"
        size = os.environ.get("MOI_DESKTOP_TEST_SIZE", "1600x1000")
        width, height = (int(part) for part in size.split("x"))
        config.write_text(f"output HEADLESS-1 mode {width}x{height}\nseat seat0 fallback true\n")
        with (root / "sway.log").open("w+") as log:
            sway = subprocess.Popen(["sway", "-c", str(config)], stdout=log, stderr=log)
            app = None
            try:
                for _ in range(100):
                    sockets = [p for p in (root / "runtime").glob("wayland-*") if not p.name.endswith(".lock")]
                    if sockets:
                        os.environ["WAYLAND_DISPLAY"] = sockets[0].name
                        break
                    if sway.poll() is not None:
                        log.seek(0)
                        raise RuntimeError(log.read())
                    time.sleep(.02)
                else:
                    raise RuntimeError("Headless compositor startup timed out")

                # Keep the real Waybar and wl-paste, replace unavailable system tools.
                for name in ("fcitx5", "hypridle"):
                    path = root / "bin" / name
                    path.write_text("#!/bin/sh\nexec sleep 300\n")
                    path.chmod(0o755)
                path = root / "bin/cliphist"
                path.write_text("#!/bin/sh\nif [ \"$1\" = list ]; then printf '1\\tClipboard draft for the release notes\\n'; else cat >/dev/null; fi\n")
                path.chmod(0o755)
                os.environ["PATH"] = str(root / "bin") + ":" + os.environ["PATH"]
                sys.path.insert(0, str(FILES))
                import backend
                source = root / "config/moi-desktop"
                shutil.copytree(FILES, source)
                # No audio, battery or Hyprland IPC exists in this private Sway.
                waybar = json.loads((source / "config/waybar.json").read_text())
                waybar.update({"modules-left": [], "modules-right": ["custom/notifications", "custom/session"]})
                (source / "config/waybar.json").write_text(json.dumps(waybar))
                backend.ROOT = source
                backend.render(source / "generated")
                fixtures = [
                    {"address": "0xabc", "title": "backend.py — Editor", "class": "kitty", "workspace": {"name": "1", "id": 1}},
                    {"address": "0xdef", "title": "Research — Zen", "class": "zen", "workspace": {"name": "2", "id": 2}},
                    {"address": "0xaaa", "title": "Development server", "class": "kitty", "workspace": {"name": "1", "id": 1}},
                ]
                backend.hypr_json = lambda _: fixtures
                backend.get_level = lambda _: 42
                spec = importlib.util.spec_from_file_location("desktop", FILES / "desktop.py")
                module = importlib.util.module_from_spec(spec)
                spec.loader.exec_module(module)
                app = module.Desktop()
                assert app.register(None)
                from gi.repository import Gio, GLib, Gtk
                context = GLib.MainContext.default()

                def spin(seconds=.2):
                    end = time.monotonic() + seconds
                    while time.monotonic() < end:
                        while context.pending():
                            context.iteration(False)
                        time.sleep(.005)

                def visible_text(widget):
                    text = [widget.get_text()] if isinstance(widget, Gtk.Label) and widget.get_visible() else []
                    if isinstance(widget, Gtk.Container):
                        for child in widget.get_children():
                            text.extend(visible_text(child))
                    return text

                def screenshot(name):
                    output = Path(os.environ.get("MOI_DESKTOP_PREVIEW_DIR", str(root / "screens")))
                    output.mkdir(parents=True, exist_ok=True)
                    path = output / (name + ".png")
                    subprocess.run(["grim", "-o", "HEADLESS-1", str(path)], check=True, timeout=5)
                    return path

                spin(1.2)
                assert app.bar.poll() is None
                app.state.toggle()
                app.sync_bar()
                spin()
                assert app.state.open and not app.overlay.get_visible(), "Super must reveal only the bar"
                app.flash()
                spin()
                assert app.state.open
                bar_capture = screenshot("bar")
                from gi.repository import GdkPixbuf
                from render_theme import palette
                rendered = GdkPixbuf.Pixbuf.new_from_file(str(bar_capture))
                offset = 10 * rendered.get_rowstride() + 30 * rendered.get_n_channels()
                actual = tuple(rendered.get_pixels()[offset:offset+3])
                surface = palette(json.loads((source / "config/theme.json").read_text()))["surface"]
                expected = tuple(int(surface[i:i+2], 16) for i in (1, 3, 5))
                assert actual == expected, ("Waybar must actually render, not merely keep its process alive", actual, expected)
                app.dismiss_bar()
                app.show("search")
                spin()
                assert app.content.get_allocated_width() <= width, "Search must fit the logical screen"
                assert not app.results.get_children(), "An empty query must show no candidates"
                assert not app.scroll.get_visible()
                assert not visible_text(app.content), "Empty search must not contain instruction labels"
                screenshot("search-empty")
                app.entry.set_text("%")
                spin(.3)
                assert len(app.results.get_children()) == 3, "A bare prefix must show candidates"
                screenshot("search-prefix")
                app.entry.set_text("% editor")
                spin(.5)
                assert app.results.get_row_at_index(0).item["kind"] == "window"
                screenshot("search-results")
                keyboard = subprocess.Popen(["wtype", "-s", "150", "-M", "ctrl", "-k", "bracketleft", "-m", "ctrl", "-s", "150"])
                spin(.5)
                assert keyboard.wait(timeout=2) == 0
                assert not app.overlay.get_visible(), "Ctrl+[ must close the palette"
                app.show("search", "$ draft")
                spin(.5)
                row = app.results.get_row_at_index(0)
                assert row is not None and row.item["kind"] == "clip"
                app.entry.set_text("")
                spin(.2)
                assert not app.results.get_children() and not app.scroll.get_visible()
                app.show("overview")
                spin(.5)
                assert len(app.grid.get_children()) == 3
                assert app.content.get_allocated_width() <= width, "Overview must fit the logical screen"
                assert app.content.get_allocated_height() + app.content.get_margin_top() <= height, "Overview must fit vertically"
                screenshot("overview")
                assert not any(text in {"Windows", "Esc", "A view across your workspaces", "Preview unavailable"}
                               or "Arrows to navigate" in text for text in visible_text(app.content))
                # Exercise the image decode/fit path with a known PNG fixture.
                from gi.repository import GdkPixbuf, Gtk
                pixbuf = GdkPixbuf.Pixbuf.new(GdkPixbuf.Colorspace.RGB, False, 8, 640, 320)
                pixbuf.fill(0x222730ff)
                _, png = pixbuf.save_to_bufferv("png", [], [])
                capture = backend.capture_preview
                backend.capture_preview = lambda _: png
                image = Gtk.Image()
                app.request_preview(fixtures[0], image, app.search_generation)
                spin(.2)
                backend.capture_preview = capture
                fitted = image.get_pixbuf()
                assert fitted is not None and fitted.get_width() <= app.preview_width
                assert abs(fitted.get_width() / fitted.get_height() - 2) < .02, "Previews must preserve aspect ratio"
                app.entry.set_text("Zen")
                spin()
                assert len(app.grid.get_children()) == 1
                assert app.grid.get_child_at_index(0).item["value"] == "0xdef"
                query = backend.hypr_json
                def delayed_query(_):
                    time.sleep(.2)
                    return fixtures
                backend.hypr_json = delayed_query
                app.show("overview")
                app.entry.set_text("Zen")
                spin(.4)
                assert len(app.grid.get_children()) == 1, "Typing while loading must not discard the window list"
                backend.hypr_json = query
                app.show("audio")
                spin()
                screenshot("sound")
                app.close()

                # Exercise the actual exported notification API via the session bus.
                replies = []
                bus = Gio.bus_get_sync(Gio.BusType.SESSION, None)
                bus.call("org.freedesktop.Notifications", "/org/freedesktop/Notifications",
                         "org.freedesktop.Notifications", "Notify",
                         GLib.Variant("(susssasa{sv}i)", ("Build", 0, "", "Ready to review", "All checks passed. Your workspace is up to date.", [], {}, 1000)),
                         GLib.VariantType.new("(u)"), Gio.DBusCallFlags.NONE, 2000, None,
                         lambda connection, result: replies.append(connection.call_finish(result).unpack()))
                spin(.2)
                assert len(replies) == 1
                assert len(app.notifications) == 1
                item = next(iter(app.notifications.values()))
                assert 0 < item["progress"].get_fraction() < 1
                screenshot("notification")
                spin(.9)
                assert not app.notifications
                assert app.notification_timer == 0
                assert json.loads((app.runtime / "notifications.json").read_text())["count"] == 1
                app.state.show()
                app.sync_bar()
                spin(2.1)
                screenshot("bar-notification")
                app.show("notifications")
                spin()
                assert json.loads((app.runtime / "notifications.json").read_text())["count"] == 0
                app.close()
                print("PASS: bar without modal panel, empty search, Ctrl+[, search, overview, level control, notification DBus/progress/expiry")
            finally:
                if app:
                    app.shutdown()
                sway.terminate()
                sway.wait(timeout=5)


if __name__ == "__main__":
    main()
