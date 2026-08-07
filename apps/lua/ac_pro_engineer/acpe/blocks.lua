-- The blocks the panel is made of.
--
-- One function per thing on screen, each drawing from `acpe.frame`'s snapshot
-- and `acpe.format`'s strings and computing nothing. The windows under
-- `acpe/windows/` decide which of these to draw and in what order; this decides
-- what each one looks like.

local settings = require('acpe.settings').values
local theme = require('acpe.theme')
local i18n = require('acpe.i18n')
local layout = require('acpe.layout')
local format = require('acpe.format')
local frame = require('acpe.frame')

local COLOR = theme.COLOR
local accentColor = theme.accentColor
local tr = i18n.tr
local say = layout.say
local gap = layout.gap
local stat = layout.stat
local pushRole = layout.pushRole
local sectionLabel = layout.sectionLabel
local contentWidth = layout.contentWidth
local nextColumn = layout.nextColumn
local BULLETS = layout.BULLETS
local SEVERITY_MARK = layout.SEVERITY_MARK
local SEVERITY_WORD = layout.SEVERITY_WORD
local SEVERITY_COLOR = layout.SEVERITY_COLOR
local textSize = layout.textSize
local text = format.text
local gearText = format.gearText
local shown = frame.shown
local hasFlag = frame.hasFlag
local MESSAGE_SLOTS = frame.MESSAGE_SLOTS
local DEMO_ADVICE = frame.DEMO_ADVICE

local TYRE_LABEL = { 'FL', 'FR', 'RL', 'RR' }
local FLAG_PIT_LIMITER = frame.FLAG_PIT_LIMITER
local FLAG_FUEL_WARNING = frame.FLAG_FUEL_WARNING

local M = {}

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

--- How far a corner is from the pressure it is meant to be at.
---
--- Half a psi either way is a setup that works; a psi is a car that does not
--- turn or does not stop.
local function pressureDeltaColor(corner)
  local target = corner <= 2 and shown.target_pressure_front or shown.target_pressure_rear
  if target <= 10 or target >= 45 then return COLOR.dim end
  local difference = math.abs(shown.tyre_pressure_psi[corner] - target)
  if difference <= 0.5 then return COLOR.good end
  if difference <= 1.0 then return COLOR.warn end
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

--- The RPM bar. Drawn by hand rather than with progressBar so the redline
--- segment can be shaded separately.
local function rpmBar(width)
  local ratio = rpmRatio()
  -- Thin lines disappear at VR resolutions; this is the one element that is
  -- read peripherally, so it is the one that has to survive that.
  local base = settings.vrMode and math.max(settings.barHeight, 12) or settings.barHeight
  local height = math.max(2, math.floor(base * layout.windowScale() + 0.5))
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
  say('caption', tr(settings.mph and 'MPH' or 'KM/H'), COLOR.label)
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

      -- What it should be, next to what it is. The number on its own is a
      -- reading; the difference is a decision.
      if settings.showPressureTarget and text.pressureDelta[i] ~= '' then
        ui.sameLine()
        say('caption', text.pressureDelta[i], pressureDeltaColor(i))
      end

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
  -- forty characters are the ones that carry the advice. With wrapping on the
  -- line has already been broken up, so nothing is cut.
  local limit = settings.engineerMaxChars or 64
  if not settings.engineerWrap and #text > limit then
    text = text:sub(1, limit - 1) .. '…'
  end
  ui.dwriteText(text, textSize('body') * (settings.engineerScale or 1), color)
end

-- How tall the advice came out last frame.
--
-- The plate has to be drawn before the text — a rectangle drawn afterwards
-- covers it — so it cannot know the height of the thing it is backing. Reusing
-- the last frame's measurement is one frame stale, and a block that only
-- changes when a new advice line arrives never shows that.
local engineerPlateHeight = 0

local function drawEngineerMessages(withLabel)
  if withLabel ~= false then sectionLabel('ENGINEER') end

  -- A plate behind the advice, on its own. Numbers are read in a glance and
  -- forgiven a busy background; a sentence is not.
  --
  -- It used to stop at 140 pixels and at the right edge of the content, so in
  -- the advice window — where the advice *is* the window — it came out as a
  -- strip across the top corner with the text running off the bottom of it.
  -- In the window it covers everything; in the panel it covers the block and
  -- not the readouts underneath.
  local plateOrigin = ui.getCursor()
  if settings.engineerBackground > 0.01 then
    local space = ui.availableSpace()
    local height = space.y
    if withLabel ~= false then
      height = math.min(space.y, math.max(engineerPlateHeight, 1))
    end
    ui.drawRectFilled(vec2(plateOrigin.x - 6, plateOrigin.y - 4),
      vec2(plateOrigin.x + contentWidth() + 6, plateOrigin.y + height + 4),
      rgbm(0.04, 0.05, 0.07, settings.engineerBackground), 4)
  end

  if settings.devSampleAdvice then
    shown.message_count = MESSAGE_SLOTS
    for i = 1, MESSAGE_SLOTS do
      shown.messages[i] = DEMO_ADVICE[i]
      shown.message_severity[i] = (i - 1) % 3
    end
  end

  local bySeverity = settings.engineerBullet == 'severity'
  local bullet = BULLETS[settings.engineerBullet] or ''
  local count = math.min(shown.message_count, settings.engineerLines, MESSAGE_SLOTS)

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

      -- Wrapped here rather than by `ui.textWrapped`, which draws in CSP's own
      -- font: a marker at the panel's size beside a sentence at CSP's size is
      -- the mismatch that made the advice look like a footnote.
      local body = bySeverity and line or (mark .. line)
      local color = bySeverity and COLOR.text or textColor
      if settings.engineerWrap then
        local limit = settings.engineerMaxChars or 64
        while #body > limit do
          local cut = body:sub(1, limit):match('^.*()%s') or limit
          sayAdvice(body:sub(1, cut - 1), color)
          body = body:sub(cut + 1)
        end
      end
      sayAdvice(body, color)
      if settings.engineerSeparator then ui.separator() end
      if settings.engineerSpacing then gap(settings.engineerLineGap or 4) end
    end
  end

  if count == 0 and withLabel ~= false then
    say('caption', tr('nothing to report'), COLOR.dim)
  elseif settings.engineerShowCount then
    say('caption', string.format('%d of %d shown', count, shown.message_count), COLOR.dim)
  end

  -- What the plate above will be drawn to next frame.
  engineerPlateHeight = math.max(0, ui.getCursor().y - plateOrigin.y)
end

--- What every window shows while the desktop application is not publishing.
---
--- Nothing else is drawn in that state on purpose: numbers from a dead feed
--- are worse than no numbers, and a panel that looks broken sends people
--- looking for the wrong problem. This says which problem it is.
local function drawWaitingForApp()
  if frame.openError() ~= nil then
    pushRole('body')
    ui.pushStyleColor(ui.StyleColor.Text, COLOR.bad)
    ui.textWrapped(tr('Waiting for AC Pro Engineer'))
    ui.popStyleColor()
    ui.popFont()

    pushRole('caption')
    ui.pushStyleColor(ui.StyleColor.Text, COLOR.dim)
    ui.textWrapped('The shared mapping is not there yet. Start the desktop '
      .. 'application — it creates the mapping, and this panel picks it up '
      .. 'within a couple of seconds.')
    -- Named because this is the state a Linux driver sits in with everything
    -- apparently running. The application writes /dev/shm itself, but only
    -- shm-bridge.exe gives that file the Win32 name CSP can open, so without
    -- it the panel waits forever beside a mapping that is right there.
    ui.textWrapped('On Linux shm-bridge.exe must be running in the game\'s '
      .. 'Proton prefix as well — the panel cannot see the mapping without it.')
    ui.textWrapped(tr('panel') .. ' v' .. frame.panelVersion()
      .. ', ' .. tr('frame') .. ' v' .. frame.expectedVersion())
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
  ui.textWrapped(tr('Start the desktop application to see telemetry.'))
  if shown.sequence == 0 then
    ui.textWrapped(tr('Nothing has been published yet.'))
  else
    ui.textWrapped(string.format('Last frame %.0f s ago.', frame.secondsSinceChange()))
  end
  ui.popStyleColor()
  ui.popFont()
end

--- The application is publishing, but there is no car yet.
---
--- Distinct from `drawWaitingForApp` on purpose, and the distinction is the
--- whole point of this screen: one of them is a problem to go and fix, the
--- other is the pit garage. They used to look identical, because the
--- application published nothing at all until a session was live — so anyone
--- opening the panel before a race was told it was not running, and went
--- looking through the bridge, the install and the Proton prefix for a fault
--- that was not there.
local function drawWaitingForCar()
  pushRole('body')
  ui.pushStyleColor(ui.StyleColor.Text, COLOR.accent)
  ui.textWrapped(tr('Waiting for the car'))
  ui.popStyleColor()
  ui.popFont()

  pushRole('caption')
  ui.pushStyleColor(ui.StyleColor.Text, COLOR.dim)
  ui.textWrapped(tr('AC Pro Engineer is running. Telemetry starts when you go on track.'))
  ui.textWrapped(tr('panel') .. ' v' .. frame.panelVersion()
    .. (shown.app_version ~= '' and ('  ·  ' .. tr('app') .. ' v' .. shown.app_version) or ''))
  ui.popStyleColor()
  ui.popFont()
end

--- Say so, once, at the top of whichever window is being drawn.
local function drawUpdateNotice()
  if not frame.panelIsStale() then return end
  pushRole('caption')
  ui.pushStyleColor(ui.StyleColor.Text, COLOR.accent)
  ui.textWrapped(string.format(
    tr('Panel %s is installed — restart Assetto Corsa to load it'),
    shown.app_version))
  ui.popStyleColor()
  ui.popFont()
end

M.rpmRatio = rpmRatio
M.rpmColor = rpmColor
M.rpmBar = rpmBar
M.header = drawHeader
M.tyres = drawTyres
M.timing = drawTiming
M.fuel = drawFuel
M.session = drawSession
M.engineer = drawEngineerMessages
M.waitingForApp = drawWaitingForApp
M.waitingForCar = drawWaitingForCar
M.updateNotice = drawUpdateNotice

return M
