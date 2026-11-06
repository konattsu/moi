import gi
gi.require_version("Gtk", "3.0")
gi.require_version("Gdk", "3.0")
gi.require_version("GtkLayerShell", "0.1")
gi.require_version("GdkPixbuf", "2.0")
from gi.repository import GdkPixbuf, GLib, Gtk, Pango
import backend

def style(widget, name):
    widget.get_style_context().add_class(name)
    return widget


def label(text, css=None):
    widget = Gtk.Label(label=text, xalign=0)
    widget.set_ellipsize(Pango.EllipsizeMode.END)
    if css:
        style(widget, css)
    return widget


def box(vertical=True, spacing=8):
    return Gtk.Box(orientation=Gtk.Orientation.VERTICAL if vertical else Gtk.Orientation.HORIZONTAL,
                   spacing=spacing)


def icon(name, size=18):
    names = {
        "system-search-symbolic": "search", "focus-windows-symbolic": "dashboard",
        "application-x-executable-symbolic": "dashboard", "edit-paste-symbolic": "content_paste",
        "web-browser-symbolic": "language", "utilities-terminal-symbolic": "terminal",
        "audio-speakers-symbolic": "settings", "system-lock-screen-symbolic": "lock",
        "weather-clear-night-symbolic": "dark_mode", "system-reboot-symbolic": "restart_alt",
        "system-shutdown-symbolic": "power_settings_new", "go-previous-symbolic": "close",
        "window-close-symbolic": "close", "notifications-none-symbolic": "notifications",
    }
    path = backend.ROOT / "generated/icons" / (names.get(name, name) + ".svg")
    if path.is_file():
        try:
            pixbuf = GdkPixbuf.Pixbuf.new_from_file_at_scale(str(path), size, size, True)
            return Gtk.Image.new_from_pixbuf(pixbuf)
        except GLib.Error:
            pass
    image = Gtk.Image.new_from_icon_name(name, Gtk.IconSize.MENU)
    image.set_pixel_size(size)
    return image
