"""Read the notification count without importing or starting a GUI."""

import json
import os
from pathlib import Path


def badge():
    instance = os.environ.get("HYPRLAND_INSTANCE_SIGNATURE", "default")
    path = Path(os.environ["XDG_RUNTIME_DIR"]) / ("moi-desktop-" + instance) / "notifications.json"
    try:
        count = max(0, int(json.loads(path.read_text())["count"]))
    except (OSError, ValueError, KeyError, TypeError):
        count = 0
    return {"text": str(count), "class": "pending" if count else "quiet"}


if __name__ == "__main__":
    print(json.dumps(badge()))
