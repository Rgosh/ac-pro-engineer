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

  -- How the advice reads. The application decides how many lines it publishes;
  -- these decide how many of them are drawn and what they look like.
  engineerLines = 4,
  engineerBullet = 'severity',   -- severity | > | dot | none
  engineerWrap = true,
  engineerHighlight = true,

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
local SEVERITY_COLOR = { [0] = COLOR.good, [1] = COLOR.warn, [2] = COLOR.bad }

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

local function pressureText(psi)
  if settings.psi then
    return string.format('%.1f psi', psi)
  end
  return string.format('%.2f bar', psi * 0.0689476)
end

--- A section caption, or nothing when the panel is set to run without them.
local function sectionLabel(text)
  if not settings.sectionLabels then return end
  pushRole('caption')
  ui.textColored(text, COLOR.label)
  ui.popFont()
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
    shown.messages[i] = frame.messages[i - 1]
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

function script.update(dt)
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
  pushRole('caption')
  ui.textColored(label, COLOR.label)
  ui.popFont()
  pushRole('body')
  ui.textColored(value, color or COLOR.text)
  ui.popFont()
  ui.endGroup()
end

-- How wide the content is allowed to get, whatever the window does.
--
-- A CSP window is resized by the driver and can end up half a screen wide.
-- Columns measured against the whole width then drift apart until a block of
-- four tyres reads as four unrelated numbers, so the layout keeps to a column
-- and lets the rest of the window be empty.
local MAX_CONTENT = 360
local MAX_CONTENT_VR = 520

local function contentWidth()
  local limit = settings.vrMode and MAX_CONTENT_VR or MAX_CONTENT
  return math.min(ui.availableSpaceX(), limit)
end

--- Where column `index` of a row of stats begins.
---
--- Three across normally. Two in VR, where the text is large enough that a
--- third column runs into its neighbour — the wrap happens by itself, since a
--- stat leaves the cursor at the start of the next line.
local function nextColumn(width, index)
  local perRow = settings.vrMode and 2 or 3
  local column = index % perRow
  if column > 0 then ui.sameLine(width / perRow * column) end
end

--- The RPM bar. Drawn by hand rather than with progressBar so the redline
--- segment can be shaded separately.
local function rpmBar(width)
  local ratio = rpmRatio()
  -- Thin lines disappear at VR resolutions; this is the one element that is
  -- read peripherally, so it is the one that has to survive that.
  local height = settings.vrMode and 14 or 6
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
  pushRole('hero')
  ui.textColored(string.format('%.0f', shown.speed_kmh), COLOR.text)
  ui.popFont()
  ui.endGroup()

  ui.sameLine()

  ui.beginGroup()
  pushRole('caption')
  ui.textColored('KM/H', COLOR.label)
  ui.popFont()
  pushRole('gear')
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

      -- Tyre Label
      pushRole('caption')
      ui.textColored(TYRE_LABEL[i], COLOR.dim)
      ui.popFont()
      ui.sameLine()

      -- Pressure
      pushRole('body')
      ui.textColored(pressureText(shown.tyre_pressure_psi[i]), COLOR.text)
      ui.popFont()

      -- Temps
      pushRole('caption')
      ui.textColored('T: ' .. tempText(shown.tyre_temp_c[i]), tyreTempColor(shown.tyre_temp_c[i]))
      ui.sameLine()
      ui.textColored('B: ' .. tempText(shown.brake_temp_c[i]), brakeColor(shown.brake_temp_c[i]))

      -- Wear
      ui.textColored(string.format('Wear: %.0f%%', shown.tyre_wear_percent[i]), wearColor(shown.tyre_wear_percent[i]))
      ui.popFont()

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

  nextColumn(width, 0)
  stat('DELTA', string.format('%+.3f', shown.delta_seconds), deltaColor)
  nextColumn(width, 1)
  stat('BEST', lapTimeText(shown.best_lap_ms), COLOR.purple)
  nextColumn(width, 2)
  stat('LAST', lapTimeText(shown.last_lap_ms), COLOR.text)
end

local function drawFuel()
  local color = hasFlag(FLAG_FUEL_WARNING) and COLOR.bad or COLOR.text

  local width = contentWidth()

  nextColumn(width, 0)
  stat('FUEL', string.format('%.1f L', shown.fuel_litres), color)
  nextColumn(width, 1)
  stat('LAPS LEFT',
    shown.fuel_laps_remaining > 0 and string.format('%.1f', shown.fuel_laps_remaining) or '--',
    color)
  nextColumn(width, 2)
  stat('PER LAP',
    shown.fuel_per_lap > 0 and string.format('%.2f L', shown.fuel_per_lap) or '--',
    COLOR.dim)
end

--- Where the session is: position, lap, the lap running now, and the
--- conditions that explain why the tyres are behaving as they are.
local function drawSession()
  sectionLabel('SESSION')

  local width = contentWidth()
  nextColumn(width, 0)
  stat('POS', shown.position > 0 and string.format('P%d', shown.position) or '--', COLOR.text)
  nextColumn(width, 1)
  stat('LAP', tostring(shown.lap_count), COLOR.text)
  nextColumn(width, 2)
  stat('CURRENT', lapTimeText(shown.current_lap_ms), COLOR.accent)

  pushRole('caption')
  ui.textColored(string.format('AIR %s   ROAD %s   GRIP %.0f%%',
    tempText(shown.air_temp_c), tempText(shown.road_temp_c), shown.surface_grip * 100),
    COLOR.dim)
  ui.popFont()
end

--- The engineer's lines, drawn the way the settings ask for.
---
--- `withLabel` is false in the advice window, where the window's own title
--- already says what this is.
local function drawEngineerMessages(withLabel)
  if withLabel ~= false then sectionLabel('ENGINEER') end

  local bySeverity = settings.engineerBullet == 'severity'
  local bullet = BULLETS[settings.engineerBullet] or ''
  local count = math.min(shown.message_count, settings.engineerLines, 4)

  pushRole('body')
  for i = 1, count do
    if shown.messages[i] ~= '' then
      local level = shown.message_severity[i] or 0
      local mark = bySeverity and (SEVERITY_MARK[level] or '') or bullet
      -- The application colours the marker and leaves the sentence readable;
      -- doing the same here is the whole point of shipping severity across.
      local markColor = SEVERITY_COLOR[level] or COLOR.text
      local textColor = settings.engineerHighlight and markColor or COLOR.text

      if bySeverity then
        ui.textColored(mark, markColor)
        ui.sameLine()
      end

      if settings.engineerWrap then
        -- textWrapped takes the colour from the style stack, not an argument.
        ui.pushStyleColor(ui.StyleColor.Text, bySeverity and COLOR.text or textColor)
        ui.textWrapped(bySeverity and shown.messages[i] or (mark .. shown.messages[i]))
        ui.popStyleColor()
      else
        ui.textColored(bySeverity and shown.messages[i] or (mark .. shown.messages[i]),
          bySeverity and COLOR.text or textColor)
      end
    end
  end
  ui.popFont()

  if count == 0 and withLabel ~= false then
    pushRole('caption')
    ui.textColored('nothing to report', COLOR.dim)
    ui.popFont()
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

  if shown.version ~= EXPECTED_VERSION then
    ui.textColored('Version mismatch', COLOR.bad)
    pushRole('caption')
    ui.textColored(string.format('app v%d, overlay v%d', shown.version, EXPECTED_VERSION),
      COLOR.dim)
    ui.popFont()
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

  drawEngineerMessages(false)
end

-- ---------------------------------------------------------------------------
-- Settings window
--
-- CSP opens this from the app's own entry in the sidebar, and it is a separate
-- window from the panel: none of this runs while the overlay is being drawn.
-- ---------------------------------------------------------------------------

--- A checkbox bound to a settings field. `ui.checkbox` reports the click, not
--- the new value, so the flip happens here.
local function settingToggle(label, key)
  if ui.checkbox(label, settings[key]) then
    settings[key] = not settings[key]
  end
end

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
  ui.tabBar('acpeSettings', function()
    ui.tabItem('Sections', function()
      settingToggle('Speed and gear', 'showHeader')
      settingToggle('RPM bar', 'showRpmBar')
      settingToggle('Tyres and brakes', 'showTyres')
      settingToggle('Lap timing', 'showTiming')
      settingToggle('Fuel', 'showFuel')
      settingToggle('Session', 'showSession')
      settingToggle('Engineer advice', 'showEngineer')
      settingToggle('Section captions', 'sectionLabels')

      -- Sections the application itself is suppressing. Without this the
      -- settings read as broken: a box is ticked and nothing appears.
      local held = not hasFlag(FLAG_SHOW_TELEMETRY) or not hasFlag(FLAG_SHOW_ENGINEER)
        or not hasFlag(FLAG_SHOW_SESSION) or not hasFlag(FLAG_SHOW_TIMING)
        or not hasFlag(FLAG_SHOW_FUEL)
      if isLive and held then
        ui.separator()
        pushRole('caption')
        if not hasFlag(FLAG_SHOW_TELEMETRY) then
          ui.textColored('Telemetry is off in the desktop app', COLOR.warn)
        end
        if not hasFlag(FLAG_SHOW_ENGINEER) then
          ui.textColored('Engineer advice is off in the desktop app', COLOR.warn)
        end
        if not hasFlag(FLAG_SHOW_SESSION) then
          ui.textColored('Session block is off in the desktop app', COLOR.warn)
        end
        if not hasFlag(FLAG_SHOW_TIMING) then
          ui.textColored('Lap timing is off in the desktop app', COLOR.warn)
        end
        if not hasFlag(FLAG_SHOW_FUEL) then
          ui.textColored('Fuel block is off in the desktop app', COLOR.warn)
        end
        ui.popFont()
      end
    end)

    ui.tabItem('Engineer', function()
      pushRole('caption')
      ui.textColored('LINES', COLOR.label)
      ui.popFont()
      for _, lines in ipairs({ 1, 2, 3, 4 }) do
        if ui.radioButton(string.format('%d line%s', lines, lines > 1 and 's' or ''),
            settings.engineerLines == lines) then
          settings.engineerLines = lines
        end
      end

      ui.separator()
      pushRole('caption')
      ui.textColored('MARKER', COLOR.label)
      ui.popFont()
      for _, bullet in ipairs(BULLET_NAMES) do
        if ui.radioButton(bullet, settings.engineerBullet == bullet) then
          settings.engineerBullet = bullet
        end
      end

      ui.separator()
      settingToggle('Wrap long lines', 'engineerWrap')
      settingToggle('Highlight advice', 'engineerHighlight')
    end)

    ui.tabItem('Text', function()
      for _, size in ipairs(TEXT_SIZES) do
        if ui.radioButton(size, settings.textSize == size) then
          settings.textSize = size
        end
      end

      ui.separator()
      settingToggle('VR mode', 'vrMode')
      pushRole('caption')
      ui.textColored('largest text, thicker bar,', COLOR.dim)
      ui.textColored('more spacing, two columns', COLOR.dim)
      ui.popFont()
    end)

    ui.tabItem('Units', function()
      settingToggle('Celsius', 'celsius')
      settingToggle('PSI', 'psi')

      pushRole('caption')
      ui.textColored(settings.celsius and 'temperatures in °C' or 'temperatures in °F', COLOR.dim)
      ui.textColored(settings.psi and 'pressures in psi' or 'pressures in bar', COLOR.dim)
      ui.popFont()

      ui.separator()
      if ui.button('Reset to defaults') then
        for key, value in pairs(DEFAULTS) do settings[key] = value end
      end
    end)
  end)
end

-- Exported for the LÖVE harness, which draws these on their own to compare
-- pieces of the layout side by side.
script.drawHeader = drawHeader
script.drawTyres = drawTyres
