"""Run in the devcontainer: python3 -m unittest discover -s .../tests."""

import json
import os
from pathlib import Path
import re
import sys
import tempfile
import unittest
from unittest.mock import patch

FILES = Path(__file__).resolve().parents[1] / "files"
sys.path.insert(0, str(FILES))
import backend
from render_theme import palette
import status


class SearchTests(unittest.TestCase):
    def setUp(self):
        self.apps = [{"id": "zen.desktop", "name": "Zen", "search": "Zen Browser"}]
        self.windows = [{"address": "0xabc", "title": "日本語 — Zen", "class": "zen",
                         "workspace": {"name": "2"}}]
        self.clips = ["10\t日本語 copied", "11\t$(touch /tmp/should-not-exist)"]

    def rows(self, text):
        return backend.search_rows(text, self.apps, self.windows, self.clips,
                                   "https://example.org/?q={query}")

    def test_unprefixed_search_includes_every_category(self):
        self.assertEqual({row["kind"] for row in self.rows("e")}, {"app", "window", "clip", "web"})

    def test_prefixes_and_unicode(self):
        for query, kind in (("@ zen", "app"), ("% 日本語", "window"),
                            ("$ 日本語", "clip"), ("& 日本語", "web")):
            with self.subTest(query=query):
                self.assertEqual([row["kind"] for row in self.rows(query)], [kind])

    def test_web_query_is_encoded_not_interpreted(self):
        row = self.rows("& x&y #日本語")[0]
        self.assertEqual(row["value"], "https://example.org/?q=x%26y%20%23%E6%97%A5%E6%9C%AC%E8%AA%9E")
        with patch.object(backend, "spawn") as spawn:
            backend.activate(row, {})
        spawn.assert_called_once_with("xdg-open", row["value"])

    def test_window_addresses_cannot_inject_lua(self):
        with self.assertRaises(ValueError):
            backend.selector('0xabc"}); os.execute("bad")')

    def test_clipboard_selection_decodes_binary_and_promotes_without_pasting(self):
        row = self.rows("$ 日本語")[0]
        with patch.object(backend, "run", return_value=b"\x89PNG\x00") as run:
            backend.activate(row, {})
        self.assertEqual(run.call_args_list[0].args, ("cliphist", "decode"))
        self.assertEqual(run.call_args_list[1].args, ("wl-copy",))
        self.assertEqual(run.call_args_list[1].kwargs["data"], b"\x89PNG\x00")
        self.assertEqual(run.call_args_list[2].args, ("cliphist", "store"))

    def test_many_apps_do_not_hide_other_categories(self):
        self.apps *= 100
        self.assertEqual({row["kind"] for row in self.rows("e")}, {"app", "window", "clip", "web"})

    def test_only_empty_input_has_no_candidates(self):
        for text in ("", "  "):
            with self.subTest(text=text):
                self.assertEqual(self.rows(text), [])

    def test_bare_prefixes_show_their_scope_immediately(self):
        for text, expected in (("@", "app"), ("% ", "window"), ("$", "clip"), ("& ", "web")):
            with self.subTest(text=text):
                rows = self.rows(text)
                self.assertTrue(rows)
                self.assertEqual({row["kind"] for row in rows}, {expected})


class VisibilityTests(unittest.TestCase):
    def test_osd_timeout_does_not_close_pinned_bar(self):
        state = backend.BarState()
        state.show()
        state.expire(state.flash())
        self.assertTrue(state.open)
        self.assertTrue(state.bar)

    def test_old_timer_cannot_hide_new_osd_or_reopened_bar(self):
        state = backend.BarState()
        old = state.flash()
        new = state.flash()
        state.expire(old)
        self.assertTrue(state.bar)
        state.expire(new)
        self.assertFalse(state.bar)
        old = state.flash()
        state.close()
        state.show()
        state.expire(old)
        self.assertTrue(state.bar)

    def test_super_toggle_needs_no_panel_state(self):
        state = backend.BarState()
        state.toggle()
        self.assertTrue(state.bar)
        state.toggle()
        self.assertFalse(state.bar)


class ScratchTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.pad = backend.Scratchpad(self.temp.name)
        self.old = {"address": "0xabc", "pid": 123, "class": "moi-scratchpad",
                    "workspace": {"name": "special:moi-scratch"}}
        self.active = {"address": "0xdef", "pid": 456, "class": "kitty",
                       "workspace": {"name": "2"}}
        self.clients = [self.old, self.active]
        self.commands = []
        self.monitor = {"focused": True, "width": 2000, "height": 1200, "scale": 1.25,
                        "specialWorkspace": {"name": ""}}

    def query(self, what):
        return {"clients": self.clients, "activeworkspace": {"id": 2},
                "activewindow": self.active,
                "monitors": [self.monitor]}[what]

    def dispatch(self, command):
        self.commands.append(command)
        match = re.search(r'address:(0x[0-9a-f]+)', command)
        target = next((w for w in self.clients if match and w["address"] == match[1]), None)
        if target and "window.fullscreen_state" in command:
            self.assertIn("internal=0,client=0", command)
            target["fullscreen"] = target["fullscreenClient"] = 0
        if target and any(name in command for name in ("window.resize", "window.float")):
            if target.get("fullscreen", 0) or target.get("fullscreenClient", 0):
                raise RuntimeError("Window is fullscreen")
        if target and "window.resize" in command:
            size = re.search(r'x=(\d+),y=(\d+)', command)
            target["size"] = [int(size[1]), int(size[2])]
        if target and "window.move" in command:
            position = re.search(r'x=(\d+),y=(\d+)', command)
            if position:
                target["at"] = [int(position[1]), int(position[2])]
        if "window.close" in command:
            self.clients.remove(self.old)

    def execute(self, action):
        with patch.object(backend, "hypr_json", side_effect=self.query), \
             patch.object(backend, "dispatch", side_effect=self.dispatch):
            self.pad.execute(action)

    def test_tile_releases_scratch_ownership_without_closing_terminal(self):
        self.pad.remember(self.old)
        self.execute("tile")
        self.assertFalse(self.pad.path.exists())
        self.assertIn(self.old, self.clients)
        self.assertTrue(any("workspace=2" in c for c in self.commands))
        self.assertTrue(any('tag="-moi-scratch"' in c for c in self.commands))

    def test_toggle_after_tiling_creates_new_terminal(self):
        self.pad.remember(self.old)
        self.execute("tile")
        new = {"address": "0xaaa", "pid": 789, "class": "moi-scratchpad-test",
               "initialTitle": "Scratchpad", "workspace": {"name": "special:moi-scratch"}}
        self.clients.append(new)
        with patch.object(backend, "spawn") as spawn, patch.object(backend.uuid, "uuid4") as uuid:
            uuid.return_value.hex = "test"
            self.execute("toggle")
            spawn.assert_called_once_with("kitty", "--start-as", "normal", "--class", "moi-scratchpad-test", "--title", "Scratchpad")
        self.assertEqual(json.loads(self.pad.path.read_text())["address"], "0xaaa")
        self.assertIn(self.old, self.clients)

    def test_adopt_replaces_old_scratch_and_scales_size(self):
        self.pad.remember(self.old)
        self.execute("adopt")
        self.assertNotIn(self.old, self.clients)
        self.assertEqual(json.loads(self.pad.path.read_text())["address"], "0xdef")
        self.assertTrue(any("x=1280,y=768" in c for c in self.commands))
        self.assertTrue(any("x=160,y=24" in c for c in self.commands))
        self.assertLess(next(i for i, c in enumerate(self.commands) if "toggle_special" in c),
                        next(i for i, c in enumerate(self.commands) if "window.resize" in c))

    def test_non_terminal_adoption_never_closes_old_scratch(self):
        self.pad.remember(self.old)
        self.active["class"] = "zen"
        with self.assertRaises(RuntimeError):
            self.execute("adopt")
        self.assertEqual(self.commands, [])

    def test_reused_address_with_other_pid_is_not_owned(self):
        self.pad.remember(self.old)
        self.old["pid"] = 999
        with patch.object(backend, "hypr_json", side_effect=self.query):
            self.assertIsNone(self.pad.current())

    def test_reported_maximized_scratchpad_at_scale_two_is_recovered(self):
        self.monitor.update(width=2560, height=1600, scale=2, reserved=[0, 0, 0, 0])
        self.old.update(fullscreen=1, fullscreenClient=1, floating=True, size=[1278, 798], at=[1, 1])
        self.pad.remember(self.old)
        self.execute("toggle")
        self.assertEqual(self.old["fullscreen"], 0)
        self.assertEqual(self.old["fullscreenClient"], 0)
        self.assertEqual(self.old["size"], [1024, 640])
        self.assertEqual(self.old["at"], [128, 24])

    def test_adopting_fullscreen_terminal_clears_both_states_before_resize(self):
        self.active.update(fullscreen=2, fullscreenClient=2)
        self.execute("adopt")
        self.assertEqual(self.active["fullscreen"], 0)
        self.assertEqual(self.active["fullscreenClient"], 0)
        self.assertEqual(self.active["size"], [1280, 768])

    def test_tiling_maximized_scratchpad_clears_fullscreen_before_float(self):
        self.old.update(fullscreen=1, fullscreenClient=1)
        self.pad.remember(self.old)
        self.execute("tile")
        self.assertEqual(self.old["fullscreen"], 0)
        self.assertFalse(self.pad.path.exists())


class RenderTests(unittest.TestCase):
    def test_readable_text_and_selection_contrast(self):
        theme = palette(json.loads((FILES / "config/theme.json").read_text()))
        def luminance(color):
            values = [int(color[i:i+2], 16) / 255 for i in (1, 3, 5)]
            linear = [v / 12.92 if v <= .04045 else ((v + .055) / 1.055) ** 2.4 for v in values]
            return sum(v * weight for v, weight in zip(linear, (.2126, .7152, .0722)))
        for foreground, background in (("foreground", "surface"), ("muted", "surface"),
                                       ("foreground", "raised"), ("background", "accent")):
            a, b = sorted((luminance(theme[foreground]), luminance(theme[background])))
            self.assertGreaterEqual((b + .05) / (a + .05), 4.5, (foreground, background))

    def test_applications_run_in_independent_session_units(self):
        with patch.object(backend.subprocess, "Popen") as popen:
            backend.spawn("kitty")
        self.assertEqual(popen.call_args.args[0], ("uwsm", "app", "-t", "service", "--", "kitty"))

    def test_render_is_complete_and_does_not_rewrite_unchanged_themes(self):
        with tempfile.TemporaryDirectory() as output:
            backend.render(output)
            files = [p for p in Path(output).rglob("*") if p.is_file()]
            self.assertTrue((Path(output) / "waybar.json").is_file())
            self.assertTrue((Path(output) / "icons/battery_full.svg").is_file())
            self.assertIn('keyboard_layout = "jp"', (Path(output) / "settings.lua").read_text())
            self.assertIn("keyboard-jp", (Path(output) / "fcitx-profile").read_text())
            before = {p.name: p.stat().st_mtime_ns for p in files}
            backend.render(output)
            for path in files:
                self.assertNotIn("{{", path.read_text())
                if path.name in before:
                    self.assertEqual(path.stat().st_mtime_ns, before[path.name])


class WindowTests(unittest.TestCase):
    def test_scaled_rotated_monitor_uses_logical_top_edge(self):
        monitor = {"width": 1200, "height": 2000, "scale": 1.25, "transform": 1,
                   "x": 100, "y": 200, "reserved": [8, 32, 8, 8]}
        self.assertEqual(backend.scratch_geometry(monitor, {"width": .8, "height": .8, "top": 24}),
                         (266, 256, 1267, 736))

    def test_new_kitty_is_tiled_focused_and_not_hidden_by_scratchpad(self):
        def query(what):
            return [{"address": "0xabc", "class": "kitty"}] if what == "clients" else [
                {"focused": True, "specialWorkspace": {"name": "special:moi-scratch"}}]
        with patch.object(backend, "hypr_json", side_effect=query), patch.object(backend, "dispatch") as dispatch:
            backend.focus_terminal("0xabc")
        commands = [call.args[0] for call in dispatch.call_args_list]
        self.assertIn("toggle_special", commands[0])
        self.assertIn("fullscreen_state", commands[1])
        self.assertIn('action="disable"', commands[2])
        self.assertIn("hl.dsp.focus", commands[3])

    def test_preview_uses_stable_toplevel_id_and_never_screen_crops(self):
        with patch.object(backend.subprocess, "run") as run:
            run.return_value.returncode = 0
            run.return_value.stdout = b"\x89PNG\r\n\x1a\nfixture"
            self.assertIsNotNone(backend.capture_preview({"stableId": "stable-window"}))
        self.assertEqual(run.call_args.args[0][:3], ("grim", "-T", "stable-window"))
        self.assertEqual(run.call_args.kwargs["timeout"], 2)

    def test_missing_or_invalid_preview_id_does_not_capture_the_screen(self):
        with patch.object(backend.subprocess, "run") as run:
            self.assertIsNone(backend.capture_preview({}))
            self.assertIsNone(backend.capture_preview({"stableId": "--output=screen"}))
        run.assert_not_called()

    def test_preview_failure_is_an_explicit_fallback(self):
        with patch.object(backend.subprocess, "run", side_effect=backend.subprocess.TimeoutExpired("grim", 2)):
            self.assertIsNone(backend.capture_preview({"stableId": "window"}))


class BadgeTests(unittest.TestCase):
    def test_badge_is_a_number_without_a_text_prefix(self):
        with tempfile.TemporaryDirectory() as directory, patch.dict(os.environ, {
            "XDG_RUNTIME_DIR": directory, "HYPRLAND_INSTANCE_SIGNATURE": "fixture",
        }):
            folder = Path(directory) / "moi-desktop-fixture"
            folder.mkdir()
            (folder / "notifications.json").write_text('{"count": 37}')
            self.assertEqual(status.badge(), {"text": "37", "class": "pending"})
            (folder / "notifications.json").write_text("invalid")
            self.assertEqual(status.badge(), {"text": "0", "class": "quiet"})


if __name__ == "__main__":
    unittest.main()
