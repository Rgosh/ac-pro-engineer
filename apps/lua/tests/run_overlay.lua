-- Runs the overlay app the way CSP would: stub the API surface, load the
-- script, then drive update() and windowMain() for a few frames. Catches
-- runtime errors that a syntax check cannot -- a nil field, a bad argument,
-- an arithmetic on a string.
-- Resolve the app relative to this script, so the harness works from any
-- checkout.
local here = debug.getinfo(1, 'S').source:sub(2):match('(.*/)')
local appDir = here .. '../ac_pro_engineer/'
package.path = appDir .. '?.lua;' .. package.path
local ffi = require('ffi')

local drawn = {}          -- everything the app tried to render
local calls = {}          -- every stubbed call, to prove the paths ran

local function note(name) calls[name] = (calls[name] or 0) + 1 end

-- Minimal vec2/rgbm.
function vec2(x, y) return { x = x or 0, y = y or 0 } end
function rgbm(r, g, b, a) return { r = r, g = g, b = b, mult = a } end
bit = require('bit')

ffi.cdef[[
typedef struct {
  uint32_t version, sequence;
  float speed_kmh, fuel_litres, fuel_laps_remaining, fuel_per_lap;
  float delta_seconds, air_temp_c, road_temp_c, surface_grip;
  float tyre_pressure_psi[4], tyre_temp_c[4], tyre_wear_percent[4], brake_temp_c[4];
  int32_t rpm, max_rpm, gear, lap_count, last_lap_ms, best_lap_ms, current_lap_ms, position;
  uint32_t flags, message_count;
  char messages[4][64];
  uint32_t message_severity[4];
} F;]]

ac = {
  StructItem = setmetatable({}, { __index = function() return function() return 0 end end }),
  readMemoryMappedFile = function(name, layout)
    local fh = assert(io.open(os.getenv('ACPE_FRAME') or '/dev/shm/acpe-luacheck', 'rb'),
      'demo frame not published; run: cargo run -p ac_core --example publish_demo_frame')
    local d = fh:read('*a'); fh:close()
    local b = ffi.new('F[1]'); ffi.copy(b, d, ffi.sizeof('F'))
    -- Wrap so `messages[i]` yields a Lua string, as CSP's string() type does.
    local raw = b[0]
    return setmetatable({}, { __index = function(_, k)
      if k == 'message_severity' then
        return setmetatable({}, { __index = function(_, i) return raw.message_severity[i] end })
      end
      if k == 'messages' then
        return setmetatable({}, { __index = function(_, i) return ffi.string(raw.messages[i]) end })
      end
      return raw[k]
    end })
  end,
}

-- Anything the app reaches for that is not spelled out below: callable, so
-- `ui.pushStyleVar(...)` works, and indexable, so `ui.StyleVar.WindowRounding`
-- does too. A stub that is only one of the two turns a missing entry into a
-- confusing error about indexing a function.
local function stub(name)
  return setmetatable({}, {
    __call = function(_, ...) note(name); return 0 end,
    __index = function() return 0 end,
  })
end

ui = setmetatable({
  Font = { Small=1, Tiny=2, Monospace=3, Main=4, Italic=5, Title=6, Huge=7 },
  text = function(s) note('text'); drawn[#drawn+1] = tostring(s) end,
  textColored = function(s) note('textColored'); drawn[#drawn+1] = tostring(s) end,
  availableSpace = function() return vec2(300, 380) end,
  availableSpaceX = function() return 300 end,
  getCursor = function() return vec2(0, 0) end,
}, { __index = function(_, k) return stub(k) end })

script = {}
local ok, err = pcall(dofile, appDir .. 'ac_pro_engineer.lua')
if not ok then print('LOAD FAILED: ' .. tostring(err)); os.exit(1) end
print('load: OK')

for i = 1, 3 do
  local u, e = pcall(script.update, 0.016)
  if not u then print('update FAILED: ' .. tostring(e)); os.exit(1) end
end
print('update: OK')

local w, e2 = pcall(script.windowMain, 0.016)
if not w then print('windowMain FAILED: ' .. tostring(e2)); os.exit(1) end
print('windowMain: OK')

-- Every window the script exposes, driven the same way CSP drives them: a
-- new one that throws on its first frame should fail here, not in the pits.
for _, name in ipairs({ 'windowEngineer', 'windowSettings', 'windowTelemetry', 'windowStatus' }) do
  if script[name] ~= nil then
    local drew, drawError = pcall(script[name], 0.016)
    if not drew then print(name .. ' FAILED: ' .. tostring(drawError)); os.exit(1) end
    print(name .. ': OK')
  end
end

print('\nrendered ' .. #drawn .. ' pieces of text:')
for i = 1, math.min(#drawn, 16) do print('  ' .. drawn[i]) end
