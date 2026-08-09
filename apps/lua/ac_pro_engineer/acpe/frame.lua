-- The shared block, and the snapshot the panel draws from.
--
-- Nothing in here draws. `update` copies fields out of the mapping when the
-- writer says the struct is settled, hands the numbers to `acpe.format` to
-- become strings, and tells `acpe.i18n` which language the application is
-- running in. Every window then reads `shown` and never the mapping.

local layout = require('frame_layout')
local format = require('acpe.format')
local i18n = require('acpe.i18n')
local settings = require('acpe.settings').values

local M = {}

-- Must match ac_core::overlay::frame::OVERLAY_MMF_NAME.
local MMF_NAME = 'AcTools.CSP.Limited.ACPE.v1'

-- How long the sequence may stand still before the application is presumed
-- gone. Two seconds rides out a stalled tick without letting anyone read
-- frozen numbers as live ones.
local LIVENESS_TIMEOUT = 2.0

-- Named fields, not an array: CSP returns raw cdata for an array of strings,
-- and the advice reached the panel as `cdata<char (&)[64]>`.
--
-- `#MESSAGE_KEYS` is the panel's only statement of how many slots the frame
-- has — everything below counts from it rather than from a literal 4, which is
-- what had to be found in six places when the frame grew to eight.
local MESSAGE_KEYS = {
  'message_0', 'message_1', 'message_2', 'message_3',
  'message_4', 'message_5', 'message_6', 'message_7',
}
local MESSAGE_SLOTS = #MESSAGE_KEYS

-- The debrief, named the same way and for the same reason. Laid out lap by
-- lap: `DEBRIEF_KEYS[lap][line]`. Counting from `#DEBRIEF_KEYS` rather than
-- from a literal 3 is what stops the next change to the frame having to be
-- found in six places.
local DEBRIEF_KEYS = {
  { 'debrief_0_0', 'debrief_0_1', 'debrief_0_2', 'debrief_0_3', 'debrief_0_4', 'debrief_0_5', 'debrief_0_6', 'debrief_0_7' },
  { 'debrief_1_0', 'debrief_1_1', 'debrief_1_2', 'debrief_1_3', 'debrief_1_4', 'debrief_1_5', 'debrief_1_6', 'debrief_1_7' },
  { 'debrief_2_0', 'debrief_2_1', 'debrief_2_2', 'debrief_2_3', 'debrief_2_4', 'debrief_2_5', 'debrief_2_6', 'debrief_2_7' },
}
local DEBRIEF_LAPS = #DEBRIEF_KEYS
local DEBRIEF_LINES = #DEBRIEF_KEYS[1]

-- The frame version this panel reads and the release it came from. Both are
-- declared in ac_pro_engineer.lua, which is the file the installer greps and
-- the tests check, and handed here at load time.
local EXPECTED_VERSION = 0
local PANEL_VERSION = '0.0.0'

local frame = nil
local openError = nil
local secondsSinceOpenAttempt = 0

--- Open the mapping the desktop application publishes.
---
--- Retried rather than attempted once: the game is usually started first, and
--- a panel that decided at load time that there is no shared memory would stay
--- wrong for the rest of the session — which is exactly what it did.
local function openFrame()
  local opened, err = pcall(function()
    frame = ac.readMemoryMappedFile(MMF_NAME, layout)
  end)
  if opened then
    openError = nil
  else
    frame = nil
    openError = tostring(err)
  end
  return opened
end

openFrame()

-- Last settled snapshot, allocated once and only ever overwritten.
local shown = {
  version = 0, sequence = 0,
  -- The release the *application* is on. Compared against PANEL_VERSION to
  -- notice a panel the game loaded before the application was updated.
  app_version = '',
  speed_kmh = 0, rpm = 0, max_rpm = 0, gear = 0,
  fuel_litres = 0, fuel_laps_remaining = 0, fuel_per_lap = 0,
  delta_seconds = 0, best_lap_ms = 0, last_lap_ms = 0, current_lap_ms = 0,
  air_temp_c = 0, road_temp_c = 0, surface_grip = 0,
  lap_count = 0, position = 0,
  target_pressure_front = 0, target_pressure_rear = 0,
  flags = 0, message_count = 0,
  tyre_pressure_psi = { 0, 0, 0, 0 },
  tyre_temp_c = { 0, 0, 0, 0 },
  tyre_wear_percent = { 0, 0, 0, 0 },
  brake_temp_c = { 0, 0, 0, 0 },
  -- Sized from the frame, not written out: a table one shorter than the frame
  -- reads the last slot as nil and the advice quietly stops at seven lines.
  messages = {},
  message_severity = {},
  -- One entry per published lap: its number, its time, and its lines.
  debrief_lap_count = 0,
  debrief = {},
  best_sector_ms = { 0, 0, 0 },
  tyre_temp_inner_c = { 0, 0, 0, 0 },
  tyre_temp_outer_c = { 0, 0, 0, 0 },
  tyre_laps_remaining = { -1, -1, -1, -1 },
}
for i = 1, MESSAGE_SLOTS do
  shown.messages[i] = ''
  shown.message_severity[i] = 0
end
for lap = 1, DEBRIEF_LAPS do
  shown.debrief[lap] = { lap_number = 0, lap_time_ms = 0, lines = {}, severity = {},
    sectors = { 0, 0, 0 } }
  for line = 1, DEBRIEF_LINES do
    shown.debrief[lap].lines[line] = ''
    shown.debrief[lap].severity[line] = 0
  end
end

-- Liveness. The sequence standing still for `LIVENESS_TIMEOUT` is how the
-- application's absence is detected — there is no other channel, and a frame
-- that stopped arriving looks exactly like one that never did.
local lastSequence = -1
local secondsSinceChange = 0
local isLive = false

-- Bit flags, matching ac_core::overlay::frame::flags.
local FLAG_PIT_LIMITER   = 1
local FLAG_CONNECTED     = 2
local FLAG_SHOW_TELEMETRY = 4
local FLAG_SHOW_ENGINEER = 8
local FLAG_FUEL_WARNING  = 16
local FLAG_SHOW_SESSION  = 32
local FLAG_SHOW_TIMING   = 64
local FLAG_SHOW_FUEL     = 128
local FLAG_RUSSIAN       = 256

M.FLAG_PIT_LIMITER = FLAG_PIT_LIMITER
M.FLAG_CONNECTED = FLAG_CONNECTED
M.FLAG_SHOW_TELEMETRY = FLAG_SHOW_TELEMETRY
M.FLAG_SHOW_ENGINEER = FLAG_SHOW_ENGINEER
M.FLAG_FUEL_WARNING = FLAG_FUEL_WARNING
M.FLAG_SHOW_SESSION = FLAG_SHOW_SESSION
M.FLAG_SHOW_TIMING = FLAG_SHOW_TIMING
M.FLAG_SHOW_FUEL = FLAG_SHOW_FUEL

-- Every section the panel can hide, and the name it goes by in the developer
-- windows. One list rather than two that drift.
M.FLAG_NAMES = {
  { 'pit limiter', FLAG_PIT_LIMITER },
  { 'connected', FLAG_CONNECTED },
  { 'telemetry', FLAG_SHOW_TELEMETRY },
  { 'engineer', FLAG_SHOW_ENGINEER },
  { 'fuel warning', FLAG_FUEL_WARNING },
  { 'session', FLAG_SHOW_SESSION },
  { 'lap timing', FLAG_SHOW_TIMING },
  { 'fuel', FLAG_SHOW_FUEL },
}

local function hasFlag(flag)
  -- Developer mode can answer yes to every section: the layout has to be
  -- judged with everything on screen, and a real session rarely shows all of
  -- it at once.
  if settings.devIgnoreFlags and flag >= FLAG_SHOW_TELEMETRY then return true end
  return bit.band(shown.flags, flag) ~= 0
end

--- Is the panel the game has loaded older than the one the application ships?
---
--- The application rewrites the panel's files at startup, but a game that was
--- already running keeps drawing the copy it loaded. Nothing on either side can
--- see that: the files on disk are current, the frame version still matches, and
--- the panel carries on. This is the only place the two versions meet.
---
--- Empty means the application predates the field, which is not a mismatch —
--- an older application with a newer panel is not something to nag about.
local function panelIsStale()
  if not settings.showUpdateNotice then return false end
  local running = shown.app_version
  if running == nil or running == '' then return false end
  return running ~= PANEL_VERSION
end

--- Copy the struct into `shown`, but only if it is settled.
local function readFrame()
  local seq = frame.sequence
  if seq % 2 ~= 0 then return false end
  if seq == lastSequence then return false end

  shown.version = frame.version
  shown.sequence = seq
  shown.app_version = frame.app_version
  shown.speed_kmh = frame.speed_kmh
  shown.rpm = frame.rpm
  shown.max_rpm = frame.max_rpm
  shown.gear = frame.gear
  shown.fuel_litres = frame.fuel_litres
  shown.fuel_laps_remaining = frame.fuel_laps_remaining
  shown.fuel_per_lap = frame.fuel_per_lap
  shown.delta_seconds = frame.delta_seconds
  shown.best_lap_ms = frame.best_lap_ms
  shown.last_lap_ms = frame.last_lap_ms
  shown.current_lap_ms = frame.current_lap_ms
  shown.air_temp_c = frame.air_temp_c
  shown.road_temp_c = frame.road_temp_c
  shown.surface_grip = frame.surface_grip
  shown.lap_count = frame.lap_count
  shown.position = frame.position
  shown.target_pressure_front = frame.target_pressure_front
  shown.target_pressure_rear = frame.target_pressure_rear
  shown.flags = frame.flags
  shown.message_count = frame.message_count

  for i = 1, 4 do
    shown.tyre_pressure_psi[i] = frame.tyre_pressure_psi[i - 1]
    shown.tyre_temp_c[i] = frame.tyre_temp_c[i - 1]
    shown.tyre_wear_percent[i] = frame.tyre_wear_percent[i - 1]
    shown.brake_temp_c[i] = frame.brake_temp_c[i - 1]
  end

  local count = math.min(frame.message_count, MESSAGE_SLOTS)
  for i = 1, count do
    shown.messages[i] = frame[MESSAGE_KEYS[i]]
    shown.message_severity[i] = frame.message_severity[i - 1]
  end
  for i = count + 1, MESSAGE_SLOTS do
    shown.messages[i] = ''
    shown.message_severity[i] = 0
  end

  shown.debrief_lap_count = math.min(frame.debrief_lap_count or 0, DEBRIEF_LAPS)
  for lap = 1, DEBRIEF_LAPS do
    local entry = shown.debrief[lap]
    entry.lap_number = frame.debrief_lap_number[lap - 1]
    entry.lap_time_ms = frame.debrief_lap_time_ms[lap - 1]
    -- Clamped: a line count larger than the slots would read past the end of
    -- the keys and hand the window nil where a sentence should be.
    local lines = math.min(frame.debrief_line_count[lap - 1], DEBRIEF_LINES)
    for line = 1, lines do
      entry.lines[line] = frame[DEBRIEF_KEYS[lap][line]]
      entry.severity[line] = frame.debrief_severity[(lap - 1) * DEBRIEF_LINES + line - 1]
    end
    for line = lines + 1, DEBRIEF_LINES do
      entry.lines[line] = ''
      entry.severity[line] = 0
    end
    entry.line_count = lines
    for sector = 1, 3 do
      entry.sectors[sector] = frame.debrief_sector_ms[(lap - 1) * 3 + sector - 1]
    end
  end

  for i = 1, 3 do shown.best_sector_ms[i] = frame.best_sector_ms[i - 1] end
  for i = 1, 4 do
    shown.tyre_temp_inner_c[i] = frame.tyre_temp_inner_c[i - 1]
    shown.tyre_temp_outer_c[i] = frame.tyre_temp_outer_c[i - 1]
    shown.tyre_laps_remaining[i] = frame.tyre_laps_remaining[i - 1]
  end

  if frame.sequence ~= seq then return false end
  lastSequence = seq
  return true
end

-- Numbers that look like a car on a warm lap. Only reachable from developer
-- mode, and the panel says so, so nobody mistakes them for telemetry.
--
-- Declared here, above `script.update` and `drawEngineerBody`, because both
-- call into it. Sitting below them made `applyDemo` and `DEMO_ADVICE` globals
-- to their callers -- that is, nil -- so turning on either developer switch
-- took the panel down: "Demo numbers" called nil, "Sample advice" indexed it.
-- Neither is on by default, which is why every harness passed for as long as
-- this was here.
-- One per slot, so developer mode shows the panel at its fullest rather than
-- half of it: "eight lines fits" is a layout question, and four lines cannot
-- answer it.
local DEMO_ADVICE = {
  'Fuel is fine for the stint',
  'Rear tyres are going off, ease the traction',
  'Box this lap',
  'Front-left pressure is 0.4 psi low and the corner is running cold in sector two',
  'Front brakes are past 700 C, open the ducts',
  'You are coasting into turn 4, brake later',
  'Rear camber is too negative for this compound',
  'Traction control is cutting on corner exit',
}

-- Three finished laps for "draw without a session": a full one, a shorter one
-- and a lap with a single critical line, so the switcher, the severity colours
-- and the "this lap had less to say" case are all reachable without a car.
local DEMO_DEBRIEF = {
  { lap_number = 12, lap_time_ms = 91234, sectors = { 28540, 31120, 31574 }, lines = {
    { 'Fronts over 28.4 psi (target 27.5)', 1 },
    { 'Front: inner edge running hot (I-O: 15.0C)', 1 },
    { 'Lockups: 4', 0 },
    { 'Coasting 18%', 0 },
  } },
  { lap_number = 11, lap_time_ms = 92871, sectors = { 28980, 31640, 32251 }, lines = {
    { 'All four cold 62C', 0 },
    { 'Rear: outer edge hotter (I-O: -6.0C)', 0 },
  } },
  { lap_number = 10, lap_time_ms = 95002, sectors = { 29800, 32400, 32802 }, lines = {
    { 'FL/RL overheating 815C', 2 },
  } },
}

local function applyDemo()
  shown.version = EXPECTED_VERSION
  shown.app_version = PANEL_VERSION
  shown.speed_kmh = 214
  shown.rpm, shown.max_rpm, shown.gear = 7400, 8500, 5
  shown.fuel_litres, shown.fuel_per_lap, shown.fuel_laps_remaining = 41.2, 3.1, 13.3
  shown.delta_seconds = -0.284
  shown.best_lap_ms, shown.last_lap_ms, shown.current_lap_ms = 91380, 92450, 34120
  shown.position, shown.lap_count = 4, 7
  shown.air_temp_c, shown.road_temp_c, shown.surface_grip = 22, 31, 0.97
  for i = 1, 4 do
    shown.tyre_pressure_psi[i] = 26.8 + i * 0.2
    shown.tyre_temp_c[i] = 78 + i * 7
    shown.tyre_wear_percent[i] = 99 - i * 3
    shown.brake_temp_c[i] = 320 + i * 90
  end
  shown.flags = 2 + 4 + 8 + 32 + 64 + 128
  shown.message_count = MESSAGE_SLOTS
  for i = 1, MESSAGE_SLOTS do
    shown.messages[i] = DEMO_ADVICE[i]
    shown.message_severity[i] = (i - 1) % 3
  end

  shown.best_sector_ms = { 28540, 31120, 31574 }
  for i = 1, 4 do
    shown.tyre_temp_inner_c[i] = 92 + i * 3
    shown.tyre_temp_outer_c[i] = 84 + i * 2
    shown.tyre_laps_remaining[i] = 12 - i * 1.5
  end
  shown.debrief_lap_count = math.min(#DEMO_DEBRIEF, DEBRIEF_LAPS)
  for lap = 1, DEBRIEF_LAPS do
    local demo = DEMO_DEBRIEF[lap]
    local entry = shown.debrief[lap]
    entry.lap_number = demo and demo.lap_number or 0
    entry.lap_time_ms = demo and demo.lap_time_ms or 0
    entry.line_count = demo and math.min(#demo.lines, DEBRIEF_LINES) or 0
    entry.sectors = demo and demo.sectors or { 0, 0, 0 }
    for line = 1, DEBRIEF_LINES do
      local source = demo and demo.lines[line]
      entry.lines[line] = source and source[1] or ''
      entry.severity[line] = source and source[2] or 0
    end
  end
end

-- How long a line of advice counts as new, and when each one was first seen.
--
-- Keyed by the sentence rather than by slot: the application packs whatever it
-- has to say into the slots in order, so a line that was second last lap can
-- be first this lap without having changed. Comparing slots would call every
-- line new every time something above it cleared.
local NEW_FOR_SECONDS = 6.0
local firstSeen = {}
local clock = 0

--- Remember which sentences are on this frame, and forget the ones that left.
---
--- Pruned every time, so this cannot grow across a session: the table only
--- ever holds what the application is currently saying, which is at most the
--- eight slots the frame carries.
local function markMessageArrivals()
  local present = {}
  for i = 1, MESSAGE_SLOTS do
    local text = shown.messages[i]
    if text ~= nil and text ~= '' then
      present[text] = true
      if firstSeen[text] == nil then firstSeen[text] = clock end
    end
  end
  for text in pairs(firstSeen) do
    if not present[text] then firstSeen[text] = nil end
  end
end

--- Has the advice in this slot only just arrived?
---
--- What makes a new warning look different from the one that has been sitting
--- there for four laps. Without it they are the same three words in the same
--- colour, and the eye stops going to them.
function M.messageIsNew(index)
  local text = shown.messages[index]
  if text == nil or text == '' then return false end
  local seen = firstSeen[text]
  return seen ~= nil and (clock - seen) < NEW_FOR_SECONDS
end

--- One tick. Called by `script.update`, which is the only caller.
function M.update(dt)
  -- Frozen on purpose: a held frame is the only way to read a number that was
  -- there for a tenth of a second.
  if settings.freezeDisplay then return end

  clock = clock + dt

  if settings.devDemo then
    applyDemo()
    format.rebuild(shown)
    markMessageArrivals()
    i18n.speak(bit.band(shown.flags, FLAG_RUSSIAN) ~= 0)
    isLive = true
    return
  end

  if frame == nil then
    -- Keep trying, quietly. Once the application starts, the panel fills in on
    -- its own instead of needing the window closed and opened again.
    secondsSinceOpenAttempt = secondsSinceOpenAttempt + dt
    if secondsSinceOpenAttempt >= 2.0 then
      secondsSinceOpenAttempt = 0
      openFrame()
    end
    return
  end

  if readFrame() then
    format.rebuild(shown)
    markMessageArrivals()
    i18n.speak(bit.band(shown.flags, FLAG_RUSSIAN) ~= 0)
    secondsSinceChange = 0
    isLive = shown.version == EXPECTED_VERSION and shown.sequence ~= 0
  else
    secondsSinceChange = secondsSinceChange + dt
  end

  -- The sequence standing still is how the application's absence is detected.
  if secondsSinceChange > LIVENESS_TIMEOUT then
    isLive = false
  end
end

--- Tell this module which versions the entry point declares.
---
--- They live in ac_pro_engineer.lua because that is the file the installer
--- reads to report what is installed and the file two cargo tests check. This
--- module is where they are compared against a frame.
function M.configure(expected, panel)
  EXPECTED_VERSION = expected
  PANEL_VERSION = panel
end

-- Reached by the LuaJIT harness so the arrival rule can be driven directly.
-- The alternative is publishing eight frames a second apart and hoping, which
-- tests the harness's patience rather than the rule.
M.markArrivalsForTest = markMessageArrivals
M.advanceClockForTest = function(seconds) clock = clock + seconds end

M.MMF_NAME = MMF_NAME
M.MESSAGE_KEYS = MESSAGE_KEYS
M.MESSAGE_SLOTS = MESSAGE_SLOTS
M.DEBRIEF_KEYS = DEBRIEF_KEYS
M.DEBRIEF_LAPS = DEBRIEF_LAPS
M.DEBRIEF_LINES = DEBRIEF_LINES
M.DEMO_ADVICE = DEMO_ADVICE
M.debugReadFrame = function() return readFrame(), frame ~= nil, tostring(frame and frame.sequence) end
M.shown = shown
M.hasFlag = hasFlag
M.panelIsStale = panelIsStale

function M.live() return isLive end
function M.openError() return openError end
function M.secondsSinceChange() return secondsSinceChange end
function M.expectedVersion() return EXPECTED_VERSION end
function M.panelVersion() return PANEL_VERSION end
function M.opened() return frame ~= nil end

return M
