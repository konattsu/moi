-- Exercise compositor callbacks without a running session or user configuration.
local source = "environments/desktop/modules/desktop-shell/files/config/hyprland.lua"
local hooks, binds, rules, animations, commands = {}, {}, {}, {}, {}
local function dispatcher(path)
    return setmetatable({}, {
        __index = function(_, name) return dispatcher(path .. "." .. name) end,
        __call = function(_, ...) return { path = path, args = {...} } end,
    })
end
hl = {
    dsp = dispatcher("hl.dsp"),
    monitor = function() end, config = function() end, env = function() end,
    layer_rule = function() end,
    animation = function(value) table.insert(animations, value) end,
    window_rule = function(value) table.insert(rules, value) end,
    bind = function(key, action, flags) binds[key] = { action = action, flags = flags } end,
    on = function(event, callback) hooks[event] = callback end,
    exec_cmd = function(command) table.insert(commands, command) end,
}

local real_dofile, real_getenv = dofile, os.getenv
os.getenv = function(key)
    if key == "XDG_CONFIG_HOME" then return "/tmp/moi-lua-check" end
    return real_getenv(key)
end
dofile = function(path)
    if path:match("generated/theme%.lua$") then
        return { accent = "rgb(9ABBF1)", background = "rgb(101216)", border = "rgb(343B47)" }
    elseif path:match("generated/settings%.lua$") then
        return { keyboard_layout = "us", scratch_width = .8, scratch_height = .8,
                 scratch_top = 24, scratch_opacity = .88, reduced_motion = true }
    end
    return real_dofile(path)
end
real_dofile(source)
dofile, os.getenv = real_dofile, real_getenv

assert(binds.Super_L.flags.release and binds.Super_R.flags.release)
assert(binds.Super_L.action.args[1]:match("desktop%.py' bar$"))
assert(binds["SUPER + O"].action.args[1]:match(" overview$"))
assert(not binds["SUPER + Y"], "The old floating shortcut must be removed")
assert(animations[1].enabled == false and animations[2].enabled == false, "Reduced motion must disable both animation branches")
assert(rules[1].match.class == "^kitty$" and rules[1].tile, "Regular terminals must start tiled")

local key = hooks["input.keyboard.key"]
key(133, 0, 1)
key(133, 0, 0)
assert(#commands == 0, "A bare release is handled only by the native modifier binding")
key(133, 0, 1)
key(65, 0, 1) -- Super+Space
key(65, 0, 0)
assert(#commands == 0, "Do not dismiss on chord press")
key(133, 0, 0)
assert(#commands == 1 and commands[1]:match(" dismiss%-bar$"))

commands = {}
key(133, 0, 1)
key(134, 0, 1)
key(32, 0, 1)
key(133, 0, 0)
assert(#commands == 0, "Keep tracking the remaining Super key")
key(134, 0, 0)
assert(#commands == 1)

commands = {}
hooks["window.open"]({ class = "zen", address = "0xdef" })
assert(#commands == 0)
hooks["window.open"]({ class = "kitty", address = "0xabc" })
assert(#commands == 1 and commands[1]:match("normal%-opened '0xabc'$"))
print("PASS: native bar binds, chord release, both Super keys, terminal mapping, reduced motion")
