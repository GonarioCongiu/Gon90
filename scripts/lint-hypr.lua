-- Esegue la configurazione Lua di Folio su un'API `hl` finta e stampa ogni
-- opzione toccata, così si può controllare senza avviare Hyprland:
--   lua5.4 scripts/lint-hypr.lua dotfiles/hypr
-- Uscita: righe "config <percorso>", "window_rule <chiave>", "layer_rule <chiave>",
-- "bind <tasti>", "gesture <...>"; codice ≠ 0 in caso di errore Lua.

local root = arg[1] or "dotfiles/hypr"
local out = {}
local function emit(kind, value) out[#out + 1] = kind .. " " .. tostring(value) end

local function walk(prefix, t)
    for k, v in pairs(t) do
        local key = prefix == "" and tostring(k) or (prefix .. "." .. tostring(k))
        -- I colori/gradienti e le coppie {x, y} sono valori, non sotto-categorie.
        if type(v) == "table" and not v.colors and #v == 0 and key ~= "general.col.active_border" then
            walk(key, v)
        else
            emit("config", key)
        end
    end
end

local function dispatcher(name)
    return setmetatable({}, {
        __call = function(_, ...) return { dispatcher = name, args = { ... } } end,
        __index = function(_, k) return dispatcher(name .. "." .. k) end,
    })
end

local function rule(kind)
    return function(t)
        assert(type(t) == "table" and type(t.match) == "table", kind .. " senza match")
        for k in pairs(t) do
            if k ~= "match" and k ~= "name" then emit(kind, k) end
        end
        for k in pairs(t.match) do emit(kind .. ".match", k) end
        return { set_enabled = function() end, is_enabled = function() return true end }
    end
end

local hooks = {}
hl = {
    config = function(t) walk("", t) end,
    monitor = function(t) assert(t.output ~= nil, "monitor senza output") end,
    env = function(k, v) assert(type(k) == "string" and type(v) == "string") end,
    bind = function(keys, action, flags)
        assert(type(keys) == "string", "bind senza tasti")
        assert(action ~= nil, "bind senza azione: " .. keys)
        emit("bind", keys)
    end,
    gesture = function(t) assert(t.fingers and t.direction and t.action, "gesture incompleto") emit("gesture", t.fingers .. ":" .. t.direction) end,
    window_rule = rule("window_rule"),
    layer_rule = rule("layer_rule"),
    workspace_rule = function() end,
    curve = function(name, t) assert(t.type == "bezier" or t.type == "spring") emit("curve", name) end,
    animation = function(t) assert(t.leaf and (t.bezier or t.spring), "animazione incompleta") emit("animation", t.leaf) end,
    on = function(ev, f) hooks[ev] = f end,
    exec_cmd = function(cmd) assert(type(cmd) == "string") end,
    permission = function() end,
    notification = { create = function() end },
    dsp = dispatcher("hl.dsp"),
    plugin = {
        hyprbars = { add_button = function(t) assert(t.action and t.icon, "pulsante hyprbars incompleto") end },
        hyprgrass = {
            bind = function(t)
                assert(t.pattern and t.pattern.kind and t.action, "hyprgrass.bind incompleto")
                emit("hyprgrass.bind", t.pattern.kind .. ":" .. tostring(t.pattern.origin or t.pattern.fingers) .. ":" .. tostring(t.pattern.direction))
            end,
            gesture = function(t)
                assert(t.pattern and t.pattern.kind and t.action, "hyprgrass.gesture incompleto")
                if type(t.action) == "table" then
                    -- Gesto "live": simula start/update/finish.
                    t.action.start({ time_ms = 0, monitor = { width = 1920, height = 1200 }, delta = { x = 0, y = 0 } })
                    t.action.update({ time_ms = 100, delta = { x = 0, y = -300 } })
                    t.action.finish({ time_ms = 400, cancelled = false })
                end
                emit("hyprgrass.gesture", t.pattern.kind .. ":" .. tostring(t.pattern.origin or t.pattern.fingers) .. ":" .. tostring(t.pattern.direction))
            end,
        },
    },
}

-- I moduli vengono cercati in `root`, come farebbe Hyprland in ~/.config/hypr.
package.path = root .. "/?.lua;" .. root .. "/?/init.lua;" .. package.path
local ok, err = pcall(dofile, root .. "/hyprland.lua")
if not ok then
    io.stderr:write("ERRORE: " .. tostring(err) .. "\n")
    os.exit(1)
end
if hooks["hyprland.start"] then hooks["hyprland.start"]() end
table.sort(out)
for _, line in ipairs(out) do print(line) end
