#!/usr/bin/env bash
set -euo pipefail

zen_dir="$HOME/.local/share/moi/apps/zen"
desktop_file="$HOME/.local/share/applications/zen.desktop"

# Desktop entries do not expand $HOME or ~. Use a quoted absolute path so
# launching from the desktop does not depend on the session's PATH.
zen_exec="$zen_dir/zen"
zen_exec="${zen_exec//\\/\\\\}"
zen_exec="${zen_exec//\"/\\\"}"
zen_exec="${zen_exec//\$/\\\$}"
zen_exec="${zen_exec//\`/\\\`}"
zen_exec="${zen_exec//%/%%}"
# Backslashes in Exec values need escaping at the desktop-entry string layer too.
zen_exec="${zen_exec//\\/\\\\}"
zen_icon="$zen_dir/browser/chrome/icons/default/default128.png"
zen_icon="${zen_icon//\\/\\\\}"

render_entry() {
cat <<EOF
[Desktop Entry]
Type=Application
Name=Zen Browser
Comment=Browse the web
Exec="$zen_exec" %u
Icon=$zen_icon
Terminal=false
StartupNotify=true
StartupWMClass=zen
Categories=Network;WebBrowser;
MimeType=text/html;text/xml;application/xhtml+xml;x-scheme-handler/http;x-scheme-handler/https;
Actions=new-window;new-private-window;profile-manager;

[Desktop Action new-window]
Name=Open a New Window
Exec="$zen_exec" --new-window %u

[Desktop Action new-private-window]
Name=Open a New Private Window
Exec="$zen_exec" --private-window %u

[Desktop Action profile-manager]
Name=Open the Profile Manager
Exec="$zen_exec" --ProfileManager
EOF
}

if [[ -L "$desktop_file" || ( -e "$desktop_file" && ! -f "$desktop_file" ) ]]; then
  echo "error: desktop entry is not a regular file: $desktop_file" >&2
  exit 2
fi

case "${1:-}" in
  check)
    [[ -f "$desktop_file" ]] || exit 1
    if cmp -s <(render_entry) "$desktop_file"; then
      exit 0
    else
      status=$?
      case "$status" in
        1) exit 1 ;;
        *) exit 2 ;;
      esac
    fi
    ;;
  apply)
    mkdir -p "$HOME/.local/share/applications"
    render_entry > "$desktop_file"
    update-desktop-database "$HOME/.local/share/applications"
    ;;
  *)
    echo 'usage: desktop-entry.sh check|apply' >&2
    exit 2
    ;;
esac
