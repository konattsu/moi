# Replace the legacy desktop vi-mode block with ordinary shell editing.
if [[ -o interactive && -n ${HYPRLAND_INSTANCE_SIGNATURE:-} ]]; then
  bindkey -e
fi
