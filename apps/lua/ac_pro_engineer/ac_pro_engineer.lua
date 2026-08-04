-- AC Pro Engineer overlay.
--
-- This script computes nothing. Every value it draws was calculated by the
-- desktop application and published into shared memory; all that happens here
-- is reading fields out of a struct and handing them to ImGui.
--
-- That split is deliberate. This code runs on Assetto Corsa's render thread,
-- where a millisecond is a sixth of the frame budget at 165 Hz, and LuaJIT
-- collects garbage mid-frame — so anything that parses text, builds tables or
-- formats strings every frame shows up as a stutter rather than as a lower
-- average frame rate.
--
-- The rules that keep it cheap:
--   * nothing is allocated per frame that can be allocated once
--   * `script.update` copies only the fields drawn, and only when the writer
--     says the struct is settled
--   * colours are picked from preallocated constants, never built with rgbm()
--     inside the draw path

local layout = require('frame_layout')

-- Must match ac_core::overlay::frame::OVERLAY_VERSION.
local EXPECTED_VERSION = 1

-- Must match ac_core::overlay::frame::OVERLAY_MMF_NAME.
local MMF_NAME = 'AcTools.CSP.Limited.ACPE.v1'

-- How long the sequence may stand still before the application is presumed
-- gone. Two seconds rides out a stalled tick without letting anyone read
-- frozen numbers as live ones.
local LIVENESS_TIMEOUT = 2.0

-- Preallocated. `rgbm()` allocates, and building the same handful of colours
-- sixty times a second is exactly the garbage this design exists to avoid.
local COLOR = {
  dim       = rgbm(0.45, 0.48, 0.52, 1),
  label     = rgbm(0.62, 0.66, 0.72, 1),
  text      = rgbm(0.88, 0.90, 0.94, 1),
  accent    = rgbm(0.20, 0.72, 1.00, 1),
  good      = rgbm(0.35, 0.85, 0.45, 1),
  warn      = rgbm(1.00, 0.76, 0.20, 1),
  bad       = rgbm(1.00, 0.34, 0.34, 1),
  cold      = rgbm(0.38, 0.60, 1.00, 1),
  purple    = rgbm(0.76, 0.44, 1.00, 1),
  barBack   = rgbm(0.16, 0.17, 0.20, 1),
  panel     = rgbm(0.10, 0.11, 0.13, 0.55),
  limiter   = rgbm(1.00, 0.62, 0.10, 1),
}

local TYRE_LABEL = { 'FL', 'FR', 'RL', 'RR' }

-- ---------------------------------------------------------------------------
-- Settings
-- ---------------------------------------------------------------------------
--
-- What the panel shows and in which units. These are read in the draw path, so
-- they are plain fields on a table that is written only when the settings
-- window is open — never computed per frame.
--
-- CSP keeps them across sessions through `ac.storage`. Everywhere else — the
-- LuaJIT harness, the LÖVE harness before it installs one — the defaults
-- simply stand, which is why the storage call is guarded rather than assumed.

local DEFAULTS = {
  showRpmBar = true,
  showTyres = true,
  showBrakes = true,
  showTiming = true,
  showFuel = true,
  showEngineer = true,
  sectionLabels = true,
  celsius = true,
  psi = true,
}

local settings = {}
for key, value in pairs(DEFAULTS) do settings[key] = value end

if type(ac) == 'table' and type(ac.storage) == 'function' then
  local storageOk, stored = pcall(ac.storage, DEFAULTS, 'acpe.')
  if storageOk and stored ~= nil then settings = stored end
end

-- Colours are decided from the real Celsius and psi values; only the text that
-- reaches the screen is converted. A warning that changes meaning with the
-- unit setting would be a bug, not a feature.
local function tempText(celsius)
  if settings.celsius then
    return string.format('%.0f°', celsius)
  end
  return string.format('%.0f°', celsius * 1.8 + 32)
end

local function pressureText(psi)
  if settings.psi then
    return string.format('%.1f', psi)
  end
  return string.format('%.2f', psi * 0.0689476)
end

local frame = nil
local openError = nil

local ok, err = pcall(function()
  frame = ac.readMemoryMappedFile(MMF_NAME, layout)
end)
if not ok then
  openError = tostring(err)
end

-- Last settled snapshot, allocated once and only ever overwritten.
local shown = {
  version = 0, sequence = 0,
  speed_kmh = 0, rpm = 0, max_rpm = 0, gear = 0,
  fuel_litres = 0, fuel_laps_remaining = 0, fuel_per_lap = 0,
  delta_seconds = 0, best_lap_ms = 0, last_lap_ms = 0,
  flags = 0, message_count = 0,
  tyre_pressure_psi = { 0, 0, 0, 0 },
  tyre_temp_c = { 0, 0, 0, 0 },
  tyre_wear_percent = { 0, 0, 0, 0 },
  brake_temp_c = { 0, 0, 0, 0 },
  messages = { '', '', '', '' },
}

local lastSequence = -1
local secondsSinceChange = 0
local isLive = false

-- Bit flags, matching ac_core::overlay::frame::flags.
local FLAG_PIT_LIMITER   = 1
local FLAG_CONNECTED     = 2
local FLAG_SHOW_TELEMETRY = 4
local FLAG_SHOW_ENGINEER = 8
local FLAG_FUEL_WARNING  = 16

local function hasFlag(flag)
  return bit.band(shown.flags, flag) ~= 0
end

--- Copy the struct into `shown`, but only if it is settled.
--
-- The writer keeps `sequence` odd while mid-write and bumps it by two per
-- update, so an odd value means we caught it in the middle, and a value that
-- moved across our read means it finished one while we copied. Either way the
-- previous snapshot is kept: skipping a frame at 60 Hz is invisible, a frame
-- spliced from two updates is a visible flicker.
local function readFrame()
  local seq = frame.sequence
  if seq % 2 ~= 0 then return false end
  if seq == lastSequence then return false end

  shown.version = frame.version
  shown.sequence = seq
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
  shown.flags = frame.flags
  shown.message_count = frame.message_count

  for i = 1, 4 do
    shown.tyre_pressure_psi[i] = frame.tyre_pressure_psi[i - 1]
    shown.tyre_temp_c[i] = frame.tyre_temp_c[i - 1]
    shown.tyre_wear_percent[i] = frame.tyre_wear_percent[i - 1]
    shown.brake_temp_c[i] = frame.brake_temp_c[i - 1]
  end

  local count = math.min(frame.message_count, 4)
  for i = 1, count do shown.messages[i] = frame.messages[i - 1] end
  for i = count + 1, 4 do shown.messages[i] = '' end

  if frame.sequence ~= seq then return false end

  lastSequence = seq
  return true
end

function script.update(dt)
  if frame == nil then return end

  if readFrame() then
    secondsSinceChange = 0
    isLive = shown.version == EXPECTED_VERSION and shown.sequence ~= 0
  else
    secondsSinceChange = secondsSinceChange + dt
  end

  -- The sequence standing still is how the application's absence is detected.
  -- No separate heartbeat is needed: the thing that proves it is alive is the
  -- thing it already sends.
  if secondsSinceChange > LIVENESS_TIMEOUT then
    isLive = false
  end
end

-- ---------------------------------------------------------------------------
-- Drawing
-- ---------------------------------------------------------------------------

local function gearText(gear)
  if gear < 0 then return 'R' end
  if gear == 0 then return 'N' end
  return tostring(gear)
end

local function rpmRatio()
  if shown.max_rpm <= 0 then return 0 end
  local r = shown.rpm / shown.max_rpm
  if r < 0 then return 0 end
  if r > 1 then return 1 end
  return r
end

local function rpmColor(ratio)
  if ratio > 0.95 then return COLOR.bad end
  if ratio > 0.85 then return COLOR.warn end
  return COLOR.good
end

local function tyreTempColor(temp)
  if temp < 70 then return COLOR.cold end
  if temp < 95 then return COLOR.good end
  if temp < 105 then return COLOR.warn end
  return COLOR.bad
end

local function wearColor(wear)
  if wear >= 96 then return COLOR.good end
  if wear >= 85 then return COLOR.warn end
  return COLOR.bad
end

local function brakeColor(temp)
  if temp < 150 then return COLOR.cold end
  if temp < 550 then return COLOR.good end
  if temp < 750 then return COLOR.warn end
  return COLOR.bad
end

local function lapTimeText(ms)
  if ms <= 0 then return '--:--.---' end
  local minutes = math.floor(ms / 60000)
  local seconds = math.floor((ms % 60000) / 1000)
  local millis = ms % 1000
  return string.format('%d:%02d.%03d', minutes, seconds, millis)
end

--- A label above a value, as its own column.
local function stat(label, value, color)
  ui.beginGroup()
  ui.pushFont(ui.Font.Tiny)
  ui.textColored(label, COLOR.label)
  ui.popFont()
  ui.textColored(value, color or COLOR.text)
  ui.endGroup()
end

--- The RPM bar. Drawn by hand rather than with progressBar so the redline
--- segment can be shaded separately.
local function rpmBar(width)
  local ratio = rpmRatio()
  local height = 6
  local origin = ui.getCursor()
  local to = vec2(origin.x + width, origin.y + height)

  ui.drawRectFilled(origin, to, COLOR.barBack, 2)
  if ratio > 0 then
    local filled = vec2(origin.x + width * ratio, origin.y + height)
    ui.drawRectFilled(origin, filled, rpmColor(ratio), 2)
  end
  ui.dummy(vec2(width, height))
end

local function drawHeader()
  ui.beginGroup()
  ui.pushFont(ui.Font.Huge)
  ui.textColored(string.format('%.0f', shown.speed_kmh), COLOR.text)
  ui.popFont()
  ui.endGroup()

  ui.sameLine()
  ui.beginGroup()
  ui.pushFont(ui.Font.Tiny)
  ui.textColored('KM/H', COLOR.label)
  ui.popFont()
  ui.pushFont(ui.Font.Title)
  ui.textColored(gearText(shown.gear), rpmColor(rpmRatio()))
  ui.popFont()
  ui.endGroup()

  if hasFlag(FLAG_PIT_LIMITER) then
    ui.sameLine()
    ui.pushFont(ui.Font.Small)
    ui.textColored('LIMITER', COLOR.limiter)
    ui.popFont()
  end

  if settings.showRpmBar then
    rpmBar(ui.availableSpaceX())
  end
end

--- A section caption, or nothing when the panel is set to run without them.
local function sectionLabel(text)
  if not settings.sectionLabels then return end
  ui.pushFont(ui.Font.Tiny)
  ui.textColored(text, COLOR.label)
  ui.popFont()
end

local function drawTyres()
  sectionLabel('TYRES')

  -- Two by two, the way they sit on the car, so a hot corner is where you
  -- expect it rather than third in a list.
  local columnWidth = ui.availableSpaceX() * 0.5
  for row = 0, 1 do
    for col = 0, 1 do
      local i = row * 2 + col + 1
      if col == 1 then ui.sameLine(columnWidth) end

      ui.beginGroup()
      ui.pushFont(ui.Font.Tiny)
      ui.textColored(TYRE_LABEL[i], COLOR.dim)
      ui.popFont()

      ui.sameLine()
      ui.textColored(pressureText(shown.tyre_pressure_psi[i]),
        tyreTempColor(shown.tyre_temp_c[i]))

      ui.pushFont(ui.Font.Tiny)
      ui.textColored(string.format('%s  %.0f%%',
        tempText(shown.tyre_temp_c[i]), shown.tyre_wear_percent[i]),
        wearColor(shown.tyre_wear_percent[i]))
      ui.popFont()
      ui.endGroup()
    end
  end
end

local function drawBrakes()
  sectionLabel('BRAKES')
  ui.pushFont(ui.Font.Tiny)
  for i = 1, 4 do
    ui.textColored(string.format('%s %s', TYRE_LABEL[i], tempText(shown.brake_temp_c[i])),
      brakeColor(shown.brake_temp_c[i]))
    if i < 4 then ui.sameLine() end
  end
  ui.popFont()
end

local function drawTiming()
  local deltaColor = COLOR.text
  if shown.delta_seconds < -0.001 then
    deltaColor = COLOR.good
  elseif shown.delta_seconds > 0.001 then
    deltaColor = COLOR.bad
  end

  -- The row width is taken once, before anything is drawn. Asking for the
  -- space left after each column gives the same answer every time — the cursor
  -- is back at the start of the line by then — and all three columns land on
  -- top of each other.
  local width = ui.availableSpaceX()

  stat('DELTA', string.format('%+.3f', shown.delta_seconds), deltaColor)
  ui.sameLine(width * 0.38)
  stat('BEST', lapTimeText(shown.best_lap_ms), COLOR.purple)
  ui.sameLine(width * 0.69)
  stat('LAST', lapTimeText(shown.last_lap_ms), COLOR.text)
end

local function drawFuel()
  local color = hasFlag(FLAG_FUEL_WARNING) and COLOR.bad or COLOR.text
  local width = ui.availableSpaceX()

  stat('FUEL', string.format('%.1f L', shown.fuel_litres), color)
  ui.sameLine(width * 0.38)
  stat('LAPS LEFT',
    shown.fuel_laps_remaining > 0 and string.format('%.1f', shown.fuel_laps_remaining) or '--',
    color)
  ui.sameLine(width * 0.69)
  stat('PER LAP',
    shown.fuel_per_lap > 0 and string.format('%.2f L', shown.fuel_per_lap) or '--',
    COLOR.dim)
end

local function drawAdvice()
  sectionLabel('ENGINEER')
  for i = 1, math.min(shown.message_count, 4) do
    if shown.messages[i] ~= '' then
      ui.textColored('• ' .. shown.messages[i], COLOR.warn)
    end
  end
end

--- What to show when there is nothing to show.
local function drawIdle(message, detail)
  local space = ui.availableSpace()
  ui.offsetCursorY(math.max(0, space.y * 0.35))
  ui.pushFont(ui.Font.Small)
  ui.textColored(message, COLOR.dim)
  if detail ~= nil then
    ui.pushFont(ui.Font.Tiny)
    ui.textColored(detail, COLOR.dim)
    ui.popFont()
  end
  ui.popFont()
end

function script.windowMain(dt)
  if openError ~= nil then
    ui.textColored('Shared memory unavailable', COLOR.bad)
    ui.pushFont(ui.Font.Tiny)
    ui.textColored(openError, COLOR.dim)
    ui.popFont()
    return
  end

  if not isLive then
    -- Deliberately quiet. A panel of stale numbers is worse than an empty one.
    drawIdle('AC Pro Engineer is not running', 'Start the desktop app to see telemetry')
    return
  end

  if shown.version ~= EXPECTED_VERSION then
    ui.textColored('Version mismatch', COLOR.bad)
    ui.pushFont(ui.Font.Tiny)
    ui.textColored(string.format('app v%d, overlay v%d', shown.version, EXPECTED_VERSION),
      COLOR.dim)
    ui.popFont()
    return
  end

  drawHeader()
  ui.offsetCursorY(4)

  -- Two gates on each section, and they mean different things: the flag is the
  -- application saying it has nothing to show, the setting is the driver
  -- saying they do not want to see it.
  if hasFlag(FLAG_SHOW_TELEMETRY) then
    if settings.showTyres then
      drawTyres()
      ui.offsetCursorY(2)
    end
    if settings.showBrakes then
      drawBrakes()
      ui.offsetCursorY(4)
    end
  end

  if settings.showTiming then
    drawTiming()
    ui.offsetCursorY(2)
  end

  if settings.showFuel then
    drawFuel()
  end

  if hasFlag(FLAG_SHOW_ENGINEER) and settings.showEngineer and shown.message_count > 0 then
    ui.offsetCursorY(4)
    drawAdvice()
  end
end

-- ---------------------------------------------------------------------------
-- Settings window
--
-- CSP opens this from the app's own title bar, and it is a separate window
-- from the panel: none of this runs while the overlay is merely being drawn.
-- ---------------------------------------------------------------------------

--- A checkbox bound to a settings field. `ui.checkbox` reports the click, not
--- the new value, so the flip happens here.
local function settingToggle(label, key)
  if ui.checkbox(label, settings[key]) then
    settings[key] = not settings[key]
  end
end

function script.windowSettings(dt)
  ui.pushFont(ui.Font.Tiny)
  ui.textColored('SECTIONS', COLOR.label)
  ui.popFont()

  settingToggle('RPM bar', 'showRpmBar')
  settingToggle('Tyres', 'showTyres')
  settingToggle('Brakes', 'showBrakes')
  settingToggle('Lap timing', 'showTiming')
  settingToggle('Fuel', 'showFuel')
  settingToggle('Engineer advice', 'showEngineer')
  settingToggle('Section captions', 'sectionLabels')

  ui.separator()
  ui.pushFont(ui.Font.Tiny)
  ui.textColored('UNITS', COLOR.label)
  ui.popFont()

  settingToggle('Celsius', 'celsius')
  settingToggle('PSI', 'psi')

  ui.pushFont(ui.Font.Tiny)
  ui.textColored(settings.celsius and 'temperatures in °C' or 'temperatures in °F', COLOR.dim)
  ui.textColored(settings.psi and 'pressures in psi' or 'pressures in bar', COLOR.dim)
  ui.popFont()

  ui.separator()
  if ui.button('Reset to defaults') then
    for key, value in pairs(DEFAULTS) do settings[key] = value end
  end

  -- Sections the application itself is suppressing. Without this the settings
  -- read as broken: a box is ticked and nothing appears.
  if not hasFlag(FLAG_SHOW_TELEMETRY) or not hasFlag(FLAG_SHOW_ENGINEER) then
    ui.separator()
    ui.pushFont(ui.Font.Tiny)
    if not hasFlag(FLAG_SHOW_TELEMETRY) then
      ui.textColored('Telemetry is switched off in the desktop app', COLOR.warn)
    end
    if not hasFlag(FLAG_SHOW_ENGINEER) then
      ui.textColored('Engineer advice is switched off in the desktop app', COLOR.warn)
    end
    ui.popFont()
  end
end
