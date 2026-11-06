import subprocess

from gi.repository import Gio, GLib, Gtk
import backend
from .widgets import box, icon, label, style

class SearchView:
    def build_search(self, prefix):
        search = style(box(False, 16), "search-input")
        search.pack_start(icon("system-search-symbolic", 18), False, False, 0)
        self.entry = Gtk.Entry()
        self.entry.set_width_chars(1)
        self.entry.get_accessible().set_name("Search applications, windows, clipboard and the web")
        self.entry.set_hexpand(True)
        search.pack_start(self.entry, True, True, 0)
        self.content.pack_start(search, False, False, 0)
        self.scroll = Gtk.ScrolledWindow()
        self.scroll.set_policy(Gtk.PolicyType.NEVER, Gtk.PolicyType.AUTOMATIC)
        self.scroll.set_propagate_natural_height(True)
        self.scroll.set_max_content_height(max(120, min(420, self.screen_height - 280)))
        self.scroll.set_no_show_all(True)
        self.results = Gtk.ListBox(selection_mode=Gtk.SelectionMode.SINGLE)
        self.results.set_activate_on_single_click(True)
        self.results.connect("row-activated", self.choose)
        self.scroll.add(self.results)
        self.content.pack_start(self.scroll, False, False, 0)
        self.entry.connect("changed", self.schedule_search)
        self.entry.connect("activate", lambda _: self.choose(self.results, self.results.get_selected_row()))
        self.entry.set_text(prefix)
        self.schedule_search()

    def clear_results(self):
        for child in self.results.get_children():
            child.destroy()
        self.scroll.hide()

    def schedule_search(self, *_):
        self.search_generation += 1
        if self.search_timer:
            GLib.source_remove(self.search_timer)
            self.search_timer = 0
        self.clear_results()
        text = self.entry.get_text().strip()
        if text:
            self.search_timer = GLib.timeout_add(80, self.search)

    def search(self):
        self.search_timer = 0
        if self.mode != "search":
            return GLib.SOURCE_REMOVE
        generation, text = self.search_generation, self.entry.get_text()
        applications = sorted((app for app in Gio.AppInfo.get_all() if app.should_show() and app.get_id()),
                              key=lambda app: app.get_display_name().casefold())
        self.app_lookup = {app.get_id(): app for app in applications}
        apps = [{"id": app.get_id(), "name": app.get_display_name(),
                 "search": app.get_display_name() + " " + (app.get_description() or "")}
                for app in applications]
        def collect():
            windows, clips = [], []
            warnings = []
            if text.lstrip()[:1] not in ("@", "&", "$"):
                try:
                    windows = backend.hypr_json("clients")
                except (OSError, ValueError, subprocess.SubprocessError):
                    warnings.append("Window search is unavailable")
            if text.lstrip()[:1] not in ("@", "&", "%"):
                try:
                    clips = backend.run("cliphist", "list").decode(errors="replace").splitlines()
                except (OSError, subprocess.SubprocessError):
                    warnings.append("Clipboard history is unavailable")
            return backend.search_rows(text, apps, windows, clips, self.settings["search_url"]), warnings
        def display(result):
            if generation != self.search_generation or self.mode != "search":
                return
            rows, warnings = result
            self.clear_results()
            for item in rows:
                row = Gtk.ListBoxRow()
                row.item = item
                row.add(self.result_content(item))
                self.results.add(row)
            if rows:
                self.scroll.show()
                self.scroll.get_child().show_all()
                self.results.select_row(self.results.get_row_at_index(0))
            self.results.get_accessible().set_description(" · ".join(warnings))
        self.later(collect, display)
        return GLib.SOURCE_REMOVE

    def result_content(self, item):
        row = style(box(False, 14), "result-content")
        names = {"window": "focus-windows-symbolic", "app": "application-x-executable-symbolic",
                 "clip": "edit-paste-symbolic", "web": "web-browser-symbolic"}
        image = icon(names[item["kind"]], 18)
        if item["kind"] == "app":
            app = self.app_lookup.get(item["value"])
            if app and app.get_icon():
                image = Gtk.Image.new_from_gicon(app.get_icon(), Gtk.IconSize.LARGE_TOOLBAR)
        row.pack_start(image, False, False, 0)
        texts = box(spacing=4)
        texts.pack_start(label(item["label"].replace("\n", " "), "result-title"), False, False, 0)
        if item["detail"]:
            texts.pack_start(label(item["detail"], "caption"), False, False, 0)
        row.pack_start(texts, True, True, 0)
        return row

    def choose(self, _, row):
        if row is not None:
            self.activate_item(row.item)

    def activate_item(self, item):
        self.close()
        self.later(lambda: backend.activate(item, getattr(self, "app_lookup", {})))
