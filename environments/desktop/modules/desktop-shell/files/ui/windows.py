from gi.repository import GdkPixbuf, GLib, Gtk
import backend
from .widgets import box, icon, label, style

class WindowsView:
    def cancel_previews(self):
        for future in self.preview_futures:
            future.cancel()
        self.preview_futures.clear()

    def build_overview(self):
        controls = style(box(False, 12), "overview-controls")
        controls.pack_start(icon("system-search-symbolic"), False, False, 0)
        self.entry = Gtk.Entry()
        self.entry.set_width_chars(1)
        self.entry.get_accessible().set_name("Filter windows by title or application")
        controls.pack_start(self.entry, True, True, 0)
        self.content.pack_start(controls, False, False, 0)
        self.overview_scroll = Gtk.ScrolledWindow()
        self.overview_scroll.set_policy(Gtk.PolicyType.NEVER, Gtk.PolicyType.AUTOMATIC)
        self.overview_scroll.set_propagate_natural_height(True)
        self.overview_limit = max(120, min(600, self.screen_height - self.content.get_margin_top() - 240))
        self.overview_scroll.set_max_content_height(self.overview_limit)
        self.grid = style(Gtk.FlowBox(), "window-grid")
        self.grid.set_selection_mode(Gtk.SelectionMode.SINGLE)
        self.grid.set_activate_on_single_click(True)
        columns = max(1, min(3, (self.surface_width - 32) // 260))
        self.grid.set_min_children_per_line(1)
        self.grid.set_max_children_per_line(columns)
        self.grid.set_column_spacing(12)
        self.grid.set_row_spacing(12)
        self.grid.set_homogeneous(True)
        self.grid.set_valign(Gtk.Align.START)
        self.grid.connect("child-activated", lambda _, child: self.activate_item(child.item))
        self.overview_scroll.add(self.grid)
        self.content.pack_start(self.overview_scroll, False, False, 0)
        self.window_data = []
        self.preview_width = max(120, (self.surface_width - 34 - (columns - 1) * 12) // columns - 26)
        self.entry.connect("changed", lambda _: self.render_overview())
        self.entry.connect("activate", lambda _: self.choose_overview())
        request = self.overview_request = object()
        def loaded(windows):
            if self.mode == "overview" and request is self.overview_request:
                self.window_data = sorted(windows, key=lambda w: (w.get("workspace", {}).get("id", 0),
                                                                  w.get("focusHistoryID", 999)))
                self.render_overview()
        self.later(lambda: backend.hypr_json("clients"), loaded)

    def render_overview(self):
        self.search_generation += 1
        generation = self.search_generation
        self.cancel_previews()
        for child in self.grid.get_children():
            child.destroy()
        query = self.entry.get_text().strip().casefold()
        windows = [w for w in self.window_data if query in (w.get("title", "") + " " + w.get("class", "")).casefold()]
        for index, window in enumerate(windows):
            child = Gtk.FlowBoxChild()
            child.item = {"kind": "window", "value": window["address"]}
            card = style(box(spacing=0), "window-card")
            preview = style(box(), "window-preview")
            preview.set_size_request(self.preview_width, round(self.preview_width * .56))
            fallback = "utilities-terminal-symbolic" if "kitty" in window.get("class", "") else "web-browser-symbolic"
            image = icon(fallback, 24)
            preview.pack_start(image, True, True, 0)
            image.get_accessible().set_description("Preview unavailable")
            card.pack_start(preview, False, False, 0)
            texts = style(box(spacing=5), "window-caption")
            texts.pack_start(label(window.get("title") or window.get("class", "Window"), "result-title"), False, False, 0)
            texts.pack_start(label(backend.window_detail(window), "caption"), False, False, 0)
            card.pack_start(texts, False, False, 0)
            child.add(card)
            self.grid.add(child)
            if index < 24:
                self.request_preview(window, image, generation)
        self.grid.show_all()
        _, natural_height = self.grid.get_preferred_height_for_width(self.surface_width - 2)
        viewport_height = max(32, min(self.overview_limit, natural_height))
        self.overview_scroll.set_min_content_height(0)
        self.overview_scroll.set_max_content_height(viewport_height)
        self.overview_scroll.set_min_content_height(viewport_height)
        first = self.grid.get_child_at_index(0)
        if first:
            self.grid.select_child(first)

    def request_preview(self, window, image, generation):
        future = self.preview_pool.submit(backend.capture_preview, window)
        self.preview_futures.append(future)
        def finish(future):
            if future.cancelled() or self.stopping:
                return
            try:
                data = future.result()
            except Exception:
                return
            def display():
                if self.mode != "overview" or generation != self.search_generation:
                    return
                if not data:
                    image.get_accessible().set_description("Preview unavailable")
                    return
                try:
                    loader = GdkPixbuf.PixbufLoader.new_with_type("png")
                    def fit(loader, width, height):
                        factor = min(self.preview_width / width, (self.preview_width * .56) / height, 1)
                        loader.set_size(max(1, round(width * factor)), max(1, round(height * factor)))
                    loader.connect("size-prepared", fit)
                    loader.write(data)
                    loader.close()
                    image.set_from_pixbuf(loader.get_pixbuf())
                    image.get_accessible().set_description("")
                except GLib.Error:
                    pass
            GLib.idle_add(display)
        future.add_done_callback(finish)

    def choose_overview(self):
        children = self.grid.get_selected_children()
        if children:
            self.activate_item(children[0].item)
