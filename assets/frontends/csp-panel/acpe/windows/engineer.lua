-- The advice, in a window of its own.
--
-- Its own entry in CSP's sidebar, because advice is read while the telemetry is
-- being watched, not instead of it — and because a window that holds a few
-- lines of text wants to sit somewhere else on screen than one holding four
-- corners of tyre data.

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

return function(dt)
  if not frame.live() then
    blocks.waitingForApp()
    return
  end

  if not hasFlag(frame.FLAG_CONNECTED) then
    blocks.waitingForCar()
    return
  end

  if not hasFlag(frame.FLAG_SHOW_ENGINEER) then
    pushRole('caption')
    ui.textColored('Engineer advice is switched off in the desktop app', COLOR.dim)
    ui.popFont()
    return
  end

  local count = math.min(shown.message_count, settings.engineerLines, frame.MESSAGE_SLOTS)
  if count == 0 then
    pushRole('caption')
    ui.textColored('Nothing to report', COLOR.dim)
    ui.popFont()
    return
  end

  local styles, colors = layout.push()
  blocks.engineer(false)
  layout.pop(styles, colors)
end
