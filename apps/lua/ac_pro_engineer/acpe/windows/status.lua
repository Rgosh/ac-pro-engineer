-- What to look at when the panel is empty.
--
-- The questions asked in that state: is the mapping open, is anything arriving,
-- and do the two sides agree about the shape of what arrives. Drawn as its own
-- window and again as a tab under Dev.

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

local M = {}

local function drawStatusBody()
  sectionLabel('LINK')
  -- The mapping's name is the longest string in the panel, and at 46% of the
  -- width in body text it lost its `.v1` off the right-hand edge — which is
  -- precisely the character that matters when the question is whether the
  -- bridge mapped the name this build looks for.
  layout.denseRow('mapping', frame.MMF_NAME, frame.opened() and COLOR.good or COLOR.bad)
  row('state', not frame.opened() and 'not opened' or (frame.live() and 'live' or 'stale'),
    frame.live() and COLOR.good or COLOR.warn)
  -- Live and no car is the normal state in a pit garage, and it used to be
  -- indistinguishable from the application being gone. Named here so the
  -- question "is anything wrong" has an answer without leaving the game.
  row(tr('car'), hasFlag(frame.FLAG_CONNECTED) and tr('on track') or tr('in the garage'),
    hasFlag(frame.FLAG_CONNECTED) and COLOR.good or COLOR.dim)
  if frame.openError() ~= nil then
    pushRole('caption')
    ui.pushStyleColor(ui.StyleColor.Text, COLOR.dim)
    ui.textWrapped(frame.openError())
    ui.popStyleColor()
    ui.popFont()
  end

  sectionLabel('FRAME')
  row('sequence', tostring(shown.sequence))
  row('since change', string.format('%.1f s', frame.secondsSinceChange()))
  row('version', string.format('%d, expected %d', shown.version, frame.expectedVersion()),
    shown.version == frame.expectedVersion() and COLOR.good or COLOR.bad)
  row('advice lines', tostring(shown.message_count))

  sectionLabel('PANEL')
  -- First, and before anything about the panel's own preferences: "which
  -- version of this is installed" is the question every report starts with,
  -- and the answer used to require reading the file in the game folder.
  row('panel version', frame.panelVersion())
  row('app version', shown.app_version ~= '' and shown.app_version or '--',
    frame.panelIsStale() and COLOR.warn or COLOR.good)
  row('frame version', tostring(frame.expectedVersion()))
  blocks.updateNotice()
  row('text size', settings.vrMode and 'VR (large)' or settings.textSize)
  row('units', (settings.celsius and 'C' or 'F') .. ' / ' .. (settings.psi and 'psi' or 'bar'))
end

M.body = drawStatusBody

M.window = function(dt)
  local styles, colors = layout.push()
  drawStatusBody()
  layout.pop(styles, colors)
end

return M
