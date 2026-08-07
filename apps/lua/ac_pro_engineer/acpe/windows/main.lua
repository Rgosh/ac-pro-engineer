-- The panel: speed, revs, tyres, timing, fuel, session, advice.
--
-- CSP owns the window. The manifest declares it, CSP draws the frame, the title
-- bar and the background, and the driver moves and resizes it. So this draws
-- contents and nothing else — `ui.begin` does not exist in the app SDK
-- (`cargo test -p ac_core the_overlay_app_only_calls` checks that against the
-- installed CSP), and pushing WindowBg or WindowRounding from in here would
-- style a window that was never opened.

local settings = require('acpe.settings').values
local theme = require('acpe.theme')
local i18n = require('acpe.i18n')
local layout = require('acpe.layout')
local format = require('acpe.format')
local frame = require('acpe.frame')
local blocks = require('acpe.blocks')

local COLOR = theme.COLOR
local tr = i18n.tr
local say = layout.say
local gap = layout.gap
local pushRole = layout.pushRole
local sectionLabel = layout.sectionLabel
local contentWidth = layout.contentWidth
local row = layout.row
local text = format.text
local shown = frame.shown
local hasFlag = frame.hasFlag
local gearText = format.gearText

return function(dt)
  if not frame.live() then
    blocks.waitingForApp()
    return
  end

  if shown.version ~= frame.expectedVersion() and not settings.devIgnoreVersion then
    ui.textColored(tr('Version mismatch'), COLOR.bad)
    pushRole('caption')
    ui.textColored(string.format('the application writes frame v%d, this panel reads v%d',
      shown.version, frame.expectedVersion()), COLOR.dim)
    -- The release, not just the frame number: which of the two to replace is
    -- the actual question, and only the release names answer it.
    ui.textColored(string.format('panel v%s — reinstall it from the desktop application',
      frame.panelVersion()), COLOR.dim)
    ui.popFont()
    return
  end

  -- The application is there and there is no car. Not a fault, and not the
  -- same screen as "the application is not running".
  if not hasFlag(frame.FLAG_CONNECTED) then
    blocks.updateNotice()
    blocks.waitingForCar()
    return
  end

  local styles, colors = layout.push()

  blocks.updateNotice()

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
      vec2(origin.x + contentWidth(), origin.y + space.y), theme.accentColor(), 2, 1)
  end

  -- One line, for a driver who wants the panel out of the way: speed, gear,
  -- delta and fuel, and nothing else on screen.
  if settings.hudMode then
    say('hero', string.format('%.0f', shown.speed_kmh), COLOR.text)
    ui.sameLine()
    say('gear', gearText(shown.gear), blocks.rpmColor(blocks.rpmRatio()))
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
      hasFlag(frame.FLAG_FUEL_WARNING) and COLOR.bad or COLOR.dim)
    if settings.showRpmBar then blocks.rpmBar(contentWidth()) end
    layout.pop(styles, colors)
    return
  end

  if settings.showHeader then
    blocks.header()
    gap(6)
  end

  -- Two gates on each section, and they mean different things: the flag is the
  -- application saying it has nothing to show, the setting is the driver
  -- saying they do not want to see it. Everything here can be switched off —
  -- a panel in the corner of a windscreen earns its space or loses it.
  if hasFlag(frame.FLAG_SHOW_TELEMETRY) then
    if settings.showTyres then
      blocks.tyres()
      gap(6)
    end
  end

  if hasFlag(frame.FLAG_SHOW_TIMING) and settings.showTiming then
    blocks.timing()
    gap(6)
  end

  if hasFlag(frame.FLAG_SHOW_FUEL) and settings.showFuel then
    blocks.fuel()
    gap(6)
  end

  if hasFlag(frame.FLAG_SHOW_SESSION) and settings.showSession then
    blocks.session()
  end

  -- The advice also has a window of its own; this block is for keeping it in
  -- the corner of the eye without a second window on screen.
  if hasFlag(frame.FLAG_SHOW_ENGINEER) and settings.showEngineer and shown.message_count > 0 then
    gap(8)
    blocks.engineer()
  end

  layout.pop(styles, colors)
end

-- ---------------------------------------------------------------------------
