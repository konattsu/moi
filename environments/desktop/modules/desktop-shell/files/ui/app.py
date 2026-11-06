from concurrent.futures import ThreadPoolExecutor
import json
import os
from pathlib import Path
import signal
import subprocess
import sys

from .widgets import box, icon, label, style
from gi.repository import Gdk, Gio, GLib, Gtk, GtkLayerShell
from .search import SearchView
from .windows import WindowsView
from .controls import ControlsView
from .notifications import Notifications
import backend


class Desktop(SearchView, WindowsView, ControlsView, Notifications, Gtk.Application):
    def __init__(self):
        super().__init__(application_id="io.github.konattsu.MoiDesktop",
                         flags=Gio.ApplicationFlags.HANDLES_COMMAND_LINE)
        self.state = backend.BarState()
        self.pool = ThreadPoolExecutor(max_workers=3)
        self.preview_pool = ThreadPoolExecutor(max_workers=2)
        self.preview_futures = []
        self.children = []
        self.notifications = {}
        self.history = []
        self.unread = set()
        self.notification_timer = 0
        self.next_notification = 1
        self.search_generation = 0
        self.search_timer = 0
        self.level_timer = 0
        self.mode = ""
        self.stopping = False

    def do_startup(self):
        Gtk.Application.do_startup(self)
        self.hold()
        self.settings = json.loads((backend.ROOT / "config/settings.json").read_text())
        instance = os.environ.get("HYPRLAND_INSTANCE_SIGNATURE", "default")
        if not all(c.isalnum() or c in "_-" for c in instance):
            raise ValueError("Invalid compositor instance")
        self.runtime = Path(os.environ["XDG_RUNTIME_DIR"]) / ("moi-desktop-" + instance)
        self.runtime.mkdir(mode=0o700, exist_ok=True)
        self.scratchpad = backend.Scratchpad(self.runtime, self.settings["scratchpad"])
        css = Gtk.CssProvider()
        css.load_from_path(str(backend.ROOT / "generated/shell.css"))
        Gtk.StyleContext.add_provider_for_screen(Gdk.Screen.get_default(), css,
                                                Gtk.STYLE_PROVIDER_PRIORITY_APPLICATION)
        if self.settings.get("reduced_motion"):
            reduced = Gtk.CssProvider()
            reduced.load_from_data(b"* { transition: none; }")
            Gtk.StyleContext.add_provider_for_screen(Gdk.Screen.get_default(), reduced,
                                                    Gtk.STYLE_PROVIDER_PRIORITY_APPLICATION + 1)
        self.build_overlay()
        self.build_notifications()
        self.bar_ready = False
        self.bar = self.child("waybar", "--config", str(backend.ROOT / "generated/waybar.json"),
                              "--style", str(backend.ROOT / "generated/waybar.css"))
        GLib.timeout_add(1000, self.ready_bar)
        self.child("wl-paste", "--watch", "cliphist", "store")
        self.child("fcitx5")
        self.child("hypridle", "--config", str(backend.ROOT / "config/hypridle.conf"))
        wallpaper = self.settings.get("wallpaper")
        if wallpaper:
            path = Path(wallpaper).expanduser()
            path = path if path.is_absolute() else backend.ROOT / path
            if path.is_file():
                self.child("swaybg", "-i", str(path), "-m", "fill")
        self.own_notifications()
        self.save_badge()
        self.connect("shutdown", self.shutdown)

    def child(self, *args):
        process = subprocess.Popen(args)
        self.children.append(process)
        return process

    def ready_bar(self):
        self.bar_ready = True
        self.sync_bar()
        self.refresh_badge()
        return GLib.SOURCE_REMOVE

    def sync_bar(self):
        if self.bar_ready and self.bar.poll() is None:
            try:
                self.bar.send_signal(signal.SIGUSR1 if self.state.bar else signal.SIGUSR2)
            except ProcessLookupError:
                pass

    def later(self, function, callback=None):
        def finish(future):
            if self.stopping:
                return
            try:
                result = future.result()
            except Exception as error:
                GLib.idle_add(self.error, str(error))
            else:
                if callback:
                    GLib.idle_add(callback, result)
        self.pool.submit(function).add_done_callback(finish)

    def error(self, message):
        print(message, file=sys.stderr)
        if not self.stopping:
            self.notify("Moi Desktop", "Unable to complete the action", message)
        return GLib.SOURCE_REMOVE

    def layer_window(self, namespace, keyboard=False):
        window = Gtk.Window()
        self.add_window(window)
        window.set_app_paintable(True)
        visual = window.get_screen().get_rgba_visual()
        if visual:
            window.set_visual(visual)
        GtkLayerShell.init_for_window(window)
        GtkLayerShell.set_namespace(window, namespace)
        GtkLayerShell.set_layer(window, GtkLayerShell.Layer.OVERLAY)
        GtkLayerShell.set_exclusive_zone(window, 0)
        GtkLayerShell.set_keyboard_mode(window, GtkLayerShell.KeyboardMode.EXCLUSIVE
                                      if keyboard else GtkLayerShell.KeyboardMode.NONE)
        return window

    def build_overlay(self):
        self.overlay = self.layer_window("moi-launcher", keyboard=True)
        for edge in (GtkLayerShell.Edge.TOP, GtkLayerShell.Edge.BOTTOM,
                     GtkLayerShell.Edge.LEFT, GtkLayerShell.Edge.RIGHT):
            GtkLayerShell.set_anchor(self.overlay, edge, True)
        self.background = Gtk.EventBox()
        self.background.set_visible_window(False)
        self.background.add_events(Gdk.EventMask.BUTTON_PRESS_MASK)
        self.background.connect("button-press-event", self.outside_click)
        self.overlay.add(self.background)
        self.content = style(box(spacing=0), "surface")
        self.content.set_valign(Gtk.Align.START)
        self.background.add(self.content)
        self.overlay.connect("key-press-event", self.key_press)

    def outside_click(self, _, event):
        allocation = self.content.get_allocation()
        if not (allocation.x <= event.x < allocation.x + allocation.width and
                allocation.y <= event.y < allocation.y + allocation.height):
            self.close()
            return True
        return False

    def close(self):
        self.search_generation += 1
        self.cancel_previews()
        self.overlay.hide()
        self.mode = ""
        self.state.close()
        self.sync_bar()

    def dismiss_bar(self):
        # Chords close only the bar, never the command palette they just opened.
        if not self.mode:
            self.state.close()
            self.sync_bar()

    def show(self, mode, prefix=""):
        self.search_generation += 1
        self.cancel_previews()
        for widget in self.content.get_children():
            widget.destroy()
        self.mode = mode
        geometry = Gdk.Display.get_default().get_monitor(0).get_geometry()
        self.screen_height = geometry.height
        wide = mode == "overview"
        detail = mode in ("audio", "brightness", "notifications", "session", "confirm")
        self.surface_width = min(1080 if wide else (360 if detail else 720), geometry.width - 48)
        self.content.set_size_request(self.surface_width, -1)
        self.content.set_margin_top(56 if detail else max(48, round(geometry.height * .16)))
        self.content.set_margin_end(24 if detail else 0)
        self.content.set_halign(Gtk.Align.END if detail else Gtk.Align.CENTER)
        if detail:
            self.state.show()
        else:
            self.state.close()
        if mode == "search":
            self.build_search(prefix)
        elif mode == "overview":
            self.build_overview()
        elif mode in ("audio", "brightness"):
            self.build_level(mode)
        elif mode == "notifications":
            self.build_history()
        elif mode == "session":
            self.build_session()
        self.overlay.show_all()
        self.sync_bar()
        if mode in ("search", "overview"):
            self.entry.grab_focus()
            self.entry.set_position(-1)

    def key_press(self, _, event):
        key = Gdk.keyval_name(event.keyval)
        if key == "Escape" or (event.state & Gdk.ModifierType.CONTROL_MASK and key in ("bracketleft", "braceleft")):
            self.close()
            return True
        if self.mode == "search" and key in ("Up", "Down"):
            selected = self.results.get_selected_row()
            index = selected.get_index() if selected else -1
            row = self.results.get_row_at_index(max(0, index + (-1 if key == "Up" else 1)))
            if row:
                self.results.select_row(row)
                allocation = row.get_allocation()
                adjustment = self.scroll.get_vadjustment()
                value, page = adjustment.get_value(), adjustment.get_page_size()
                if allocation.y < value:
                    adjustment.set_value(allocation.y)
                elif allocation.y + allocation.height > value + page:
                    adjustment.set_value(allocation.y + allocation.height - page)
            return True
        if self.mode == "overview" and key == "Down" and self.entry.has_focus():
            child = self.grid.get_child_at_index(0)
            if child:
                child.grab_focus()
            return True
        return False

    def do_command_line(self, command_line):
        args = command_line.get_arguments()[1:]
        action = args[0] if args else "start"
        if action == "bar":
            self.close() if self.mode else self.state.toggle()
            self.sync_bar()
        elif action == "dismiss-bar":
            self.dismiss_bar()
        elif action == "close":
            self.close()
        elif action == "search":
            self.show("search", args[1] if len(args) > 1 else "")
        elif action in ("overview", "audio", "brightness", "notifications", "session"):
            self.show(action)
        elif action in ("wifi", "bluetooth"):
            self.close()
            backend.spawn(*(("kitty", "-e", "nmtui-connect") if action == "wifi" else ("blueman-manager",)))
        elif action in ("volume-up", "volume-down", "mute", "brightness-up", "brightness-down"):
            self.adjust(action)
        elif action in ("toggle", "tile", "adopt"):
            self.close()
            self.later(lambda: self.scratchpad.execute(action))
        elif action == "normal-opened" and len(args) == 2:
            self.close()
            self.later(lambda: backend.focus_terminal(args[1]))
        elif action == "lock":
            self.close()
            self.lock()
        elif action == "quit":
            self.quit()
        elif action != "start":
            return 2
        return 0

    def shutdown(self, *_):
        if self.stopping:
            return
        self.stopping = True
        self.cancel_previews()
        for process in self.children:
            if process.poll() is None:
                process.terminate()
        for process in self.children:
            try:
                process.wait(timeout=1)
            except subprocess.TimeoutExpired:
                process.kill()
        self.pool.shutdown(wait=False, cancel_futures=True)
        self.preview_pool.shutdown(wait=False, cancel_futures=True)
        Gio.bus_unown_name(self.bus_owner)
