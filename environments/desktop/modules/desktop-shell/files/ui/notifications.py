import json
import signal
import sys
import time

from gi.repository import Gdk, Gio, GLib, Gtk, GtkLayerShell, Pango
import backend
from .widgets import box, icon, label, style

NOTIFICATION_XML = """<node><interface name="org.freedesktop.Notifications">
<method name="GetCapabilities"><arg type="as" direction="out"/></method>
<method name="GetServerInformation"><arg type="s" direction="out"/><arg type="s" direction="out"/><arg type="s" direction="out"/><arg type="s" direction="out"/></method>
<method name="Notify"><arg type="s" direction="in"/><arg type="u" direction="in"/><arg type="s" direction="in"/><arg type="s" direction="in"/><arg type="s" direction="in"/><arg type="as" direction="in"/><arg type="a{sv}" direction="in"/><arg type="i" direction="in"/><arg type="u" direction="out"/></method>
<method name="CloseNotification"><arg type="u" direction="in"/></method>
<signal name="NotificationClosed"><arg type="u"/><arg type="u"/></signal>
<signal name="ActionInvoked"><arg type="u"/><arg type="s"/></signal>
</interface></node>"""


class Notifications:
    def build_notifications(self):
        self.toasts = self.layer_window("moi-notifications")
        GtkLayerShell.set_anchor(self.toasts, GtkLayerShell.Edge.TOP, True)
        GtkLayerShell.set_anchor(self.toasts, GtkLayerShell.Edge.RIGHT, True)
        GtkLayerShell.set_margin(self.toasts, GtkLayerShell.Edge.TOP, 56)
        GtkLayerShell.set_margin(self.toasts, GtkLayerShell.Edge.RIGHT, 24)
        self.toast_box = box(spacing=12)
        self.toasts.add(self.toast_box)

    def own_notifications(self):
        def acquired(connection, _):
            self.bus = connection
            info = Gio.DBusNodeInfo.new_for_xml(NOTIFICATION_XML)
            self.registration = connection.register_object("/org/freedesktop/Notifications", info.interfaces[0], self.dbus_call, None, None)
        def lost(*_):
            print("Notification service unavailable: stop any other notification daemon.", file=sys.stderr)
        self.bus_owner = Gio.bus_own_name(Gio.BusType.SESSION, "org.freedesktop.Notifications", Gio.BusNameOwnerFlags.DO_NOT_QUEUE,
                                         acquired, None, lost)

    def dbus_call(self, connection, sender, path, interface, method, parameters, invocation):
        if method == "GetCapabilities":
            invocation.return_value(GLib.Variant("(as)", (["body", "actions"],)))
        elif method == "GetServerInformation":
            invocation.return_value(GLib.Variant("(ssss)", ("Moi Desktop", "moi", "2", "1.2")))
        elif method == "CloseNotification":
            self.dismiss(parameters.unpack()[0], 3)
            invocation.return_value(None)
        elif method == "Notify":
            app, replace, image, summary, body, actions, hints, timeout = parameters.unpack()
            ident = self.notify(app, summary, body, timeout, replace, actions, hints.get("urgency", 1))
            invocation.return_value(GLib.Variant("(u)", (ident,)))

    def notify(self, app, summary, body, timeout=-1, replace=0, actions=(), urgency=1):
        ident = replace if replace in self.notifications else self.next_notification
        if ident == self.next_notification:
            self.next_notification += 1
        if ident in self.notifications:
            self.notifications[ident]["widget"].destroy()
        timeout = self.settings["notification_timeout_ms"] if timeout < 0 else timeout
        if urgency == 2:
            timeout = 0
        widget = style(box(spacing=0), "notification")
        if urgency == 2:
            style(widget, "critical")
        toast_width = min(352, Gdk.Display.get_default().get_monitor(0).get_geometry().width - 48)
        widget.set_size_request(toast_width, -1)
        body_box = style(box(spacing=8), "notification-body")
        header = box(False)
        header.pack_start(label(app or "Notification", "caption"), True, True, 0)
        close = style(Gtk.Button(), "icon-button")
        close.add(icon("window-close-symbolic", 14))
        close.get_accessible().set_name("Dismiss notification")
        close.connect("clicked", lambda _: self.dismiss(ident, 2))
        header.pack_end(close, False, False, 0)
        body_box.pack_start(header, False, False, 0)
        for text, css in ((summary, "result-title"), (body, "notification-text")):
            if text:
                line = label(text, css)
                line.set_line_wrap(True)
                line.set_line_wrap_mode(Pango.WrapMode.WORD_CHAR)
                line.set_max_width_chars(max(16, (toast_width - 38) // 7))
                line.set_lines(4)
                body_box.pack_start(line, False, False, 0)
        if actions:
            buttons = box(False)
            for key, text in zip(actions[::2], actions[1::2]):
                button = style(Gtk.Button(label=text), "notification-action")
                button.connect("clicked", lambda _, k=key: self.notification_action(ident, k))
                buttons.pack_start(button, False, False, 0)
            body_box.pack_start(buttons, False, False, 0)
        widget.pack_start(body_box, False, False, 0)
        progress = Gtk.ProgressBar()
        progress.set_fraction(1)
        if timeout:
            widget.pack_start(progress, False, False, 0)
        self.toast_box.pack_start(widget, False, False, 0)
        self.notifications[ident] = {"widget": widget, "progress": progress, "start": time.monotonic(), "timeout": timeout}
        self.history = [item for item in self.history if item["id"] != ident]
        self.history.insert(0, {"id": ident, "app": app, "summary": summary, "body": body})
        del self.history[50:]
        self.unread.add(ident)
        self.unread.intersection_update(item["id"] for item in self.history)
        self.save_badge()
        self.toasts.show_all()
        if timeout and not self.notification_timer:
            self.notification_timer = GLib.timeout_add(40, self.tick_notifications)
        return ident

    def build_history(self):
        self.unread.clear()
        self.save_badge()
        scroll = Gtk.ScrolledWindow()
        scroll.set_policy(Gtk.PolicyType.NEVER, Gtk.PolicyType.AUTOMATIC)
        scroll.set_propagate_natural_height(True)
        scroll.set_max_content_height(min(480, self.screen_height - 190))
        contents = style(box(spacing=20), "detail-body")
        if not self.history:
            contents.pack_start(icon("notifications-none-symbolic", 24), False, False, 0)
        for item in self.history:
            entry = box(spacing=6)
            entry.pack_start(label(item["app"], "caption"), False, False, 0)
            for text, css in ((item["summary"], "result-title"), (item["body"], "notification-text")):
                if text:
                    line = label(text, css)
                    line.set_max_width_chars(30)
                    line.set_line_wrap(True)
                    line.set_line_wrap_mode(Pango.WrapMode.WORD_CHAR)
                    entry.pack_start(line, False, False, 0)
            contents.pack_start(entry, False, False, 0)
        scroll.add(contents)
        self.content.pack_start(scroll, False, False, 0)

    def save_badge(self):
        count = len(self.unread)
        backend.write_output(self.runtime / "notifications.json", json.dumps({"count": count}))
        if count != getattr(self, "_badge_count", None):
            self._badge_count = count
            self.refresh_badge()

    def refresh_badge(self):
        if self.bar_ready and self.bar.poll() is None:
            try:
                self.bar.send_signal(signal.SIGRTMIN + 3)
            except ProcessLookupError:
                pass

    def notification_action(self, ident, action):
        if hasattr(self, "bus"):
            self.bus.emit_signal(None, "/org/freedesktop/Notifications", "org.freedesktop.Notifications",
                                 "ActionInvoked", GLib.Variant("(us)", (ident, action)))
        self.dismiss(ident, 2)

    def dismiss(self, ident, reason):
        if reason == 2:
            self.unread.discard(ident)
        item = self.notifications.pop(ident, None)
        if item:
            item["widget"].destroy()
            if hasattr(self, "bus"):
                self.bus.emit_signal(None, "/org/freedesktop/Notifications", "org.freedesktop.Notifications",
                                     "NotificationClosed", GLib.Variant("(uu)", (ident, reason)))
            self.save_badge()
        if not self.notifications:
            self.toasts.hide()

    def tick_notifications(self):
        for ident, item in list(self.notifications.items()):
            if item["timeout"]:
                remaining = 1 - (time.monotonic() - item["start"]) * 1000 / item["timeout"]
                if remaining <= 0:
                    self.dismiss(ident, 1)
                else:
                    item["progress"].set_fraction(remaining)
        if not any(item["timeout"] for item in self.notifications.values()):
            self.notification_timer = 0
            return GLib.SOURCE_REMOVE
        return GLib.SOURCE_CONTINUE
