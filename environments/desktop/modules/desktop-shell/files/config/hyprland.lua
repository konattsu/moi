-- Hyprland Lua configuration. Personal overrides belong in hypr/local.lua.
local config = os.getenv("XDG_CONFIG_HOME") or (os.getenv("HOME") .. "/.config")
local root = config .. "/moi-desktop/"
local colors = dofile(root .. "generated/theme.lua")
local settings = dofile(root .. "generated/settings.lua")
local function quote(s) return "'" .. s:gsub("'", "'\\''") .. "'" end
local controller = "python3 " .. quote(root .. "desktop.py") .. " "
local function command(action) return hl.dsp.exec_cmd(controller .. action) end
local function bind(key, action)
    hl.bind(key, action)
end

hl.monitor({ output = "", mode = "preferred", position = "auto", scale = "auto" })
hl.config({
    general = {
        layout = "dwindle", gaps_in = 5, gaps_out = 10, border_size = 1,
        col = { active_border = colors.accent, inactive_border = colors.border },
    },
    decoration = {
        rounding = 10, active_opacity = 1, inactive_opacity = 1,
        blur = { enabled = false }, shadow = { enabled = false },
    },
    animations = { enabled = not settings.reduced_motion },
    dwindle = { preserve_split = true },
    input = {
        kb_layout = settings.keyboard_layout,
        -- A bindable non-locking keysym: no Caps state or root-level keyd needed.
        kb_options = "caps:menu",
        follow_mouse = 0,
        special_fallthrough = true,
    },
    misc = { disable_hyprland_logo = true, force_default_wallpaper = 0 },
})
hl.animation({ leaf = "global", enabled = not settings.reduced_motion, speed = 1.6, bezier = "default" })
hl.animation({ leaf = "specialWorkspace", enabled = not settings.reduced_motion, speed = 2, bezier = "default", style = "slidevert" })
hl.layer_rule({ match = { namespace = "^moi-launcher$" }, no_anim = true })
hl.window_rule({ match = { class = "^kitty$" }, tile = true })
hl.window_rule({ match = { float = false }, rounding = 0 })
hl.window_rule({
    match = { class = "^moi-scratchpad(-.*)?$" },
    workspace = "special:moi-scratch silent", float = true,
    size = { "monitor_w*" .. settings.scratch_width, "monitor_h*" .. settings.scratch_height },
    move = { "(monitor_w-window_w)/2", settings.scratch_top },
    animation = "slide top",
    fullscreen_state = "0 0", suppress_event = "maximize fullscreen",
})
hl.window_rule({ match = { tag = "moi-scratch" }, opacity = string.format("%.2f override %.2f override", settings.scratch_opacity, settings.scratch_opacity) })
-- Browser gets a predictable home; other workspaces remain free.
hl.window_rule({ match = { class = "^(zen|zen-browser)$" }, workspace = "2" })

-- Native modifier-only releases never open the bar after a shortcut chord.
hl.bind("Super_L", command("bar"), { release = true })
hl.bind("Super_R", command("bar"), { release = true })
-- The bar never captures keyboard focus. Close it after a Super chord too.
local super_down, super_used = {}, false
hl.on("input.keyboard.key", function(key, _, state)
    if key == 133 or key == 134 then
        if state == 1 then
            if not next(super_down) then super_used = false end
            super_down[key] = true
        elseif state == 0 then
            super_down[key] = nil
            if super_used and not next(super_down) then hl.exec_cmd(controller .. "dismiss-bar") end
        end
    elseif state == 1 and next(super_down) then
        super_used = true
    end
end)
hl.bind("SUPER + Space", command("search"))
hl.bind("SUPER + O", command("overview"))
hl.bind("SUPER + P", command("search '$'"))
hl.bind("SUPER + U", command("toggle"))
hl.bind("SUPER + SHIFT + U", command("tile"))
hl.bind("SUPER + CTRL + U", command("adopt"))
hl.bind("SUPER + I", command("lock"))
bind("SUPER + Return", hl.dsp.exec_cmd("uwsm app -t service -- kitty"))
bind("SUPER + N", hl.dsp.window.close())
bind("SUPER + M", hl.dsp.window.fullscreen())
for key, direction in pairs({ H = "left", J = "down", K = "up", L = "right" }) do
    bind("SUPER + " .. key, hl.dsp.focus({ direction = direction }))
    bind("SUPER + SHIFT + " .. key, hl.dsp.window.move({ direction = direction }))
    local x = (direction == "left" and -40) or (direction == "right" and 40) or 0
    local y = (direction == "up" and -40) or (direction == "down" and 40) or 0
    bind("SUPER + CTRL + " .. key, hl.dsp.window.resize({ x = x, y = y, relative = true }))
end
for i = 1, 10 do
    bind("SUPER + " .. (i % 10), hl.dsp.focus({ workspace = i }))
    bind("SUPER + SHIFT + " .. (i % 10), hl.dsp.window.move({ workspace = i }))
end
hl.bind("Menu", hl.dsp.exec_cmd("fcitx5-remote -t"))
for key, action in pairs({
    XF86AudioRaiseVolume = "volume-up", XF86AudioLowerVolume = "volume-down",
    XF86AudioMute = "mute", XF86MonBrightnessUp = "brightness-up",
    XF86MonBrightnessDown = "brightness-down",
}) do
    hl.bind(key, command(action), { repeating = true })
end

hl.env("XMODIFIERS", "@im=fcitx")
hl.env("QT_IM_MODULE", "fcitx")
hl.env("GTK_IM_MODULE", "fcitx")
hl.env("LC_MESSAGES", "C.UTF-8")
hl.on("window.open", function(window)
    if window.class == "kitty" then
        hl.exec_cmd(controller .. "normal-opened " .. quote(window.address))
    end
end)
hl.on("hyprland.start", function()
    hl.exec_cmd("systemctl --user start moi-desktop.service")
    hl.exec_cmd("systemctl --user start hyprpolkitagent.service")
end)
local override = config .. "/hypr/local.lua"
local file = io.open(override, "r")
if file then file:close(); dofile(override) end
