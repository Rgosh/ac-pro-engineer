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
local sliderMoved = nil   -- {id, value}: pretend the driver dragged that one
local buttonPressed = nil -- label: pretend the driver clicked that one
local checkboxClicked = nil -- id: pretend the driver ticked that one
local carPresent = true   -- false publishes a frame with CONNECTED cleared

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
  char messages[8][64];
  uint32_t message_severity[8];
  char app_version[16];
  uint32_t debrief_lap_count;
  uint32_t debrief_lap_number[3], debrief_lap_time_ms[3], debrief_line_count[3];
  uint32_t debrief_severity[12];
  char debrief[12][64];
} F;]]

--- PANEL_VERSION as the app under test declares it.
---
--- Read from the source rather than written here twice: a copy in the harness
--- is a copy that goes stale, and a stale one makes every run draw the update
--- notice, which is the state the notice exists to make unusual.
local function currentPanelVersion()
  local fh = assert(io.open(appDir .. 'ac_pro_engineer.lua', 'r'))
  local text = fh:read('*a'); fh:close()
  return text:match("local PANEL_VERSION%s*=%s*'([^']+)'") or '0.0.0'
end

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
  f.version = 6            -- EXPECTED_VERSION; a mismatch draws the error page
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
  end
  for i = 0, 7 do f.message_severity[i] = i % 3 end
  -- Every section on, so no draw path is skipped: connected, telemetry,
  -- engineer, session, timing, fuel.
  f.flags = 2 + 4 + 8 + 32 + 64 + 128
  -- More lines than the panel draws by default, so the "there are more than
  -- you asked for" path runs. With two the cap was never reached and a slot
  -- past the fourth could have been unreadable without anything noticing.
  f.message_count = 8
  local advice = {
    'Fuel is fine for the stint',
    'Rear tyres are going off, ease the traction',
    'Box this lap',
    'Front-left pressure is low and the corner is cold',
    'Front brakes are past 700 C, open the ducts',
    'You are coasting into turn 4, brake later',
    'Rear camber is too negative for this compound',
    'Traction control is cutting on corner exit',
  }
  for i = 1, 8 do ffi.copy(f.messages[i - 1], advice[i]) end
  -- ACPE_APP_VERSION lets a run pretend the application is on another release,
  -- which is the only way to reach the "restart the game" notice from here.
  ffi.copy(f.app_version, os.getenv('ACPE_APP_VERSION') or currentPanelVersion())

  -- Three finished laps, each with its own advice, so the debrief window has
  -- something to switch between. Full slots on the newest and fewer further
  -- back: a lap that had less to say must not read the next lap's lines.
  f.debrief_lap_count = 3
  local debriefs = {
    { lap = 12, ms = 91234, lines = {
      { 'Fronts over 28.4 psi (target 27.5)', 1 },
      { 'Front: inner edge running hot (I-O: 15.0C)', 1 },
      { 'Lockups: 4', 0 },
      { 'Coasting 18%', 0 },
    } },
    { lap = 11, ms = 92871, lines = {
      { 'All four cold 62C', 0 },
      { 'Rear: outer edge hotter (I-O: -6.0C)', 0 },
    } },
    { lap = 10, ms = 95002, lines = {
      { 'FL/RL overheating 815C', 2 },
    } },
  }
  for lapIndex = 0, 2 do
    local entry = debriefs[lapIndex + 1]
    f.debrief_lap_number[lapIndex] = entry.lap
    f.debrief_lap_time_ms[lapIndex] = entry.ms
    f.debrief_line_count[lapIndex] = #entry.lines
    for line = 1, #entry.lines do
      local slot = lapIndex * 4 + line - 1
      ffi.copy(f.debrief[slot], entry.lines[line][1])
      f.debrief_severity[slot] = entry.lines[line][2]
    end
  end
end

-- Somewhere for the panel's own settings file, which is the copy that has to
-- survive a CSP whose `ac.storage` does not. Its own directory, emptied first,
-- so a run cannot pass on what the last one left and cannot write into the
-- checkout.
local SETTINGS_DIR = os.getenv('ACPE_SETTINGS_DIR') or '/tmp/acpe-overlay-test'
os.execute('rm -rf ' .. SETTINGS_DIR .. ' && mkdir -p ' .. SETTINGS_DIR)

ac = {
  StructItem = setmetatable({}, { __index = function() return function() return 0 end end }),
  -- CSP's folder API, which is where the panel looks first for somewhere to
  -- keep its file.
  FolderID = { ExtCfgUser = 1 },
  getFolder = function() return SETTINGS_DIR end,
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
    --
    -- `b` is captured on purpose. `b[0]` is a *reference* into the array, and a
    -- reference does not keep its owner alive: once this function returned, `b`
    -- was unreachable, and the next collection freed the memory `raw` points
    -- at — after which every field read as zero. It survived for as long as the
    -- panel was one file, because nothing allocated enough between opening the
    -- mapping and reading it to trigger a collection. Splitting the panel into
    -- a dozen modules did, and the whole harness quietly went back to drawing
    -- the "waiting for AC Pro Engineer" screen.
    local held = b
    local raw = held[0]
    return setmetatable({}, { __index = function(_, k)
      local _ = held
      if k == 'message_severity' then
        return setmetatable({}, { __index = function(_, i) return raw.message_severity[i] end })
      end
      if k == 'app_version' then return ffi.string(raw.app_version) end
      -- The debrief travels as named string fields for the same reason the
      -- advice does: an array of strings comes back as raw cdata.
      local lap, line = k:match('^debrief_(%d)_(%d)$')
      if lap ~= nil then
        return ffi.string(raw.debrief[tonumber(lap) * 4 + tonumber(line)])
      end
      if k == 'debrief_lap_number' or k == 'debrief_lap_time_ms'
        or k == 'debrief_line_count' or k == 'debrief_severity' then
        return setmetatable({}, { __index = function(_, i) return raw[k][i] end })
      end
      local slot = k:match('^message_(%d)$')
      if slot ~= nil then
        return ffi.string(raw.messages[tonumber(slot)])
      end
      if k == 'messages' then
        return setmetatable({}, { __index = function(_, i) return ffi.string(raw.messages[i]) end })
      end
      -- `carPresent` clears the CONNECTED bit, which is the state the panel is
      -- in whenever the application is publishing and AC has no telemetry:
      -- the launcher, the menus, and the pit garage before a session starts.
      if k == 'flags' and not carPresent then
        return bit.band(raw.flags, bit.bnot(2))
      end
      return raw[k]
    end })
  end,
}

-- ---------------------------------------------------------------------------
-- Persistent settings
--
-- `ac.storage` was not stubbed at all, so `type(ac.storage) == 'function'` was
-- false, the panel fell back to defaults that only last for the run, and the
-- entire save path -- the thing drivers reported as "the panel forgets
-- everything" -- was never executed here once.
--
-- Same shape as the LOVE harness's (apps/lua/love/csp.lua): hand it a table of
-- defaults, get back a proxy whose writes land in `savedValues`. `savedValues`
-- outlives a reload of the script, which is the whole point -- that is what
-- CSP's storage does when a window is closed and opened again.
-- ---------------------------------------------------------------------------
local savedValues = {}
local storageAsked = 0

ac.storage = function(defaults, _prefix)
  storageAsked = storageAsked + 1
  for key, value in pairs(defaults) do
    if savedValues[key] == nil then savedValues[key] = value end
  end
  return setmetatable({}, {
    __index = savedValues,
    __newindex = function(_, key, value) savedValues[key] = value end,
    __pairs = function() return pairs(savedValues) end,
  })
end

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

  -- Tabs run their bodies. The catch-all stub below returns 0 and calls
  -- nothing, so every `ui.tabItem(name, function() ... end)` in the settings
  -- window was skipped -- which is the whole settings window, all fifteen
  -- tabs of it. `windowSettings: OK` meant the tab bar was constructed, not
  -- that a single control inside it had been drawn, and that is how a slider
  -- with a nil bound or a caption calling a function declared below it got
  -- through: this harness is documented as the thing to run after every panel
  -- edit, and it was reporting on an empty window.
  -- Widgets that report what the user did report that the user did nothing.
  --
  -- The catch-all stub returns 0, and 0 is truthy in Lua, so the moment the
  -- tab bodies below started running, every `if ui.checkbox(...)` in the
  -- settings window fired at once: the panel came out of one frame with every
  -- toggle inverted, including `freezeDisplay`, which stops it reading the
  -- frame at all. A harness that clicks every control it draws is not driving
  -- the panel, it is fighting it.
  -- One named box at a time, the same way `sliderMoved` drags one slider.
  -- A checkbox is what a driver actually changes in this window, and until
  -- this existed nothing here had ever clicked one.
  checkbox = function(id)
    note('checkbox')
    return checkboxClicked ~= nil and id == checkboxClicked
  end,
  button = function(label)
    note('button')
    return buttonPressed ~= nil and label == buttonPressed
  end,
  radioButton = function() note('radioButton'); return false end,
  colorButton = function() note('colorButton'); return false end,
  colorPicker = function() note('colorPicker'); return false end,
  -- `sliderMoved` lets one check below drag one slider, which is the only way
  -- to reach the panel's save path the way a driver reaches it.
  slider = function(id, value)
    note('slider')
    if sliderMoved ~= nil and id == sliderMoved.id then
      return sliderMoved.value, true
    end
    return value, false
  end,
  inputText = function(_id, text) note('inputText'); return text, false, false end,

  tabBar = function(_id, body)
    note('tabBar')
    if type(body) == 'function' then body() end
  end,
  tabItem = function(name, a, b)
    note('tabItem')
    drawn[#drawn + 1] = tostring(name)
    local body = type(b) == 'function' and b or (type(a) == 'function' and a or nil)
    if body ~= nil then body() end
  end,
}, { __index = function(_, k) return stub(k) end })

--- Load the panel the way CSP loads it: from nothing.
---
--- `package.loaded` has to be cleared, or `require` hands the reloaded entry
--- point the module instances the previous load left behind — with their frame
--- already read, their settings already applied and their liveness already
--- decided. CSP throws the whole Lua state away between loads; `dofile` on its
--- own does not, and the two checks below that reload the panel were quietly
--- testing nothing.
local function loadPanel()
  for name in pairs(package.loaded) do
    if name == 'frame_layout' or name:match('^acpe') then
      package.loaded[name] = nil
    end
  end
  script = {}
  return pcall(dofile, appDir .. 'ac_pro_engineer.lua')
end

local ok, err = loadPanel()
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

-- A local declared after its callers is a global to them -- that is, nil -- and
-- the file still loads, so nothing above catches it. It has now happened four
-- times: the developer tab, the harness's control panel, the console, and
-- `applyDemo`/`DEMO_ADVICE`, where turning on either developer switch called or
-- indexed a nil. Every one of them was in a path that is off by default, which
-- is exactly why driving the windows does not find them.
--
-- So: compile the panel and look at which names it reads from the global table.
-- Anything outside this list is either a typo or a local declared too late.
local ALLOWED_GLOBALS = {
  -- CSP's API, and the two constructors it exposes as bare globals.
  ac = true, ui = true, script = true, vec2 = true, rgbm = true,
  -- LuaJIT's bit library, and the standard library the panel uses.
  bit = true, ipairs = true, pairs = true, math = true, string = true,
  table = true, pcall = true, require = true, tonumber = true,
  tostring = true, type = true,
  -- `acpe/persist.lua` keeps the settings in a file. It reads this once, into
  -- a local, and checks it is there before using it — CSP's sandbox is not
  -- guaranteed to hand a script the file library.
  io = true,
}

-- Every file, not just the entry point. The panel is a dozen modules now, and
-- checking only `ac_pro_engineer.lua` let `isLive` come through the split as a
-- global — nil to everything that read it, so every window drew the "waiting
-- for AC Pro Engineer" screen and the load, update and window checks above all
-- still said OK.
local sources = {}
local listing = io.popen('find ' .. appDir .. " -name '*.lua' | sort")
if listing ~= nil then
  for path in listing:lines() do sources[#sources + 1] = path end
  listing:close()
end

local strayByFile = {}
local strayCount = 0
for _, path in ipairs(sources) do
  local pipe = io.popen('luajit -bl ' .. path .. ' 2>/dev/null')
  if pipe ~= nil then
    local bytecode = pipe:read('*a')
    pipe:close()

    local stray = {}
    for name in bytecode:gmatch('GGET%s+%d+%s+%d+%s*;%s*"([^"]+)"') do
      if not ALLOWED_GLOBALS[name] then stray[name] = true end
    end

    local names = {}
    for name in pairs(stray) do names[#names + 1] = name end
    table.sort(names)
    if #names > 0 then
      strayByFile[#strayByFile + 1] = { path, names }
      strayCount = strayCount + #names
    end
  end
end

if strayCount > 0 then
  print('\nFAILED: the panel reads ' .. strayCount .. ' name(s) from the global table:')
  for _, entry in ipairs(strayByFile) do
    print('  ' .. entry[1]:gsub('.*/ac_pro_engineer/', ''))
    for _, name in ipairs(entry[2]) do print('    ' .. name) end
  end
  print('Each is either a typo or a local declared below something that calls')
  print('it. A local declared after its callers is nil to them, and the file')
  print('still loads, so only this check finds it.')
  os.exit(1)
end
print('globals: OK (' .. #sources .. ' files)')

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

-- ---------------------------------------------------------------------------
-- A setting changed in the window has to be there after the script reloads
--
-- CSP reloads the script when a window is reopened, so "does the panel save"
-- is not a question about a call succeeding — it is a question about what a
-- freshly loaded copy reads. Nothing here checked it, in either direction:
-- storage was not even stubbed.
-- ---------------------------------------------------------------------------

if storageAsked == 0 then
  print('\nFAILED: the panel never asked for ac.storage, so nothing it is told')
  print('can outlive the window being closed.')
  os.exit(1)
end

-- Drag the advice-lines slider to two, the way a driver would. Two rather than
-- a number near the default, so counting the lines afterwards cannot pass by
-- accident.
sliderMoved = { id = '##adviceLines', value = 2 }
local dragged, dragError = pcall(script.windowSettings, 0.016)
sliderMoved = nil
if not dragged then
  print('\nFAILED: moving a slider threw: ' .. tostring(dragError))
  os.exit(1)
end

if savedValues.engineerLines ~= 2 then
  print('\nFAILED: the panel drew a slider, took the new value and did not save it.')
  print('storage holds engineerLines = ' .. tostring(savedValues.engineerLines)
    .. ', expected 2')
  os.exit(1)
end
print('settings reach storage: OK')

-- Load it again, as CSP does when the window is reopened, and ask the fresh
-- copy what it thinks the setting is. Its own developer tab answers that:
-- "Dump settings to console" prints the live `settings` table into the console,
-- which the Console tab draws. Reading the panel's answer rather than counting
-- advice lines keeps this independent of whatever frame the run was given —
-- ACPE_FRAME points at a real published one under `cargo test`.
-- A mark in `drawn`, not a fresh table: emptying it threw away the drive
-- output, and the sample printed at the end -- which is what the cargo test
-- greps for the published speed -- came out as nothing but settings captions.
local reloadedFrom = #drawn

-- Developer mode on, so the reloaded copy has a Dev tab to dump from. Through
-- the panel's own store rather than by poking `savedValues`: the panel keeps a
-- file as well as storage now, and a value written into storage behind its
-- back is one the two sources disagree about — which is exactly the case the
-- file is designed to win.
local store = package.loaded['acpe.settings']
store.values.devMode = true
store.save()

local reloaded, reloadError = loadPanel()
if not reloaded then
  print('\nFAILED: the panel would not load a second time: ' .. tostring(reloadError))
  os.exit(1)
end
pcall(script.update, 0.016)

-- Twice: the Console tab is drawn before the Dev tab, so the dump lands after
-- the console has already been laid out and shows up on the next frame.
buttonPressed = 'Dump settings to console'
local dumped, dumpError = pcall(script.windowSettings, 0.016)
buttonPressed = nil
if dumped then dumped, dumpError = pcall(script.windowSettings, 0.016) end
if not dumped then
  print('\nFAILED: windowSettings after a reload: ' .. tostring(dumpError))
  os.exit(1)
end

local restored = nil
for i = reloadedFrom + 1, #drawn do
  restored = drawn[i]:match('engineerLines=(%S+)') or restored
end

if restored ~= '2' then
  print('\nFAILED: the reloaded panel reads engineerLines = ' .. tostring(restored)
    .. ', expected the 2 it was told to keep.')
  print('The setting was written and not read back, which is what a driver')
  print('sees as the panel forgetting everything between sessions.')
  os.exit(1)
end
print('settings survive a reload: OK')

-- ---------------------------------------------------------------------------
-- A checkbox survives a CSP whose storage does not
--
-- Everything above proves the panel talks to `ac.storage` and reads back what
-- it wrote. None of it proves anything about a build where storage accepts a
-- write and forgets it — which is a black box from the panel's side, and is
-- what a driver reports as "it does not save the checkboxes".
--
-- So: tick a box, throw storage away, reload, and ask the panel what it draws.
-- The only thing that can carry the answer across is the panel's own file.
-- ---------------------------------------------------------------------------

-- Tick "Tyres and brakes" off. A checkbox, because that is what the report is
-- about, and this one has a visible consequence in another window.
checkboxClicked = '##showTyres'
local ticked, tickError = pcall(script.windowSettings, 0.016)
checkboxClicked = nil
if not ticked then
  print('\nFAILED: clicking a checkbox threw: ' .. tostring(tickError))
  os.exit(1)
end

if package.loaded['acpe.settings'].values.showTyres ~= false then
  print('\nFAILED: the box was clicked and the setting did not change.')
  os.exit(1)
end

-- The file has to exist and hold it, or there is nothing to survive on.
local settingsFile = package.loaded['acpe.persist'].path()
if settingsFile == nil then
  print('\nFAILED: the panel never wrote a settings file, so nothing it is told')
  print('outlives a build whose ac.storage does not persist.')
  os.exit(1)
end

local onDisk = io.open(settingsFile, 'r')
local body = onDisk and onDisk:read('*a') or ''
if onDisk then onDisk:close() end
if not body:match('%["showTyres"%]%s*=%s*false') then
  print('\nFAILED: ' .. settingsFile .. ' does not hold showTyres = false.')
  print(body)
  os.exit(1)
end

-- Now a CSP whose storage is write-only: it takes assignments, hands back the
-- defaults, and remembers nothing. Before the file existed this lost every
-- setting the driver had ever changed.
ac.storage = function(defaults)
  storageAsked = storageAsked + 1
  local forgetful = {}
  for key, value in pairs(defaults) do forgetful[key] = value end
  return setmetatable({}, {
    __index = forgetful,
    __newindex = function() end,
    __pairs = function() return pairs(forgetful) end,
  })
end

local survivedFrom = #drawn
local reloadedAgain, reloadAgainError = loadPanel()
if not reloadedAgain then
  print('\nFAILED: the panel would not load against forgetful storage: '
    .. tostring(reloadAgainError))
  os.exit(1)
end
pcall(script.update, 0.016)

if package.loaded['acpe.settings'].values.showTyres ~= false then
  print('\nFAILED: with ac.storage forgetting everything, the reloaded panel')
  print('reads showTyres = '
    .. tostring(package.loaded['acpe.settings'].values.showTyres)
    .. ', expected the false it was told to keep.')
  print('This is the report: the checkboxes do not stay where they are put.')
  os.exit(1)
end

-- And it has to reach the screen, not just the table.
local drewMain, mainError = pcall(script.windowMain, 0.016)
if not drewMain then
  print('\nFAILED: windowMain after the forgetful reload: ' .. tostring(mainError))
  os.exit(1)
end
for i = survivedFrom + 1, #drawn do
  if drawn[i]:match('TYRES') then
    print('\nFAILED: the tyres block was switched off and the panel drew it anyway.')
    os.exit(1)
  end
end
print('a checkbox outlives storage that forgets: OK')

-- ---------------------------------------------------------------------------
-- The Changed tab lists what was changed, and undoes it
--
-- Eighty-five settings across six tabs, and the one making the panel look
-- wrong is the one you do not remember touching. This is the list of
-- everything that differs from the defaults, so it has to actually name them
-- and its reset has to actually work.
-- ---------------------------------------------------------------------------

local liveSettings = package.loaded['acpe.settings'].values
liveSettings.engineerLines = 7

local changedFrom = #drawn
local listed, listError = pcall(script.windowSettings, 0.016)
if not listed then
  print('\nFAILED: the Changed tab threw: ' .. tostring(listError))
  os.exit(1)
end

local listing = table.concat(drawn, '\n', changedFrom + 1, #drawn)
if not listing:find('engineerLines', 1, true) then
  print('\nFAILED: a changed setting is not in the list of changed settings.')
  os.exit(1)
end

-- The way back, through the button beside it.
buttonPressed = 'reset##acpeResetengineerLines'
local undone, undoError = pcall(script.windowSettings, 0.016)
buttonPressed = nil
if not undone then
  print('\nFAILED: resetting one setting threw: ' .. tostring(undoError))
  os.exit(1)
end

local default = package.loaded['acpe.settings'].DEFAULTS.engineerLines
if liveSettings.engineerLines ~= default then
  print('\nFAILED: reset left engineerLines at ' .. tostring(liveSettings.engineerLines)
    .. ', expected the default ' .. tostring(default))
  os.exit(1)
end
print('the Changed tab lists and undoes: OK')

-- ---------------------------------------------------------------------------
-- New advice is told apart from advice that has been there four laps
--
-- Tracked by the sentence, not by the slot: the application packs what it has
-- to say into the slots in order, so a line that was second last lap is first
-- this lap without having changed. Slot tracking would call everything new
-- every time a line above it cleared, which is no signal at all.
-- ---------------------------------------------------------------------------

local liveFrame = package.loaded['acpe.frame']
local liveShown = liveFrame.shown

liveShown.message_count = 2
liveShown.messages[1] = 'Rear tyres are going off'
liveShown.messages[2] = 'Box this lap'
liveShown.message_severity[1] = 1
liveShown.message_severity[2] = 2
liveFrame.markArrivalsForTest()

if not liveFrame.messageIsNew(1) or not liveFrame.messageIsNew(2) then
  print('\nFAILED: advice that just arrived does not count as new.')
  os.exit(1)
end

-- The same two sentences, in the other order, six seconds later. Neither is
-- new any more, and swapping places must not make them look new again.
liveShown.messages[1] = 'Box this lap'
liveShown.messages[2] = 'Rear tyres are going off'
liveFrame.advanceClockForTest(7.0)
liveFrame.markArrivalsForTest()

if liveFrame.messageIsNew(1) or liveFrame.messageIsNew(2) then
  print('\nFAILED: settled advice is still counted as new after it moved slot.')
  os.exit(1)
end

-- One of them clears and a different one arrives. Only the arrival is new.
liveShown.messages[1] = 'Front brakes are past 700 C'
liveShown.messages[2] = 'Rear tyres are going off'
liveFrame.markArrivalsForTest()

if not liveFrame.messageIsNew(1) then
  print('\nFAILED: a sentence the panel has never shown is not counted as new.')
  os.exit(1)
end
if liveFrame.messageIsNew(2) then
  print('\nFAILED: a sentence that has been there all along counts as new.')
  os.exit(1)
end
print('new advice is told apart from settled advice: OK')

-- ---------------------------------------------------------------------------
-- Every button in the settings window does what its label says
--
-- The window is built from buttons whose effect is a line of Lua somewhere
-- else, and a button that silently does nothing looks exactly like a button
-- that worked. Each of these is pressed the way a driver presses it — one
-- frame, one button — and checked by what it changed.
-- ---------------------------------------------------------------------------

local liveStore = package.loaded['acpe.settings']
local vals = liveStore.values

--- Press one button, by the exact label it draws, and hand back what threw.
local function press(label)
  buttonPressed = label
  local ok, err = pcall(script.windowSettings, 0.016)
  buttonPressed = nil
  return ok, err
end

local function require_(condition, message)
  if not condition then
    print('\nFAILED: ' .. message)
    os.exit(1)
  end
end

-- The console's quick buttons. Each is a command line, and each has to leave
-- the setting it names in the state it names.
for key, value in pairs(liveStore.DEFAULTS) do vals[key] = value end

local ok, err = press('4K')
require_(ok, '"4K" threw: ' .. tostring(err))
require_(vals.fontScale == 2 and vals.contentWidth == 680 and vals.barHeight == 12,
  '"4K" left scale=' .. tostring(vals.fontScale) .. ' width=' .. tostring(vals.contentWidth)
  .. ' bar=' .. tostring(vals.barHeight) .. ', expected 2 / 680 / 12')

ok, err = press('1080p')
require_(ok, '"1080p" threw: ' .. tostring(err))
require_(vals.fontScale == 1 and vals.contentWidth == 360 and vals.barHeight == 6,
  '"1080p" did not put the panel back to 1 / 360 / 6')

ok, err = press('VR')
require_(ok, '"VR" threw: ' .. tostring(err))
require_(vals.vrMode == true and vals.fontScale == 1.6,
  '"VR" left vrMode=' .. tostring(vals.vrMode) .. ' scale=' .. tostring(vals.fontScale))

ok, err = press('Smaller')
require_(ok, '"Smaller" threw: ' .. tostring(err))
require_(vals.fontScale == 0.85, '"Smaller" left scale=' .. tostring(vals.fontScale))

ok, err = press('Bigger')
require_(ok, '"Bigger" threw: ' .. tostring(err))
require_(vals.fontScale == 1.2, '"Bigger" left scale=' .. tostring(vals.fontScale))

-- Developer mode is a toggle, so it has to go both ways.
local before = vals.devMode
ok, err = press('Dev')
require_(ok, '"Dev" threw: ' .. tostring(err))
require_(vals.devMode ~= before, '"Dev" did not toggle developer mode')
ok = press('Dev')
require_(vals.devMode == before, '"Dev" pressed twice did not come back')

-- Reset puts everything back, including what the presses above changed.
vals.fontScale = 2.5
ok, err = press('Reset')
require_(ok, '"Reset" threw: ' .. tostring(err))
require_(vals.fontScale == liveStore.DEFAULTS.fontScale,
  '"Reset" left scale=' .. tostring(vals.fontScale))

-- The Units tab's two, which are the ones a driver presses when they do not
-- believe the settings saved.
ok, err = press('Save now')
require_(ok, '"Save now" threw: ' .. tostring(err))
require_(liveStore.report.ever, '"Save now" did not record that it saved')

vals.engineerLines = 8
ok, err = press('Reset to defaults')
require_(ok, '"Reset to defaults" threw: ' .. tostring(err))
require_(vals.engineerLines == liveStore.DEFAULTS.engineerLines,
  '"Reset to defaults" left engineerLines=' .. tostring(vals.engineerLines))

-- The Look tab's palette button.
vals.colorText = '0.10,0.10,0.10'
ok, err = press('Default palette')
require_(ok, '"Default palette" threw: ' .. tostring(err))
require_(vals.colorText == liveStore.DEFAULTS.colorText,
  '"Default palette" left colorText=' .. tostring(vals.colorText))

-- The Dev tab's two, which are only reachable with developer mode on.
vals.devMode = true
vals.showTyres = false
ok, err = press('Everything on')
require_(ok, '"Everything on" threw: ' .. tostring(err))
require_(vals.showTyres == true and vals.devIgnoreFlags == true,
  '"Everything on" left showTyres=' .. tostring(vals.showTyres)
  .. ' devIgnoreFlags=' .. tostring(vals.devIgnoreFlags))

ok, err = press('Leave developer mode')
require_(ok, '"Leave developer mode" threw: ' .. tostring(err))
require_(vals.devMode == false and vals.devIgnoreFlags == false,
  '"Leave developer mode" left devMode=' .. tostring(vals.devMode))

-- The console's own four. `Again` and `Clear` are the two that can quietly do
-- nothing, because both are about state the button does not itself set.
local liveConsole = package.loaded['acpe.console']

ok, err = press('Help')
require_(ok, '"Help" threw: ' .. tostring(err))

local helpFrom = #drawn
ok = press('Clear')
local afterClear = #drawn
ok, err = press('Help')
require_(ok, '"Help" after "Clear" threw: ' .. tostring(err))
local helpLines = table.concat(drawn, '\n', afterClear + 1, #drawn)
require_(helpLines:find('--scale', 1, true) ~= nil,
  '"Help" did not print the command list')

-- `Again` repeats the last command, and a quick button is a command. This is
-- the path that was broken: the presets went straight to `runCommand` without
-- recording themselves, so `4K` then `Again` re-ran whatever had been typed
-- before — or nothing at all on a session where nothing had been.
ok, err = press('4K')
require_(ok, '"4K" threw: ' .. tostring(err))
vals.fontScale = 1.0
ok, err = press('Again')
require_(ok, '"Again" threw: ' .. tostring(err))
require_(vals.fontScale == 2,
  '"Again" did not repeat the quick button before it; scale is '
  .. tostring(vals.fontScale))

-- And the typed path still records itself.
liveConsole.run('--scale 1.5')
vals.fontScale = 1.0
ok, err = press('Again')
require_(ok, '"Again" after a typed command threw: ' .. tostring(err))
require_(vals.fontScale == 1.5,
  '"Again" did not repeat a typed command; scale is ' .. tostring(vals.fontScale))

-- An unknown command is answered rather than swallowed.
local unknownFrom = #drawn
liveConsole.run('--nonsense')
ok, err = pcall(script.windowSettings, 0.016)
require_(ok, 'drawing after an unknown command threw: ' .. tostring(err))
local said = table.concat(drawn, '\n', unknownFrom + 1, #drawn)
require_(said:find('unknown', 1, true) ~= nil,
  'an unknown command was swallowed instead of answered')

for key, value in pairs(liveStore.DEFAULTS) do vals[key] = value end
print('every button in the settings window does what it says: OK')


-- ---------------------------------------------------------------------------
-- A published frame with no car in it is not the same as no application
--
-- The application publishes from its launcher screen and while AC has nothing
-- in shared memory, so the panel is reachable in the garage. It has to say
-- which of the two states it is in: "AC Pro Engineer is not running" sends
-- someone hunting through the bridge and the Proton prefix, and in the garage
-- there is nothing there to find.
-- ---------------------------------------------------------------------------

local garageFrom = #drawn
carPresent = false
local idled, idleError = loadPanel()
if idled then idled, idleError = pcall(script.update, 0.016) end
if idled then idled, idleError = pcall(script.windowMain, 0.016) end
if idled then idled, idleError = pcall(script.windowEngineer, 0.016) end
carPresent = true
if not idled then
  print('\nFAILED: the panel threw with no car in the frame: ' .. tostring(idleError))
  os.exit(1)
end

local garage = table.concat(drawn, '\n', garageFrom + 1, #drawn)
if not garage:find('Waiting for the car', 1, true) then
  print('\nFAILED: with CONNECTED clear the panel did not say it is waiting for')
  print('the car. It drew:')
  for i = garageFrom + 1, math.min(#drawn, garageFrom + 12) do print('  ' .. drawn[i]) end
  os.exit(1)
end
if garage:find('AC Pro Engineer is not running', 1, true) then
  print('\nFAILED: the panel says the application is not running while reading')
  print('frames from it.')
  os.exit(1)
end
print('no car is not no application: OK')

-- Last, and after a reload. The combinations below leave the panel holding
-- demo numbers and developer switches, and every check above reads what the
-- panel drew — running them in the middle left "waiting for the car" looking
-- at a demo frame from two combinations earlier.
local combosLoaded, combosError = loadPanel()
if not combosLoaded then
  print('\nFAILED: reload before the setting combinations: ' .. tostring(combosError))
  os.exit(1)
end
pcall(script.update, 0.016)

-- ---------------------------------------------------------------------------
-- Every window, under settings nobody runs by default
--
-- Everything above draws the panel as it ships. A driver who turns things on
-- is running code no check has ever executed: one-line mode, VR, every block
-- off, every block on, the largest and smallest text. Each of those takes a
-- different path through the layout, and a nil or an arithmetic-on-nil in one
-- of them is a window that draws an error instead of the telemetry.
-- ---------------------------------------------------------------------------

local live = package.loaded['acpe.settings'].values

--- Put the settings back the way they ship, so one combination cannot leak
--- into the next.
local function resetSettings()
  for key, value in pairs(package.loaded['acpe.settings'].DEFAULTS) do
    live[key] = value
  end
end

local COMBINATIONS = {
  { name = 'one-line mode', apply = function() live.hudMode = true end },
  { name = 'VR', apply = function() live.vrMode = true end },
  { name = 'largest text', apply = function() live.textSize = 'large' end },
  { name = 'compact text', apply = function() live.textSize = 'compact' end },
  { name = 'no auto scale', apply = function() live.autoScale = false end },
  { name = 'no section captions', apply = function() live.sectionLabels = false end },
  {
    name = 'every block off',
    apply = function()
      for _, key in ipairs(package.loaded['acpe.settings'].KEYS) do
        if key:match('^show') and type(live[key]) == 'boolean' then live[key] = false end
      end
    end,
  },
  {
    name = 'every block on, every marker word',
    apply = function()
      for _, key in ipairs(package.loaded['acpe.settings'].KEYS) do
        if key:match('^show') and type(live[key]) == 'boolean' then live[key] = true end
      end
      live.engineerSeverityWord = true
      live.engineerNumbered = true
      live.engineerUppercase = true
      live.engineerSpacing = true
      live.engineerSeparator = true
      live.engineerBackground = 0.6
      live.engineerBullet = 'dot'
      live.engineerLines = 8
    end,
  },
  { name = 'two columns', apply = function() live.columnsPerRow = 2 end },
  { name = 'developer, demo numbers', apply = function()
      live.devMode = true; live.devDemo = true; live.devSampleAdvice = true
    end },
}

local WINDOWS = { 'windowMain', 'windowEngineer', 'windowSettings',
  'windowTelemetry', 'windowStatus' }

for _, combination in ipairs(COMBINATIONS) do
  resetSettings()
  combination.apply()
  pcall(script.update, 0.016)
  for _, window in ipairs(WINDOWS) do
    local drew, drawError = pcall(script[window], 0.016)
    if not drew then
      print('\nFAILED: ' .. window .. ' with ' .. combination.name .. ': '
        .. tostring(drawError))
      os.exit(1)
    end
  end
end
resetSettings()
print('every window under ' .. #COMBINATIONS .. ' setting combinations: OK')

-- Sixteen is enough to see the panel came up; ACPE_ALL=1 prints the lot, which
-- is how a wrong unit or an untranslated caption is spotted without the game.
local show = os.getenv('ACPE_ALL') and #drawn or math.min(#drawn, 16)
print('\nrendered ' .. #drawn .. ' pieces of text:')
for i = 1, show do print('  ' .. drawn[i]) end
