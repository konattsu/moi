from gi.repository import GLib, Gtk
import backend
from .widgets import box, icon, label, style

class ControlsView:
    def build_level(self, kind):
        generation = self.search_generation
        body = style(box(spacing=16), "detail-body")
        readout = label("—", "level-value")
        body.pack_start(readout, False, False, 0)
        scale = Gtk.Scale.new_with_range(Gtk.Orientation.HORIZONTAL, 0 if kind == "audio" else 1, 100, 1)
        scale.set_draw_value(False)
        scale.get_accessible().set_name("Volume" if kind == "audio" else "Screen brightness")
        body.pack_start(scale, False, False, 0)
        def loaded(value):
            if self.mode != kind or generation != self.search_generation:
                return
            scale.set_value(value)
            readout.set_text(f"{value:.0f}%")
            def changed(widget):
                value = round(widget.get_value())
                readout.set_text(f"{value}%")
                if self.level_timer:
                    GLib.source_remove(self.level_timer)
                def apply():
                    self.level_timer = 0
                    self.later(lambda: backend.set_level(kind, value))
                    return GLib.SOURCE_REMOVE
                self.level_timer = GLib.timeout_add(80, apply)
            scale.connect("value-changed", changed)
        self.content.pack_start(body, False, False, 0)
        self.later(lambda: backend.get_level(kind), loaded)
        if kind == "audio":
            self.action_row("Audio devices", "audio-speakers-symbolic", lambda: backend.spawn("pavucontrol"))

    def action_row(self, title, image, callback):
        button = style(Gtk.Button(), "action-row")
        contents = box(False, 12)
        contents.pack_start(icon(image), False, False, 0)
        contents.pack_start(label(title), True, True, 0)
        button.add(contents)
        def clicked(_):
            self.close()
            callback()
        button.connect("clicked", clicked)
        self.content.pack_start(button, False, False, 0)
        return button

    def build_session(self):
        self.action_row("Lock", "system-lock-screen-symbolic", self.lock)
        self.action_row("Sleep", "weather-clear-night-symbolic", lambda: backend.spawn("systemctl", "suspend"))
        self.action_row("Restart…", "system-reboot-symbolic", lambda: self.confirm("Restart", "reboot"))
        self.action_row("Shut down…", "system-shutdown-symbolic", lambda: self.confirm("Shut down", "poweroff"))

    def confirm(self, title, command):
        self.show("confirm")
        self.action_row("Cancel", "go-previous-symbolic", lambda: None)
        self.action_row(title, "system-shutdown-symbolic", lambda: backend.spawn("systemctl", command))
        self.content.show_all()

    def lock(self):
        backend.spawn("hyprlock", "--no-fade-in", "--config", str(backend.ROOT / "generated/hyprlock.conf"))

    def adjust(self, action):
        self.later(lambda: backend.control(action), lambda _: self.flash())

    def flash(self):
        generation = self.state.flash()
        self.sync_bar()
        def expire():
            self.state.expire(generation)
            self.sync_bar()
            return GLib.SOURCE_REMOVE
        GLib.timeout_add(self.settings["bar_timeout_ms"], expire)
