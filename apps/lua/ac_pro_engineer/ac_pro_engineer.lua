-- AC Pro Engineer overlay.
-- This script computes nothing. Every value it draws was calculated by the
-- desktop application and published into shared memory; all that happens here
-- is reading fields out of a struct and handing them to ImGui.
-- That split is deliberate. This code runs on Assetto Corsa's render thread,
-- where a millisecond is a sixth of the frame budget at 165 Hz, and LuaJIT
-- collects garbage mid-frame so anything that parses text, builds tables or
-- formats strings every frame shows up as a stutter rather than as a lower
-- average frame rate.
-- The rules that keep it cheap:
--   * nothing is allocated per frame that can be allocated once
--   * `script.update` copies only the fields drawn, and only when the writer
--     says the struct is settled
--   * colours are picked from preallocated constants, never built with rgbm()
--     inside the draw path

local layout = require('frame_layout')

-- Must match ac_core::overlay::frame::OVERLAY_VERSION.
local EXPECTED_VERSION = 2
-- Must match ac_core::overlay::frame::OVERLAY_MMF_NAME.
local MMF_NAME = 'AcTools.CSP.Limited.ACPE.v1'
-- How long the sequence may stand still before the application is presumed
-- gone. Two seconds rides out a stalled tick without letting anyone read
-- frozen numbers as live ones.
local LIVENESS_TIMEOUT = 2.0

-- Preallocated constants
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

-- Four named fields, not an array: CSP returns raw cdata for an array of
-- strings, and the advice reached the panel as `cdata<char (&)[64]>`.
local MESSAGE_KEYS = { 'message_0', 'message_1', 'message_2', 'message_3' }

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
  textSize = 'normal',   -- compact | normal | large
  vrMode = false,        -- bigger text, thicker bar, more air between blocks
  showHeader = true,
  showRpmBar = true,
  showTyres = true,
  showTiming = true,
  showFuel = true,
  showSession = true,
  showEngineer = true,
  showLimiter = true,
  shiftLight = true,
  shiftAt = 0.95,          -- share of the rev range where the bar goes red
  hudMode = false,         -- one line: speed, gear, delta, fuel
  showTyreTemp = true,
  showBrakeTemp = true,
  showWear = true,
  showDelta = true,
  showBest = true,
  showLast = true,
  showFuelLitres = true,
  showLapsLeft = true,
  showPerLap = true,
  showPosition = true,
  showLapNumber = true,
  showCurrentLap = true,
  showConditions = true,
  pressureDecimals = 1,
  tyreCold = 70, tyreHot = 95, tyreOver = 105,
  brakeCold = 150, brakeHot = 550, brakeOver = 750,
  wearWarn = 96, wearBad = 85,
  columnsPerRow = 0,       -- 0 follows the layout, 2 or 3 force it
  mainWidth = 320, mainHeight = 420,
  engineerWidth = 300, engineerHeight = 170,
  settingsWidth = 560, settingsHeight = 680,

  -- How the advice reads. The application decides how many lines it publishes;
  -- these decide how many of them are drawn and what they look like.
  engineerLines = 4,
  engineerBullet = 'severity',   -- severity | > | dot | none
  engineerWrap = true,
  engineerHighlight = true,

  sectionLabels = true,
  devMode = false,
  contentWidth = 360,
  barHeight = 6,
  fontScale = 1.0,        -- everything the panel draws, in one number
  autoScale = true,       -- and it follows the window, so stretching grows it
  engineerScale = 1.0,    -- advice, sized on its own
  engineerMinSeverity = 0,-- 0 everything, 1 warnings up, 2 critical only
  engineerSpacing = false,
  engineerLineGap = 4,
  engineerSeparator = false,
  engineerBackground = 0.0,
  engineerShowCount = false,
  engineerMaxChars = 64,   -- a line longer than this is cut, not wrapped away
  engineerUppercase = false,
  engineerNumbered = false,
  engineerSeverityWord = false,
  showDebugBounds = false,
  freezeDisplay = false,
  devDemo = false,          -- plausible numbers with no application running
  devIgnoreFlags = false,   -- draw every section whatever the app asked for
  devIgnoreVersion = false, -- draw anyway when the versions disagree
  devSampleAdvice = false,  -- four lines, one of each severity, one very long
  background = 0.0,       -- panel backing, 0 for none
  accent = 'blue',
  colorText = '0.88,0.90,0.94',
  colorLabel = '0.62,0.66,0.72',
  colorDim = '0.45,0.48,0.52',
  colorGood = '0.35,0.85,0.45',
  colorWarn = '1.00,0.76,0.20',
  colorBad = '1.00,0.34,0.34',
  celsius = true,
  psi = true,
  mph = false,
  gallons = false,
  shortLapTimes = false,
  unitSuffix = true,
}

local settings = {}
local SETTING_KEYS = {}
for key, value in pairs(DEFAULTS) do
  settings[key] = value
  SETTING_KEYS[#SETTING_KEYS + 1] = key
end
table.sort(SETTING_KEYS)

local storageActive = false

if type(ac) == 'table' and type(ac.storage) == 'function' then
  local storageOk, stored = pcall(ac.storage, DEFAULTS, 'acpe.')
  if storageOk and stored ~= nil then
    settings = stored
    storageActive = true
  end
end

--- Write every setting back through the storage proxy.
---
--- Storage persists on write, so a value only reaches the disk if something
--- assigns it. Assigning all of them is how a settings screen that was edited
--- while storage was unavailable — or a default that was never touched — ends
--- up saved anyway.
local lastSaved = {}

--- Save when anything changed, whatever changed it.
---
--- Wiring every control to a save call missed the ones that set a value
--- directly — accents, palette entries, presets. Comparing the table against
--- what was last written catches all of them, and it only runs while the
--- settings window is open.
local function autoSave()
  local dirty = false
  for _, key in ipairs(SETTING_KEYS) do
    if lastSaved[key] ~= settings[key] then
      lastSaved[key] = settings[key]
      dirty = true
    end
  end
  if dirty then return true end
  return false
end

local function saveSettings()
  if not storageActive then return false end
  local ok = pcall(function()
    for _, key in ipairs(SETTING_KEYS) do
      settings[key] = settings[key]
    end
  end)
  return ok
end

-- Anything but a string here is a palette saved by an older build, or one that
-- came back from storage as something the panel cannot read. Either way the
-- default is the only safe answer: a palette that throws takes every window
-- down with it, and the way back would be a button nobody can see.
for _, key in ipairs({ 'colorText', 'colorLabel', 'colorDim', 'colorGood', 'colorWarn',
    'colorBad' }) do
  if type(settings[key]) ~= 'string' then settings[key] = DEFAULTS[key] end
end

-- Text size, as font tiers rather than a scale factor: CSP has no API for
-- scaling a font, but it does give five of them, and stepping between tiers is
-- what "bigger text" has to mean here.
local FONT_TIERS = {
  compact = { caption = 'Tiny',  body = 'Tiny',   hero = 'Title', gear = 'Small' },
  normal  = { caption = 'Tiny',  body = 'Main',   hero = 'Huge',  gear = 'Title' },
  large   = { caption = 'Small', body = 'Title',  hero = 'Huge',  gear = 'Huge' },
}

local TEXT_SIZES = { 'compact', 'normal', 'large' }
local BULLETS = { ['>'] = '> ', dot = '• ', none = '' }
local BULLET_NAMES = { 'severity', '>', 'dot', 'none' }

-- Severity as the desktop application shows it: red, yellow, green, with the
-- marker carrying the colour and the sentence staying readable. The emoji it
-- uses in the terminal are not in CSP's font, so the marker is the two
-- characters that read the same at a glance.
local SEVERITY_MARK = { [0] = 'i  ', [1] = '!  ', [2] = '!! ' }
local SEVERITY_WORD = { [0] = 'INFO ', [1] = 'WARN ', [2] = 'CRIT ' }
local SEVERITY_COLOR = { [0] = COLOR.good, [1] = COLOR.warn, [2] = COLOR.bad }

-- Base text sizes, multiplied by the driver's scale. CSP has five font tiers
-- and no way to scale them, which on a 4K screen means a panel nobody can
-- read; DirectWrite draws at any size, so that is what the panel uses.
local TEXT_BASE = { caption = 11, body = 15, hero = 38, gear = 22 }
local VR_BOOST = 1.35

-- What the layout was drawn against. A window twice that wide should show
-- everything twice the size rather than the same numbers with a field of empty
-- pixels beside them, which is what stretching used to do.
local DESIGN_WIDTH = 360

-- Measured once per window, not per item: `availableSpaceX` shrinks as the
-- cursor moves right, so asking it inside a column made the second column half
-- the size of the first.
local frameScale = 1
local frameWidth = 360

local function measureWindowScale()
  if not settings.autoScale then
    frameScale = 1
    return
  end
  local available = ui.availableSpaceX()
  if available <= 0 then
    frameScale = 1
    return
  end
  frameScale = math.max(0.75, math.min(2.5, available / DESIGN_WIDTH))
end

local function measureWindowWidth()
  frameWidth = math.max(120, ui.availableSpaceX())
end

local function windowScale()
  return frameScale
end

local function textSize(role)
  local base = TEXT_BASE[role] or TEXT_BASE.body
  local scale = (settings.fontScale or 1) * windowScale()
  if settings.vrMode then scale = scale * VR_BOOST end
  if settings.textSize == 'compact' then
    scale = scale * 0.85
  elseif settings.textSize == 'large' then
    scale = scale * 1.25
  end
  return base * scale
end

--- Draw a piece of the panel's text. One call instead of push/draw/pop, and
--- one place where the size of everything is decided.
local function say(role, text, color)
  ui.dwriteText(text, textSize(role), color)
end

local function pushRole(role)
  -- In a headset the panel is a metre away through lenses that blur the edges,
  -- so VR does not get its own layout — it gets the largest one, unless the
  -- driver has already asked for something larger still.
  local size = settings.textSize
  if settings.vrMode and size ~= 'large' then size = 'large' end
  local tier = FONT_TIERS[size] or FONT_TIERS.normal
  ui.pushFont(ui.Font[tier[role]] or ui.Font.Main)
end

--- Vertical air between blocks. Doubled in VR, where things that touch each
--- other read as one thing.
local function gap(pixels)
  ui.offsetCursorY(settings.vrMode and pixels * 2 or pixels)
end

-- Colours are decided from the real Celsius and psi values; only the text that
-- reaches the screen is converted. A warning that changes meaning with the
-- unit setting would be a bug, not a feature.
local function tempText(celsius)
  if settings.celsius then
    return string.format('%.0f°C', celsius)
  end
  return string.format('%.0f°F', celsius * 1.8 + 32)
end

local function gearText(gear)
  if gear < 0 then return 'R' end
  if gear == 0 then return 'N' end
  return tostring(gear)
end

local function lapTimeText(ms)
  if ms <= 0 then return settings.shortLapTimes and '--.--' or '--:--.---' end
  local minutes = math.floor(ms / 60000)
  local seconds = math.floor((ms % 60000) / 1000)
  local millis = ms % 1000
  if settings.shortLapTimes then
    return string.format('%d:%02d.%01d', minutes, seconds, math.floor(millis / 100))
  end
  return string.format('%d:%02d.%03d', minutes, seconds, millis)
end

--- Speed and volume in whichever unit the driver thinks in.
local function speedText(kmh)
  if settings.mph then
    return string.format(settings.unitSuffix and '%.0f mph' or '%.0f', kmh * 0.621371)
  end
  return string.format('%.0f', kmh)
end

local function volumeText(litres)
  if settings.gallons then
    return string.format(settings.unitSuffix and '%.1f gal' or '%.1f', litres * 0.264172)
  end
  return string.format(settings.unitSuffix and '%.1f L' or '%.1f', litres)
end

local PRESSURE_FORMAT = { [0] = '%.0f psi', '%.1f psi', '%.2f psi' }
local BAR_FORMAT = { [0] = '%.1f bar', '%.2f bar', '%.3f bar' }
local PLAIN_FORMAT = { [0] = '%.0f', '%.1f', '%.2f' }

local function pressureText(psi)
  local decimals = settings.pressureDecimals or 1
  if not settings.unitSuffix then
    return string.format(PLAIN_FORMAT[decimals] or '%.1f',
      settings.psi and psi or psi * 0.0689476)
  end
  if settings.psi then
    return string.format(PRESSURE_FORMAT[decimals] or '%.1f psi', psi)
  end
  return string.format(BAR_FORMAT[decimals] or '%.2f bar', psi * 0.0689476)
end

--- A section caption, or nothing when the panel is set to run without them.
local function sectionLabel(text)
  if not settings.sectionLabels then return end
  say('caption', text, COLOR.label)
end

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
  speed_kmh = 0, rpm = 0, max_rpm = 0, gear = 0,
  fuel_litres = 0, fuel_laps_remaining = 0, fuel_per_lap = 0,
  delta_seconds = 0, best_lap_ms = 0, last_lap_ms = 0, current_lap_ms = 0,
  air_temp_c = 0, road_temp_c = 0, surface_grip = 0,
  lap_count = 0, position = 0,
  flags = 0, message_count = 0,
  tyre_pressure_psi = { 0, 0, 0, 0 },
  tyre_temp_c = { 0, 0, 0, 0 },
  tyre_wear_percent = { 0, 0, 0, 0 },
  brake_temp_c = { 0, 0, 0, 0 },
  messages = { '', '', '', '' },
  message_severity = { 0, 0, 0, 0 },
}

local lastSequence = -1
local secondsSinceChange = 0
local isLive = false

-- Text is built when a frame arrives, not when one is drawn.
--
-- The application publishes about sixty times a second; AC draws at whatever
-- the car is running at, 165 or more. Formatting in the draw path meant the
-- same numbers were turned into the same strings three times over, and every
-- one of those strings is garbage for a collector that runs mid-frame.
local text = {
  speed = '0', gear = 'N',
  pressure = { '', '', '', '' },
  tyreTemp = { '', '', '', '' },
  brakeTemp = { '', '', '', '' },
  wear = { '', '', '', '' },
  delta = '+0.000', best = '', last = '', current = '',
  fuel = '', lapsLeft = '', perLap = '',
  position = '', lap = '', conditions = '',
}

-- Window sizes are CSP's business. A script can set size constraints through
-- `ac.setWindowSizeConstraints`, and doing so took the resize grip off every
-- window in the app — including the ones that had one before. Left to CSP.

-- Bit flags, matching ac_core::overlay::frame::flags.
local FLAG_PIT_LIMITER   = 1
local FLAG_CONNECTED     = 2
local FLAG_SHOW_TELEMETRY = 4
local FLAG_SHOW_ENGINEER = 8
local FLAG_FUEL_WARNING  = 16
local FLAG_SHOW_SESSION  = 32
local FLAG_SHOW_TIMING   = 64
local FLAG_SHOW_FUEL     = 128

local function hasFlag(flag)
  -- Developer mode can answer yes to every section: the layout has to be
  -- judged with everything on screen, and a real session rarely shows all of
  -- it at once.
  if settings.devIgnoreFlags and flag >= FLAG_SHOW_TELEMETRY then return true end
  return bit.band(shown.flags, flag) ~= 0
end

--- Copy the struct into `shown`, but only if it is settled.
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
  shown.current_lap_ms = frame.current_lap_ms
  shown.air_temp_c = frame.air_temp_c
  shown.road_temp_c = frame.road_temp_c
  shown.surface_grip = frame.surface_grip
  shown.lap_count = frame.lap_count
  shown.position = frame.position
  shown.flags = frame.flags
  shown.message_count = frame.message_count

  for i = 1, 4 do
    shown.tyre_pressure_psi[i] = frame.tyre_pressure_psi[i - 1]
    shown.tyre_temp_c[i] = frame.tyre_temp_c[i - 1]
    shown.tyre_wear_percent[i] = frame.tyre_wear_percent[i - 1]
    shown.brake_temp_c[i] = frame.brake_temp_c[i - 1]
  end

  local count = math.min(frame.message_count, 4)
  for i = 1, count do
    shown.messages[i] = frame[MESSAGE_KEYS[i]]
    shown.message_severity[i] = frame.message_severity[i - 1]
  end
  for i = count + 1, 4 do
    shown.messages[i] = ''
    shown.message_severity[i] = 0
  end

  if frame.sequence ~= seq then return false end
  lastSequence = seq
  return true
end

--- Turn the snapshot into the strings the panel draws.
---
--- Called once per settled frame and once when a setting that changes a format
--- is touched — never from the draw path.
local function formatFrame()
  text.speed = speedText(shown.speed_kmh)
  text.gear = gearText(shown.gear)

  for i = 1, 4 do
    text.pressure[i] = pressureText(shown.tyre_pressure_psi[i])
    text.tyreTemp[i] = 'T: ' .. tempText(shown.tyre_temp_c[i])
    text.brakeTemp[i] = 'B: ' .. tempText(shown.brake_temp_c[i])
    text.wear[i] = string.format('Wear: %.0f%%', shown.tyre_wear_percent[i])
  end

  text.delta = string.format('%+.3f', shown.delta_seconds)
  text.best = lapTimeText(shown.best_lap_ms)
  text.last = lapTimeText(shown.last_lap_ms)
  text.current = lapTimeText(shown.current_lap_ms)

  text.fuel = volumeText(shown.fuel_litres)
  text.lapsLeft = shown.fuel_laps_remaining > 0
    and string.format('%.1f', shown.fuel_laps_remaining) or '--'
  text.perLap = shown.fuel_per_lap > 0 and volumeText(shown.fuel_per_lap) or '--'

  text.position = shown.position > 0 and string.format('P%d', shown.position) or '--'
  text.lap = tostring(shown.lap_count)
  text.conditions = string.format('AIR %s   ROAD %s   GRIP %.0f%%',
    tempText(shown.air_temp_c), tempText(shown.road_temp_c), shown.surface_grip * 100)
end

function script.update(dt)

  -- Frozen on purpose: a held frame is the only way to read a number that was
  -- there for a tenth of a second.
  if settings.freezeDisplay then return end

  if settings.devDemo then
    applyDemo()
    formatFrame()
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
    formatFrame()
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

-- ---------------------------------------------------------------------------
-- Drawing
-- ---------------------------------------------------------------------------

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

-- The thresholds are settings, not constants. A slick at 95°C is in its window
-- and a hard at 95°C is cold, and the panel cannot know which is on the car.
local function tyreTempColor(temp)
  if temp < settings.tyreCold then return COLOR.cold end
  if temp < settings.tyreHot then return COLOR.good end
  if temp < settings.tyreOver then return COLOR.warn end
  return COLOR.bad
end

local function wearColor(wear)
  if wear >= settings.wearWarn then return COLOR.good end
  if wear >= settings.wearBad then return COLOR.warn end
  return COLOR.bad
end

local function brakeColor(temp)
  if temp < settings.brakeCold then return COLOR.cold end
  if temp < settings.brakeHot then return COLOR.good end
  if temp < settings.brakeOver then return COLOR.warn end
  return COLOR.bad
end

--- A label above a value, as its own column.
local function stat(label, value, color)
  ui.beginGroup()
  say('caption', label, COLOR.label)
  say('body', value, color or COLOR.text)
  ui.endGroup()
end

-- How wide the content is allowed to get, whatever the window does.
--
-- A CSP window is resized by the driver and can end up half a screen wide.
-- Columns measured against the whole width then drift apart until a block of
-- four tyres reads as four unrelated numbers, so the layout keeps to a column
-- and lets the rest of the window be empty.
-- Spacing, pinned rather than inherited.
--
-- The LÖVE harness lays the panel out with these exact numbers, so the layout
-- judged there is the layout that ships. Left to CSP's theme, the same code
-- comes out with different gaps in game and every decision made in the harness
-- has to be made again.
local ITEM_SPACING = vec2(6, 3)
local ITEM_SPACING_VR = vec2(8, 8)
local FRAME_PADDING = vec2(6, 3)

-- CSP's theme is whatever the driver picked for CSP — red accents, in the
-- default one — and it repainted every tab, checkbox and slider in this panel.
-- The colours below are the harness's, so the thing being designed and the
-- thing being driven are the same colour.
-- Accent presets. The panel is looked at through a windscreen, so this is not
-- decoration: a colour that disappears against the track is a readout nobody
-- reads.
local ACCENTS = {
  blue   = rgbm(0.20, 0.72, 1.00, 1),
  teal   = rgbm(0.20, 0.85, 0.75, 1),
  amber  = rgbm(1.00, 0.72, 0.20, 1),
  violet = rgbm(0.76, 0.44, 1.00, 1),
  green  = rgbm(0.40, 0.90, 0.45, 1),
}
local ACCENT_NAMES = { 'blue', 'teal', 'amber', 'violet', 'green' }

-- Starting points by screen, so the first thing a driver sees is legible.
-- The third field keeps two buttons on a line.
-- The windows the manifest declares, and the sizes a script can pin them to.
local PRESETS = {
  { '1080p', { fontScale = 1.0, contentWidth = 360, barHeight = 6, textSize = 'normal' }, true },
  { '1440p', { fontScale = 1.35, contentWidth = 460, barHeight = 8, textSize = 'normal' }, false },
  { '4K', { fontScale = 2.0, contentWidth = 680, barHeight = 12, textSize = 'normal' }, true },
  { 'VR', { fontScale = 1.6, contentWidth = 520, barHeight = 14, vrMode = true }, false },
}

local function accentColor()
  return ACCENTS[settings.accent] or ACCENTS.blue
end

local STYLE_COLORS = {
  { 'Tab', rgbm(0.13, 0.16, 0.20, 1) },
  { 'TabHovered', rgbm(0.20, 0.42, 0.60, 1) },
  { 'TabActive', rgbm(0.16, 0.34, 0.50, 1) },
  { 'Header', rgbm(0.16, 0.34, 0.50, 0.8) },
  { 'HeaderHovered', rgbm(0.20, 0.48, 0.70, 0.9) },
  { 'HeaderActive', rgbm(0.20, 0.55, 0.80, 1) },
  { 'CheckMark', rgbm(0.20, 0.72, 1.00, 1) },
  { 'SliderGrab', rgbm(0.20, 0.60, 0.90, 1) },
  { 'SliderGrabActive', rgbm(0.20, 0.72, 1.00, 1) },
  { 'Button', rgbm(0.16, 0.17, 0.21, 1) },
  { 'ButtonHovered', rgbm(0.22, 0.40, 0.56, 1) },
  { 'ButtonActive', rgbm(0.20, 0.55, 0.80, 1) },
  { 'FrameBg', rgbm(0.16, 0.17, 0.21, 1) },
  { 'FrameBgHovered', rgbm(0.22, 0.24, 0.29, 1) },
  { 'FrameBgActive', rgbm(0.20, 0.42, 0.60, 1) },
  { 'Separator', rgbm(1.00, 1.00, 1.00, 0.10) },
}

-- The palette is settings, not constants: a colour that works on a grey wall
-- at Nordschleife is not the one that works against Bahrain at noon.
local PALETTE = {
  { 'text', 'colorText' },
  { 'label', 'colorLabel' },
  { 'dim', 'colorDim' },
  { 'good', 'colorGood' },
  { 'warn', 'colorWarn' },
  { 'bad', 'colorBad' },
}

--- Copy the chosen colours into the preallocated table the draw path uses.
--- Values, not references: `rgbm()` allocates, and the draw path must not.
-- No colour may be picked into invisibility. A settings screen that can hide
-- itself is a trap: the way back out is a button nobody can see.
local MIN_ALPHA = 0.35

--- Read "r,g,b" into an existing colour. No allocation, no new table, and a
--- malformed value leaves the colour alone rather than blanking it.
local function readColorInto(text, target)
  if type(text) ~= 'string' or target == nil then return end
  local r, g, b = text:match('^%s*([%d%.]+)%s*,%s*([%d%.]+)%s*,%s*([%d%.]+)%s*$')
  if r == nil then return end
  target.r, target.g, target.b = tonumber(r), tonumber(g), tonumber(b)
  target.mult = 1
end

local appliedPalette = {}
local paletteSelection = 1

local function applyPalette()
  for _, entry in ipairs(PALETTE) do
    local stored = settings[entry[2]]
    if appliedPalette[entry[1]] ~= stored then
      readColorInto(stored, COLOR[entry[1]])
      appliedPalette[entry[1]] = stored
    end
  end
  local accent = accentColor()
  COLOR.accent.r, COLOR.accent.g = accent.r, accent.g
  COLOR.accent.b, COLOR.accent.mult = accent.b, accent.mult
end

--- Apply the panel's spacing and colours, and take the window's measure.
--- Returns what to pop.
local function pushLayoutStyle()
  measureWindowScale()
  measureWindowWidth()
  -- Guarded: a palette is a setting, and no setting is worth an empty panel.
  pcall(applyPalette)
  ui.pushStyleVar(ui.StyleVar.ItemSpacing,
    settings.vrMode and ITEM_SPACING_VR or ITEM_SPACING)
  ui.pushStyleVar(ui.StyleVar.FramePadding, FRAME_PADDING)
  ui.pushStyleVar(ui.StyleVar.FrameRounding, 3)
  ui.pushStyleVar(ui.StyleVar.GrabRounding, 3)
  ui.pushStyleVar(ui.StyleVar.TabRounding, 3)
  ui.pushStyleVar(ui.StyleVar.ScrollbarRounding, 3)
  ui.pushStyleVar(ui.StyleVar.ItemInnerSpacing, vec2(6, 3))

  local colors = 0
  for _, entry in ipairs(STYLE_COLORS) do
    local slot = ui.StyleColor[entry[1]]
    if slot ~= nil then
      ui.pushStyleColor(slot, entry[2])
      colors = colors + 1
    end
  end
  return 7, colors
end

local function popLayoutStyle(vars, colors)
  if colors ~= nil and colors > 0 then ui.popStyleColor(colors) end
  ui.popStyleVar(vars)
end

local MAX_CONTENT = 360
local MAX_CONTENT_VR = 520

local function contentWidth()
  -- With auto scale the content fills the window: the size grew with it, so
  -- capping the width would only put the extra room back as emptiness. Taken
  -- once for the same reason the scale is.
  if settings.autoScale then return frameWidth end
  local limit = settings.contentWidth or MAX_CONTENT
  if settings.vrMode then limit = math.max(limit, MAX_CONTENT_VR) end
  return math.min(ui.availableSpaceX(), limit)
end

--- Where column `index` of a row of stats begins.
---
--- Three across normally. Two in VR, where the text is large enough that a
--- third column runs into its neighbour — the wrap happens by itself, since a
--- stat leaves the cursor at the start of the next line.
local function nextColumn(width, index)
  local perRow = settings.columnsPerRow or 0
  if perRow < 2 then perRow = settings.vrMode and 2 or 3 end
  local column = index % perRow
  if column > 0 then ui.sameLine(width / perRow * column) end
end

--- The RPM bar. Drawn by hand rather than with progressBar so the redline
--- segment can be shaded separately.
local function rpmBar(width)
  local ratio = rpmRatio()
  -- Thin lines disappear at VR resolutions; this is the one element that is
  -- read peripherally, so it is the one that has to survive that.
  local base = settings.vrMode and math.max(settings.barHeight, 12) or settings.barHeight
  local height = math.max(2, math.floor(base * windowScale() + 0.5))
  local origin = ui.getCursor()
  local to = vec2(origin.x + width, origin.y + height)

  ui.drawRectFilled(origin, to, COLOR.barBack, 2)
  if ratio > 0 then
    local filled = vec2(origin.x + width * ratio, origin.y + height)
    ui.drawRectFilled(origin, filled, rpmColor(ratio), 2)
  end

  -- The shift point, as a line on the bar and a full-width flash past it.
  -- Peripheral vision reads a change in the whole bar long before it reads a
  -- number, which is the entire reason this bar exists.
  if settings.shiftLight then
    local mark = origin.x + width * settings.shiftAt
    ui.drawRectFilled(vec2(mark - 1, origin.y), vec2(mark + 1, origin.y + height),
      COLOR.dim, 0)
    if ratio >= settings.shiftAt then
      ui.drawRectFilled(origin, to, COLOR.bad, 2)
    end
  end
  ui.dummy(vec2(width, height))
end

local function drawHeader()
  ui.beginGroup()
  say('hero', text.speed, COLOR.text)
  ui.endGroup()

  ui.sameLine()

  ui.beginGroup()
  say('caption', settings.mph and 'MPH' or 'KM/H', COLOR.label)
  say('gear', text.gear, rpmColor(rpmRatio()))
  ui.endGroup()

  if settings.showLimiter and hasFlag(FLAG_PIT_LIMITER) then
    ui.sameLine()
    ui.pushFont(ui.Font.Small)
    ui.textColored('LIMITER', COLOR.limiter)
    ui.popFont()
  end

  if settings.showRpmBar then
    rpmBar(contentWidth())
  end
end

local function drawTyres()
  sectionLabel('TYRES & BRAKES')

  -- Two by two, the way they sit on the car
  local columnWidth = contentWidth() * 0.5
  for row = 0, 1 do
    for col = 0, 1 do
      local i = row * 2 + col + 1
      if col == 1 then ui.sameLine(columnWidth) end

      ui.beginGroup()

      say('caption', TYRE_LABEL[i], COLOR.dim)
      ui.sameLine()
      say('body', text.pressure[i], COLOR.text)

      if settings.showTyreTemp then
        say('caption', text.tyreTemp[i], tyreTempColor(shown.tyre_temp_c[i]))
      end
      if settings.showBrakeTemp then
        if settings.showTyreTemp then ui.sameLine() end
        say('caption', text.brakeTemp[i], brakeColor(shown.brake_temp_c[i]))
      end

      if settings.showWear then
        say('caption', text.wear[i], wearColor(shown.tyre_wear_percent[i]))
      end

      ui.endGroup()
    end
  end
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
  local width = contentWidth()

  local column = 0
  if settings.showDelta then
    nextColumn(width, column)
    stat('DELTA', text.delta, deltaColor)
    column = column + 1
  end
  if settings.showBest then
    nextColumn(width, column)
    stat('BEST', text.best, COLOR.purple)
    column = column + 1
  end
  if settings.showLast then
    nextColumn(width, column)
    stat('LAST', text.last, COLOR.text)
  end
end

local function drawFuel()
  local color = hasFlag(FLAG_FUEL_WARNING) and COLOR.bad or COLOR.text

  local width = contentWidth()

  local column = 0
  if settings.showFuelLitres then
    nextColumn(width, column)
    stat('FUEL', text.fuel, color)
    column = column + 1
  end
  if settings.showLapsLeft then
    nextColumn(width, column)
    stat('LAPS LEFT', text.lapsLeft, color)
    column = column + 1
  end
  if not settings.showPerLap then return end
  nextColumn(width, column)
  stat('PER LAP', text.perLap, COLOR.dim)
end

--- Where the session is: position, lap, the lap running now, and the
--- conditions that explain why the tyres are behaving as they are.
local function drawSession()
  sectionLabel('SESSION')

  local width = contentWidth()
  local column = 0
  if settings.showPosition then
    nextColumn(width, column)
    stat('POS', text.position, COLOR.text)
    column = column + 1
  end
  if settings.showLapNumber then
    nextColumn(width, column)
    stat('LAP', text.lap, COLOR.text)
    column = column + 1
  end
  if settings.showCurrentLap then
    nextColumn(width, column)
    stat('CURRENT', text.current, accentColor())
  end

  if settings.showConditions then
    say('caption', text.conditions, COLOR.dim)
  end
end

--- The engineer's lines, drawn the way the settings ask for.
---
--- `withLabel` is false in the advice window, where the window's own title
--- already says what this is.
--- Advice, at its own size: it is the one thing read while the car is moving,
--- and the one thing worth making bigger than everything else.
local function sayAdvice(text, color)
  -- A limit on the line, not just on how many of them: one long sentence can
  -- push the rest of the panel off the bottom of a small window, and the first
  -- forty characters are the ones that carry the advice.
  local limit = settings.engineerMaxChars or 64
  if #text > limit then text = text:sub(1, limit - 1) .. '…' end
  ui.dwriteText(text, textSize('body') * (settings.engineerScale or 1), color)
end

local function drawEngineerMessages(withLabel)
  if withLabel ~= false then sectionLabel('ENGINEER') end

  -- A plate behind the advice, on its own. Numbers are read in a glance and
  -- forgiven a busy background; a sentence is not.
  if settings.engineerBackground > 0.01 then
    local origin = ui.getCursor()
    local space = ui.availableSpace()
    ui.drawRectFilled(vec2(origin.x - 4, origin.y - 3),
      vec2(origin.x + contentWidth(), origin.y + math.min(space.y, 140)),
      rgbm(0.04, 0.05, 0.07, settings.engineerBackground), 4)
  end

  if settings.devSampleAdvice then
    shown.message_count = 4
    for i = 1, 4 do
      shown.messages[i] = DEMO_ADVICE[i]
      shown.message_severity[i] = (i - 1) % 3
    end
  end

  local bySeverity = settings.engineerBullet == 'severity'
  local bullet = BULLETS[settings.engineerBullet] or ''
  local count = math.min(shown.message_count, settings.engineerLines, 4)

  for i = 1, count do
    local level = shown.message_severity[i] or 0
    if shown.messages[i] ~= '' and level >= (settings.engineerMinSeverity or 0) then
      local mark = bySeverity and (SEVERITY_MARK[level] or '') or bullet
      if settings.engineerSeverityWord then mark = SEVERITY_WORD[level] or mark end
      -- The application colours the marker and leaves the sentence readable;
      -- doing the same here is the whole point of shipping severity across.
      local markColor = SEVERITY_COLOR[level] or COLOR.text
      local textColor = settings.engineerHighlight and markColor or COLOR.text
      local line = shown.messages[i]
      if settings.engineerUppercase then line = line:upper() end
      if settings.engineerNumbered then line = i .. '. ' .. line end

      if bySeverity then
        sayAdvice(mark, markColor)
        ui.sameLine()
      end

      if settings.engineerWrap then
        -- textWrapped takes the colour from the style stack, not an argument.
        ui.pushStyleColor(ui.StyleColor.Text, bySeverity and COLOR.text or textColor)
        ui.textWrapped(bySeverity and line or (mark .. line))
        ui.popStyleColor()
      else
        sayAdvice(bySeverity and line or (mark .. line), bySeverity and COLOR.text or textColor)
      end
      if settings.engineerSeparator then ui.separator() end
      if settings.engineerSpacing then gap(settings.engineerLineGap or 4) end
    end
  end

  if count == 0 and withLabel ~= false then
    say('caption', 'nothing to report', COLOR.dim)
  elseif settings.engineerShowCount then
    say('caption', string.format('%d of %d shown', count, shown.message_count), COLOR.dim)
  end
end

-- Numbers that look like a car on a warm lap. Only reachable from developer
-- mode, and the panel says so, so nobody mistakes them for telemetry.
local DEMO_ADVICE = {
  'Fuel is fine for the stint',
  'Rear tyres are going off, ease the traction',
  'Box this lap',
  'Front-left pressure is 0.4 psi low and the corner is running cold in sector two',
}

local function applyDemo()
  shown.version = EXPECTED_VERSION
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
  shown.message_count = 4
  for i = 1, 4 do
    shown.messages[i] = DEMO_ADVICE[i]
    shown.message_severity[i] = (i - 1) % 3
  end
end

--- What every window shows while the desktop application is not publishing.
---
--- Nothing else is drawn in that state on purpose: numbers from a dead feed
--- are worse than no numbers, and a panel that looks broken sends people
--- looking for the wrong problem. This says which problem it is.
local function drawWaitingForApp()
  if openError ~= nil then
    pushRole('body')
    ui.pushStyleColor(ui.StyleColor.Text, COLOR.bad)
    ui.textWrapped('Waiting for AC Pro Engineer')
    ui.popStyleColor()
    ui.popFont()

    pushRole('caption')
    ui.pushStyleColor(ui.StyleColor.Text, COLOR.dim)
    ui.textWrapped('The shared mapping is not there yet. Start the desktop '
      .. 'application — it creates the mapping, and this panel picks it up '
      .. 'within a couple of seconds.')
    ui.popStyleColor()
    ui.popFont()
    return
  end

  -- Wrapped, not clipped: this text is wider than a narrow panel and the half
  -- of the sentence that fits is not the useful half.
  pushRole('body')
  ui.pushStyleColor(ui.StyleColor.Text, COLOR.warn)
  ui.textWrapped('AC Pro Engineer is not running')
  ui.popStyleColor()
  ui.popFont()

  pushRole('caption')
  ui.pushStyleColor(ui.StyleColor.Text, COLOR.dim)
  ui.textWrapped('Start the desktop application to see telemetry.')
  if shown.sequence == 0 then
    ui.textWrapped('Nothing has been published yet.')
  else
    ui.textWrapped(string.format('Last frame %.0f s ago.', secondsSinceChange))
  end
  ui.popStyleColor()
  ui.popFont()
end

-- ---------------------------------------------------------------------------
-- The panel
--
-- CSP owns the window: the manifest declares it, CSP draws the frame, the
-- title bar and the background, and the driver moves and resizes it. So this
-- draws contents and nothing else — `ui.begin` does not exist in the app SDK
-- (`cargo test -p ac_core the_overlay_app_only_calls` checks that against the
-- installed CSP), and pushing WindowBg or WindowRounding from in here would
-- style a window that was never opened.
-- ---------------------------------------------------------------------------

function script.windowMain(dt)
  if not isLive then
    drawWaitingForApp()
    return
  end

  if shown.version ~= EXPECTED_VERSION and not settings.devIgnoreVersion then
    ui.textColored('Version mismatch', COLOR.bad)
    pushRole('caption')
    ui.textColored(string.format('app v%d, overlay v%d', shown.version, EXPECTED_VERSION),
      COLOR.dim)
    ui.popFont()
    return
  end

  local styles, colors = pushLayoutStyle()

  -- The panel's own backing. CSP's window background is whatever the driver
  -- set for every app; a readout over a bright sky needs its own.
  if settings.background > 0.01 then
    local origin = ui.getCursor()
    local space = ui.availableSpace()
    ui.drawRectFilled(vec2(origin.x - 6, origin.y - 4),
      vec2(origin.x + contentWidth() + 6, origin.y + space.y),
      rgbm(0.05, 0.06, 0.08, settings.background), 4)
  end

  if settings.showDebugBounds then
    local origin = ui.getCursor()
    local space = ui.availableSpace()
    ui.drawRect(vec2(origin.x, origin.y),
      vec2(origin.x + contentWidth(), origin.y + space.y), accentColor(), 2, 1)
  end

  -- One line, for a driver who wants the panel out of the way: speed, gear,
  -- delta and fuel, and nothing else on screen.
  if settings.hudMode then
    say('hero', string.format('%.0f', shown.speed_kmh), COLOR.text)
    ui.sameLine()
    say('gear', gearText(shown.gear), rpmColor(rpmRatio()))
    ui.sameLine()
    local deltaColor = COLOR.text
    if shown.delta_seconds < -0.001 then
      deltaColor = COLOR.good
    elseif shown.delta_seconds > 0.001 then
      deltaColor = COLOR.bad
    end
    say('gear', string.format('%+.2f', shown.delta_seconds), deltaColor)
    ui.sameLine()
    say('gear', string.format('%.0f L', shown.fuel_litres),
      hasFlag(FLAG_FUEL_WARNING) and COLOR.bad or COLOR.dim)
    if settings.showRpmBar then rpmBar(contentWidth()) end
    popLayoutStyle(styles, colors)
    return
  end

  if settings.showHeader then
    drawHeader()
    gap(6)
  end

  -- Two gates on each section, and they mean different things: the flag is the
  -- application saying it has nothing to show, the setting is the driver
  -- saying they do not want to see it. Everything here can be switched off —
  -- a panel in the corner of a windscreen earns its space or loses it.
  if hasFlag(FLAG_SHOW_TELEMETRY) then
    if settings.showTyres then
      drawTyres()
      gap(6)
    end
  end

  if hasFlag(FLAG_SHOW_TIMING) and settings.showTiming then
    drawTiming()
    gap(6)
  end

  if hasFlag(FLAG_SHOW_FUEL) and settings.showFuel then
    drawFuel()
    gap(6)
  end

  if hasFlag(FLAG_SHOW_SESSION) and settings.showSession then
    drawSession()
  end

  -- The advice also has a window of its own; this block is for keeping it in
  -- the corner of the eye without a second window on screen.
  if hasFlag(FLAG_SHOW_ENGINEER) and settings.showEngineer and shown.message_count > 0 then
    gap(8)
    drawEngineerMessages()
  end

  popLayoutStyle(styles, colors)
end

-- ---------------------------------------------------------------------------
-- Engineer window
--
-- Its own entry in CSP's sidebar and its own window, because advice is read
-- while the telemetry is being watched, not instead of it — and because a
-- window that holds four lines of text wants to sit somewhere else on screen
-- than one holding four corners of tyre data.
-- ---------------------------------------------------------------------------

function script.windowEngineer(dt)
  if not isLive then
    drawWaitingForApp()
    return
  end

  if not hasFlag(FLAG_SHOW_ENGINEER) then
    pushRole('caption')
    ui.textColored('Engineer advice is switched off in the desktop app', COLOR.dim)
    ui.popFont()
    return
  end

  local count = math.min(shown.message_count, settings.engineerLines, 4)
  if count == 0 then
    pushRole('caption')
    ui.textColored('Nothing to report', COLOR.dim)
    ui.popFont()
    return
  end

  local styles, colors = pushLayoutStyle()
  drawEngineerMessages(false)
  popLayoutStyle(styles, colors)
end

-- ---------------------------------------------------------------------------
-- Telemetry window
--
-- Everything in the frame, as it arrived. The panel decides what is worth a
-- driver's attention mid-corner; this is for the other question — whether a
-- number is reaching the game at all — and it answers it without alt-tabbing.
-- ---------------------------------------------------------------------------

local FLAG_NAMES = {
  { 'pit limiter', 1 },
  { 'connected', 2 },
  { 'telemetry', 4 },
  { 'engineer', 8 },
  { 'fuel warning', 16 },
  { 'session', 32 },
  { 'lap timing', 64 },
  { 'fuel', 128 },
}

--- A label on the left, a value on the right, filling the width.
local function row(label, value, color)
  local width = contentWidth()
  say('caption', label, COLOR.dim)
  ui.sameLine(width * 0.46)
  say('body', value, color or COLOR.text)
end

-- A console, with the same vocabulary as the harness's: settings that would
-- otherwise need a mouse and four tabs can be typed, and the ones with no
-- widget at all — like --dev-mode — have somewhere to live.
local consoleInput = ''
local consoleLines = { 'type --help' }

local function consoleSay(line)
  consoleLines[#consoleLines + 1] = line
  while #consoleLines > 12 do table.remove(consoleLines, 1) end
end

local COMMANDS = {
  ['--help'] = function()
    consoleSay('--scale N   --width N   --bar N   --backing N')
    consoleSay('--accent blue|teal|amber|violet|green')
    consoleSay('--vr on|off   --dev-mode   --units c|f  --psi|--bar-units')
    consoleSay('--lines N   --palette   --reset')
  end,
  ['--dev-mode'] = function()
    settings.devMode = not settings.devMode
    consoleSay('developer mode: ' .. (settings.devMode and 'on' or 'off'))
  end,
  ['--reset'] = function()
    for key, value in pairs(DEFAULTS) do settings[key] = value end
    consoleSay('settings reset')
  end,
}

local NUMERIC = {
  ['--scale'] = { 'fontScale', 0.5, 4 },
  ['--width'] = { 'contentWidth', 200, 1200 },
  ['--bar'] = { 'barHeight', 2, 40 },
  ['--backing'] = { 'background', 0, 1 },
  ['--lines'] = { 'engineerLines', 1, 4 },
}

local function runCommand(line)
  local words = {}
  for word in tostring(line):gmatch('%S+') do words[#words + 1] = word end
  if #words == 0 then return end
  consoleSay('> ' .. line)

  local index = 1
  while index <= #words do
    local word = words[index]
    local numeric = NUMERIC[word]
    if COMMANDS[word] ~= nil then
      COMMANDS[word]()
    elseif numeric ~= nil then
      local value = tonumber(words[index + 1])
      if value ~= nil then
        settings[numeric[1]] = math.max(numeric[2], math.min(numeric[3], value))
        consoleSay(numeric[1] .. ' = ' .. tostring(settings[numeric[1]]))
        index = index + 1
      else
        consoleSay(word .. ' needs a number')
      end
    elseif word == '--accent' then
      local name = words[index + 1]
      if ACCENTS[name] ~= nil then
        settings.accent = name
        index = index + 1
      else
        consoleSay('accents: blue teal amber violet green')
      end
    elseif word == '--vr' then
      settings.vrMode = words[index + 1] ~= 'off'
      index = index + 1
    elseif word == '--units' then
      settings.celsius = words[index + 1] ~= 'f'
      index = index + 1
    elseif word == '--palette' then
      for _, entry in ipairs(PALETTE) do settings[entry[2]] = DEFAULTS[entry[2]] end
      consoleSay('palette reset')
    elseif word == '--psi' then
      settings.psi = true
    elseif word == '--bar-units' then
      settings.psi = false
    else
      consoleSay('unknown: ' .. word)
    end
    index = index + 1
  end
end

--- A checkbox bound to a settings field. `ui.checkbox` reports the click, not
--- the new value, so the flip happens here.
---
--- Declared up here because every tab uses it, and a local declared after its
--- callers is a global to them — which is how the developer tab came out as a
--- heading with nothing underneath it.
local function settingToggle(label, key)
  -- The box carries no label of its own: CSP draws widget text at its own font
  -- size, which cannot be scaled, and on a 4K screen that is a settings window
  -- nobody can read. The label is drawn beside it at the panel's size instead.
  if ui.checkbox('##' .. key, settings[key]) then
    settings[key] = not settings[key]
    saveSettings()
  end
  ui.sameLine()
  say('body', label, COLOR.text)
end

--- A radio button with the same treatment.
--- A slider bound to a settings field, labelled at the panel's size and saved
--- when it moves.
local function settingSlider(id, key, low, high, format, integer)
  ui.setNextItemWidth(contentWidth())
  local value, changed = ui.slider('##' .. id, settings[key], low, high, format, integer)
  if changed then
    settings[key] = value
    saveSettings()
  end
  return changed
end

--- A radio button with the same treatment.
local function settingRadio(label, id, selected)
  local clicked = ui.radioButton('##' .. id, selected)
  ui.sameLine()
  say('body', label, selected and COLOR.text or COLOR.dim)
  return clicked
end

-- One-press commands, so the common ones do not have to be typed at all. The
-- console is for the ones with no widget; these are the ones worth reaching in
-- a pit lane.
local QUICK = {
  { '4K', '--scale 2 --width 680 --bar 12' },
  { '1080p', '--scale 1 --width 360 --bar 6' },
  { 'VR', '--vr on --scale 1.6' },
  { 'Bigger', '--scale 1.2' },
  { 'Smaller', '--scale 0.85' },
  { 'Dev', '--dev-mode' },
  { 'Reset', '--reset' },
}

local lastCommand = ''

local function drawConsoleBody()
  say('caption', 'QUICK', COLOR.label)
  for index, entry in ipairs(QUICK) do
    if ui.button(entry[1]) then runCommand(entry[2]) end
    if index % 4 ~= 0 and index < #QUICK then ui.sameLine() end
  end

  ui.separator()
  say('caption', 'COMMAND', COLOR.label)
  ui.setNextItemWidth(contentWidth())
  local typed, _, entered = ui.inputText('##acpeConsole', consoleInput)
  consoleInput = typed or consoleInput
  if entered then
    lastCommand = consoleInput
    runCommand(consoleInput)
    consoleInput = ''
  end

  if ui.button('Run') then
    lastCommand = consoleInput
    runCommand(consoleInput)
    consoleInput = ''
  end
  ui.sameLine()
  if ui.button('Again') and lastCommand ~= '' then runCommand(lastCommand) end
  ui.sameLine()
  if ui.button('Help') then runCommand('--help') end
  ui.sameLine()
  if ui.button('Clear') then consoleLines = {} end

  ui.separator()
  for _, line in ipairs(consoleLines) do
    local color = COLOR.dim
    if line:sub(1, 1) == '>' then
      color = COLOR.accent
    elseif line:match('^unknown') or line:match('needs a number') then
      color = COLOR.bad
    end
    say('caption', line, color)
  end
end

--- What is only worth seeing while working on the panel: the numbers behind
--- the numbers, and the switches that make the panel lie on purpose.
local function drawDevBody()
  say('caption', 'DRAW WITHOUT A SESSION', COLOR.label)
  settingToggle('Demo numbers', 'devDemo')
  settingToggle('Sample advice, all severities', 'devSampleAdvice')
  settingToggle('Ignore what the app asked for', 'devIgnoreFlags')
  settingToggle('Ignore version mismatch', 'devIgnoreVersion')

  ui.separator()
  say('caption', 'INSPECT', COLOR.label)
  settingToggle('Freeze the display', 'freezeDisplay')
  settingToggle('Outline the content', 'showDebugBounds')

  ui.separator()
  if ui.button('Everything on') then
    for _, key in ipairs(SETTING_KEYS) do
      if key:match('^show') and type(settings[key]) == 'boolean' then
        settings[key] = true
      end
    end
    settings.devIgnoreFlags = true
  end
  ui.sameLine()
  if ui.button('Leave developer mode') then
    settings.devMode = false
    settings.devDemo = false
    settings.devSampleAdvice = false
    settings.devIgnoreFlags = false
    settings.freezeDisplay = false
    settings.showDebugBounds = false
  end

  ui.separator()
  say('caption', 'FRAME', COLOR.label)
  row('sequence', tostring(shown.sequence))
  row('since change', string.format('%.2f s', secondsSinceChange))
  row('flags', string.format('0x%02X', shown.flags))
  row('version', string.format('%d / %d', shown.version, EXPECTED_VERSION))

  say('caption', 'LAYOUT', COLOR.label)
  row('window scale', string.format('%.2fx', windowScale()))
  row('content width', string.format('%.0f px', contentWidth()))
  row('body text', string.format('%.1f px', textSize('body')))
  row('advice text', string.format('%.1f px',
    textSize('body') * (settings.engineerScale or 1)))

  ui.separator()
  if ui.button('Dump settings to console') then
    for _, key in ipairs(SETTING_KEYS) do
      consoleSay(key .. ' = ' .. tostring(settings[key]))
    end
  end
end

local function drawTelemetryBody()
  if not isLive then
    drawWaitingForApp()
    return
  end

  sectionLabel('CAR')
  row('speed', string.format('%.1f km/h', shown.speed_kmh))
  row('rpm', string.format('%d / %d', shown.rpm, shown.max_rpm))
  row('gear', gearText(shown.gear))

  sectionLabel('FUEL')
  row('in tank', string.format('%.2f L', shown.fuel_litres))
  row('per lap', string.format('%.2f L', shown.fuel_per_lap))
  row('laps left', string.format('%.1f', shown.fuel_laps_remaining))

  sectionLabel('TIMING')
  row('delta', string.format('%+.3f s', shown.delta_seconds))
  row('best', lapTimeText(shown.best_lap_ms))
  row('last', lapTimeText(shown.last_lap_ms))
  row('current', lapTimeText(shown.current_lap_ms))

  sectionLabel('SESSION')
  row('position', string.format('P%d', shown.position))
  row('lap', tostring(shown.lap_count))
  row('air', tempText(shown.air_temp_c))
  row('road', tempText(shown.road_temp_c))
  row('grip', string.format('%.0f%%', shown.surface_grip * 100))

  sectionLabel('CORNERS')
  for i = 1, 4 do
    row(TYRE_LABEL[i], string.format('%s  %s  %s  %.0f%%',
      pressureText(shown.tyre_pressure_psi[i]),
      tempText(shown.tyre_temp_c[i]),
      tempText(shown.brake_temp_c[i]),
      shown.tyre_wear_percent[i]))
  end

  sectionLabel('FLAGS')
  for _, entry in ipairs(FLAG_NAMES) do
    local on = hasFlag(entry[2])
    row(entry[1], on and 'on' or 'off', on and COLOR.good or COLOR.dim)
  end
end

function script.windowTelemetry(dt)
  local styles, colors = pushLayoutStyle()
  drawTelemetryBody()
  popLayoutStyle(styles, colors)
end

-- ---------------------------------------------------------------------------
-- Status window
--
-- The questions asked when the panel is empty: is the mapping open, is anything
-- arriving, and do the two sides agree about the shape of what arrives.
-- ---------------------------------------------------------------------------

local function drawStatusBody()
  sectionLabel('LINK')
  row('mapping', MMF_NAME, frame ~= nil and COLOR.good or COLOR.bad)
  row('state', frame == nil and 'not opened' or (isLive and 'live' or 'stale'),
    isLive and COLOR.good or COLOR.warn)
  if openError ~= nil then
    pushRole('caption')
    ui.pushStyleColor(ui.StyleColor.Text, COLOR.dim)
    ui.textWrapped(openError)
    ui.popStyleColor()
    ui.popFont()
  end

  sectionLabel('FRAME')
  row('sequence', tostring(shown.sequence))
  row('since change', string.format('%.1f s', secondsSinceChange))
  row('version', string.format('%d, expected %d', shown.version, EXPECTED_VERSION),
    shown.version == EXPECTED_VERSION and COLOR.good or COLOR.bad)
  row('advice lines', tostring(shown.message_count))

  sectionLabel('PANEL')
  row('text size', settings.vrMode and 'VR (large)' or settings.textSize)
  row('units', (settings.celsius and 'C' or 'F') .. ' / ' .. (settings.psi and 'psi' or 'bar'))
end

function script.windowStatus(dt)
  local styles, colors = pushLayoutStyle()
  drawStatusBody()
  popLayoutStyle(styles, colors)
end

-- ---------------------------------------------------------------------------
-- Settings window
--
-- CSP opens this from the app's own entry in the sidebar, and it is a separate
-- window from the panel: none of this runs while the overlay is being drawn.
-- ---------------------------------------------------------------------------

function script.windowSettings(dt)
  -- The settings stay usable with the application closed — they are this
  -- machine's preferences, not the feed's — but the panel will not come back
  -- until it publishes, and saying so here saves a hunt through the checkboxes.
  if not isLive then
    pushRole('caption')
    ui.textColored('Panel hidden: AC Pro Engineer is not running', COLOR.warn)
    ui.popFont()
    ui.separator()
  end

  -- Four tabs rather than one long column: the window is as tall as the driver
  -- left it, and a list that runs past the bottom edge hides the half of the
  -- settings nobody scrolled to.
  local styles, colors = pushLayoutStyle()
  ui.tabBar('acpeSettings', function()
    ui.tabItem('Panel', function()
      ui.tabBar('acpePanel', function()
        ui.tabItem('Blocks', function()
          settingToggle('Speed and gear', 'showHeader')
          settingToggle('RPM bar', 'showRpmBar')
          settingToggle('Tyres and brakes', 'showTyres')
          settingToggle('Lap timing', 'showTiming')
          settingToggle('Fuel', 'showFuel')
          settingToggle('Session', 'showSession')
          settingToggle('Engineer advice', 'showEngineer')
          settingToggle('Section captions', 'sectionLabels')
          settingToggle('LIMITER badge', 'showLimiter')

          ui.separator()
          settingToggle('One-line mode', 'hudMode')
          say('caption', 'speed, gear, delta, fuel — nothing else', COLOR.dim)

          ui.separator()
          settingToggle('Shift light', 'shiftLight')
          local shift, shiftChanged = ui.slider('##shiftAt', settings.shiftAt * 100, 80, 100,
            'shift at  %.0f%% of the range')
          if shiftChanged then settings.shiftAt = shift / 100 end
        end)

        ui.tabItem('Corners', function()
          settingToggle('Tyre temperature', 'showTyreTemp')
          settingToggle('Brake temperature', 'showBrakeTemp')
          settingToggle('Wear', 'showWear')

          ui.separator()
          say('caption', 'PRESSURE', COLOR.label)
          for decimals = 0, 2 do
            if ui.radioButton(string.format('%d decimal%s', decimals,
                decimals == 1 and '' or 's'), settings.pressureDecimals == decimals) then
              settings.pressureDecimals = decimals
              formatFrame()
            end
          end
        end)

        ui.tabItem('Limits', function()
          say('caption', 'TYRE TEMPERATURE', COLOR.label)
          local function limit(label, key, low, high, format)
            local value, changed = ui.slider('##' .. key, settings[key], low, high, format, true)
            if changed then settings[key] = value end
          end
          limit('cold', 'tyreCold', 40, 100, 'cold below  %.0f')
          limit('hot', 'tyreHot', 60, 130, 'working to  %.0f')
          limit('over', 'tyreOver', 80, 160, 'overheating past  %.0f')

          ui.separator()
          say('caption', 'BRAKE TEMPERATURE', COLOR.label)
          limit('bcold', 'brakeCold', 50, 400, 'cold below  %.0f')
          limit('bhot', 'brakeHot', 200, 900, 'working to  %.0f')
          limit('bover', 'brakeOver', 400, 1200, 'overheating past  %.0f')

          ui.separator()
          say('caption', 'WEAR', COLOR.label)
          limit('wwarn', 'wearWarn', 80, 100, 'good above  %.0f%%')
          limit('wbad', 'wearBad', 50, 99, 'worn below  %.0f%%')

          ui.separator()
          say('caption', 'the colours are yours; a slick at 95 is in its window', COLOR.dim)
          say('caption', 'and a hard at 95 is stone cold', COLOR.dim)
        end)

        ui.tabItem('Fields', function()
          say('caption', 'TIMING', COLOR.label)
          settingToggle('Delta', 'showDelta')
          settingToggle('Best lap', 'showBest')
          settingToggle('Last lap', 'showLast')

          ui.separator()
          say('caption', 'FUEL', COLOR.label)
          settingToggle('In the tank', 'showFuelLitres')
          settingToggle('Laps left', 'showLapsLeft')
          settingToggle('Per lap', 'showPerLap')

          ui.separator()
          say('caption', 'SESSION', COLOR.label)
          settingToggle('Position', 'showPosition')
          settingToggle('Lap number', 'showLapNumber')
          settingToggle('Current lap', 'showCurrentLap')
          settingToggle('Track conditions', 'showConditions')

          ui.separator()
          say('caption', 'COLUMNS', COLOR.label)
          for _, option in ipairs({ { 'automatic', 0 }, { 'two', 2 }, { 'three', 3 } }) do
            if ui.radioButton(option[1], settings.columnsPerRow == option[2]) then
              settings.columnsPerRow = option[2]
            end
          end
        end)

        -- Sections the application itself is suppressing. Without this the
        -- settings read as broken: a box is ticked and nothing appears.
        ui.tabItem('State', function()
          -- What the application is sending, flag by flag. "Nothing wrong" is
          -- an answer too, and an empty tab does not give it.
          say('caption', 'THE APPLICATION IS SENDING', COLOR.label)
          for _, entry in ipairs(FLAG_NAMES) do
            local on = bit.band(shown.flags, entry[2]) ~= 0
            row(entry[1], on and 'on' or 'off', on and COLOR.good or COLOR.dim)
          end

          ui.separator()
          if not isLive then
            say('caption', 'the desktop application is not running', COLOR.warn)
          elseif settings.devIgnoreFlags then
            say('caption', 'developer mode is drawing every block anyway', COLOR.warn)
          else
            say('caption', 'a block needs its flag here and its switch in Blocks',
              COLOR.dim)
          end
        end)
      end)
    end)

    ui.tabItem('Advice', function()
      pushRole('caption')
      ui.textColored('LINES', COLOR.label)
      ui.popFont()
      for _, lines in ipairs({ 1, 2, 3, 4 }) do
        if settingRadio(string.format('%d line%s', lines, lines > 1 and 's' or ''),
            'lines' .. lines, settings.engineerLines == lines) then
          settings.engineerLines = lines
        end
      end

      ui.separator()
      pushRole('caption')
      ui.textColored('MARKER', COLOR.label)
      ui.popFont()
      for _, bullet in ipairs(BULLET_NAMES) do
        if settingRadio(bullet, 'bullet' .. bullet, settings.engineerBullet == bullet) then
          settings.engineerBullet = bullet
        end
      end

      ui.separator()
      settingSlider('adviceScale', 'engineerScale', 0.6, 2.5, 'advice scale  %.2fx')

      settingSlider('adviceChars', 'engineerMaxChars', 20, 64, 'line limit  %.0f chars', true)

      ui.separator()
      say('caption', 'SHOW', COLOR.label)
      for index, name in ipairs({ 'everything', 'warnings and worse', 'critical only' }) do
        if settingRadio(name, 'sev' .. index, settings.engineerMinSeverity == index - 1) then
          settings.engineerMinSeverity = index - 1
        end
      end

      ui.separator()
      settingToggle('Wrap long lines', 'engineerWrap')
      settingToggle('Highlight advice', 'engineerHighlight')
      settingToggle('Space between lines', 'engineerSpacing')
      settingSlider('adviceGap', 'engineerLineGap', 0, 20, 'gap  %.0f px', true)
      settingToggle('Rule between lines', 'engineerSeparator')
      settingToggle('Show how many are hidden', 'engineerShowCount')

      settingSlider('advicePlate', 'engineerBackground', 0, 1, 'plate behind the advice  %.2f')

      ui.separator()
      say('caption', 'FORMAT', COLOR.label)
      settingToggle('Upper case', 'engineerUppercase')
      settingToggle('Number the lines', 'engineerNumbered')
      settingToggle('Spell the severity', 'engineerSeverityWord')
    end)

    ui.tabItem('Look', function()
      ui.tabBar('acpeLook', function()
        ui.tabItem('Screen', function()
          -- Presets first: a panel that opens unreadable on a 4K screen is a
          -- panel nobody gets as far as configuring.
          for _, preset in ipairs(PRESETS) do
            if ui.button(preset[1]) then
              for key, value in pairs(preset[2]) do settings[key] = value end
            end
            if preset[3] then ui.sameLine() end
          end

          ui.separator()
          settingToggle('Grow with the window', 'autoScale')
          say('caption', settings.autoScale
            and 'size follows the window; width is the window'
            or 'fixed size and width', COLOR.dim)
        end)

        ui.tabItem('Size', function()
      -- Sliders, because these are the two numbers worth nudging while looking
      -- at the panel rather than picking from a list.
      settingSlider('scale', 'fontScale', 0.6, 3.0, 'text scale  %.2fx')

      settingSlider('width', 'contentWidth', 240, 900, 'content width  %.0f px', true)

      settingSlider('bar', 'barHeight', 3, 24, 'rev bar  %.0f px', true)

          ui.separator()
          for _, size in ipairs(TEXT_SIZES) do
            if settingRadio(size, 'text' .. size, settings.textSize == size) then
              settings.textSize = size
            end
          end

          ui.separator()
          settingToggle('VR mode', 'vrMode')
          say('caption', 'largest text, thicker bar, more spacing', COLOR.dim)
        end)

        ui.tabItem('Colour', function()
          say('caption', 'ACCENT', COLOR.label)
          for _, name in ipairs(ACCENT_NAMES) do
            if settingRadio(name, 'accent' .. name, settings.accent == name) then
              settings.accent = name
            end
          end

          ui.separator()
          say('caption', 'PALETTE', COLOR.label)
          -- A colour button is a swatch, not a picker: clicking one does not
          -- open anything, which is why nothing happened. So the swatches
          -- choose, and one picker edits what was chosen.
          for index, entry in ipairs(PALETTE) do
            ui.colorButton(entry[1] .. '##swatch', COLOR[entry[1]])
            ui.sameLine()
            if ui.button(entry[1] .. (paletteSelection == index and '  •' or '')) then
              paletteSelection = index
            end
            if index % 2 == 0 then ui.newLine() else ui.sameLine() end
          end
          ui.newLine()

          ui.separator()
          local chosen = PALETTE[paletteSelection]
          if chosen ~= nil then
            local current = COLOR[chosen[1]]
            say('caption', 'editing ' .. chosen[1], COLOR.label)
            if ui.colorPicker('##palettePicker', current) then
              settings[chosen[2]] =
                string.format('%.3f,%.3f,%.3f', current.r, current.g, current.b)
            end
          end

          ui.separator()
          if ui.button('Default palette') then
            for _, entry in ipairs(PALETTE) do settings[entry[2]] = DEFAULTS[entry[2]] end
          end

          ui.separator()
          local backing2, backingChanged2 = ui.slider('##backing2', settings.background, 0, 1,
            'backing  %.2f')
          if backingChanged2 then settings.background = backing2 end
          say('caption', 'the panel\'s own plate, for a bright sky', COLOR.dim)
        end)
      end)
    end)

    ui.tabItem('Units', function()
      if ui.checkbox('Celsius', settings.celsius) then
        settings.celsius = not settings.celsius
        formatFrame()
      end
      if ui.checkbox('PSI', settings.psi) then
        settings.psi = not settings.psi
        formatFrame()
      end
      if ui.checkbox('Miles per hour', settings.mph) then
        settings.mph = not settings.mph
        formatFrame()
      end
      if ui.checkbox('Gallons', settings.gallons) then
        settings.gallons = not settings.gallons
        formatFrame()
      end

      ui.separator()
      say('caption', 'FORMAT', COLOR.label)
      if ui.checkbox('Short lap times', settings.shortLapTimes) then
        settings.shortLapTimes = not settings.shortLapTimes
        formatFrame()
      end
      if ui.checkbox('Unit suffixes', settings.unitSuffix) then
        settings.unitSuffix = not settings.unitSuffix
        formatFrame()
      end

      pushRole('caption')
      ui.textColored(settings.celsius and 'temperatures in °C' or 'temperatures in °F', COLOR.dim)
      ui.textColored(settings.psi and 'pressures in psi' or 'pressures in bar', COLOR.dim)
      ui.popFont()

      ui.separator()
      say('caption', storageActive and 'settings are saved as you change them'
        or 'storage unavailable: settings last for this session', 
        storageActive and COLOR.dim or COLOR.warn)
      if ui.button('Save now') then saveSettings() end
      ui.sameLine()
      if ui.button('Reset to defaults') then
        for key, value in pairs(DEFAULTS) do settings[key] = value end
        saveSettings()
        formatFrame()
      end
    end)

    -- The same two panels that have windows of their own, here as tabs: one
    -- window to open when the question is "what is going on", rather than
    -- three entries to hunt for in the sidebar.
    ui.tabItem('Console', drawConsoleBody)

    -- Red, and only here when asked for. The raw frame and the link state live
    -- under it: they answer questions a driver does not have, and four tabs
    -- nobody needs are four tabs in the way of the two they do.
    if settings.devMode then
      ui.pushStyleColor(ui.StyleColor.Tab, rgbm(0.45, 0.12, 0.14, 1))
      ui.pushStyleColor(ui.StyleColor.TabHovered, rgbm(0.75, 0.20, 0.22, 1))
      ui.pushStyleColor(ui.StyleColor.TabActive, rgbm(0.62, 0.16, 0.18, 1))
      ui.tabItem('Dev', function()
        ui.tabBar('acpeDev', function()
          ui.tabItem('Switches', drawDevBody)
          ui.tabItem('Data', drawTelemetryBody)
          ui.tabItem('Link', drawStatusBody)
        end)
      end)
      ui.popStyleColor(3)
    end
  end)

  -- Anything the tabs changed is written here, once, at the end of the frame
  -- that changed it.
  if autoSave() then saveSettings() end

  popLayoutStyle(styles, colors)
end

-- Exported for the LÖVE harness, which draws these on their own to compare
-- pieces of the layout side by side.
script.drawHeader = drawHeader
script.drawTyres = drawTyres
