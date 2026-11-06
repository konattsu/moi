#!/usr/bin/env python3
"""Session entry point and single-instance command forwarding."""

import signal
import sys

from ui.app import Desktop
from gi.repository import GLib, GLibUnix


if __name__ == "__main__":
    app = Desktop()
    GLibUnix.signal_add(GLib.PRIORITY_DEFAULT, signal.SIGTERM, lambda: (app.quit(), False)[1])
    GLibUnix.signal_add(GLib.PRIORITY_DEFAULT, signal.SIGINT, lambda: (app.quit(), False)[1])
    raise SystemExit(app.run(sys.argv))
