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
  float target_pressure_front, target_pressure_rear;
  char messages[4][64];
  uint32_t message_severity[4];
} F;]]

-- A frame the panel will treat as live, built here rather than read from disk.
--
-- Without this the harness only ever ran the "waiting for AC Pro Engineer"
-- screen: with no application publishing, `readMemoryMappedFile` threw, the
-- panel took the branch it takes when there is nothing to draw, and the check
-- documented as the thing to run after every panel edit exercised none of the
-- drawing. It reported OK for a panel whose every readout was dead.
--
-- `version` and `sequence` are what make the panel call it live; the rest are
-- plausible numbers so a format string that cannot take them fails here.
local function synthesise(b)
  local f = b[0]
  f.version = 3            -- EXPECTED_VERSION; a mismatch draws the error page
  f.sequence = 2           -- even: settled. Zero reads as "never written"
  f.speed_kmh = 214.0
  f.rpm, f.max_rpm, f.gear = 6000, 8000, 4
  f.fuel_litres, f.fuel_per_lap, f.fuel_laps_remaining = 41.2, 3.1, 13.3
  f.delta_seconds = -0.284
  f.best_lap_ms, f.last_lap_ms, f.current_lap_ms = 91380, 92450, 34120
  f.air_temp_c, f.road_temp_c, f.surface_grip = 22, 31, 0.97
  f.lap_count, f.position = 7, 4
  f.target_pressure_front, f.target_pressure_rear = 27.0, 26.5
  for i = 0, 3 do
    f.tyre_pressure_psi[i] = 26.8 + i * 0.2
    f.tyre_temp_c[i] = 78 + i * 7
    f.tyre_wear_percent[i] = 99 - i * 3
    f.brake_temp_c[i] = 320 + i * 90
    f.message_severity[i] = i % 3
  end
  -- Every section on, so no draw path is skipped: connected, telemetry,
  -- engineer, session, timing, fuel.
  f.flags = 2 + 4 + 8 + 32 + 64 + 128
  f.message_count = 2
  ffi.copy(f.messages[0], 'Fuel is fine for the stint')
  ffi.copy(f.messages[1], 'Rear tyres are going off, ease the traction')
end

ac = {
  StructItem = setmetatable({}, { __index = function() return function() return 0 end end }),
  readMemoryMappedFile = function(name, layout)
    local b = ffi.new('F[1]')
    -- A real published frame when one is there — that is what the cargo test
    -- drives, and it proves the offsets as well as the draw paths. A made-up
    -- one otherwise, so the harness is worth running on its own.
    local fh = io.open(os.getenv('ACPE_FRAME') or '/dev/shm/acpe-luacheck', 'rb')
    if fh ~= nil then
      local d = fh:read('*a'); fh:close()
      ffi.copy(b, d, math.min(#d, ffi.sizeof('F')))
    else
      synthesise(b)
    end
    -- Wrap so `messages[i]` yields a Lua string, as CSP's string() type does.
    local raw = b[0]
    return setmetatable({}, { __index = function(_, k)
      if k == 'message_severity' then
        return setmetatable({}, { __index = function(_, i) return raw.message_severity[i] end })
      end
      local slot = k:match('^message_(%d)$')
      if slot ~= nil then
        return ffi.string(raw.messages[tonumber(slot)])
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
  -- The panel draws through DirectWrite so it can pick its own sizes; without
  -- recording it here the harness sees an empty panel and says so.
  dwriteText = function(s) note('dwriteText'); drawn[#drawn+1] = tostring(s) end,
  textWrapped = function(s) note('textWrapped'); drawn[#drawn+1] = tostring(s) end,
  measureDWriteText = function(s, size) return vec2(#tostring(s) * (size or 14) * 0.5, size or 14) end,
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

-- Every check above passes while the panel draws its "waiting for the
-- application" screen in every window, and that is a state with no readouts,
-- no advice and no tyre grid in it. Insist the numbers reached the screen, or
-- this reports OK for a panel that draws nothing.
local screen = table.concat(drawn, '\n')
if not screen:find('214', 1, true) then
  print('\nFAILED: the speed never reached the screen. Every window took its '
    .. '"waiting for AC Pro Engineer" branch, so no drawing was checked.')
  print('rendered ' .. #drawn .. ' pieces of text:')
  for i = 1, math.min(#drawn, 24) do print('  ' .. drawn[i]) end
  os.exit(1)
end
print('live draw path: OK')

-- Sixteen is enough to see the panel came up; ACPE_ALL=1 prints the lot, which
-- is how a wrong unit or an untranslated caption is spotted without the game.
local show = os.getenv('ACPE_ALL') and #drawn or math.min(#drawn, 16)
print('\nrendered ' .. #drawn .. ' pieces of text:')
for i = 1, show do print('  ' .. drawn[i]) end
