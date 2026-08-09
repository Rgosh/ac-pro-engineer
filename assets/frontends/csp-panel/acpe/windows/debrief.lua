-- What the engineer made of the lap you have just finished.
--
-- Its own window, like the live advice, and for a sharper version of the same
-- reason: this one is read stopped. It is what you look at rolling down the
-- pit lane or sitting in the garage, and it wants to be somewhere the live
-- panel is not — a driver who has just come in does not want a debrief on top
-- of their rev counter.
--
-- The laps come out of the frame already written; paging between them is local
-- to the panel, so it works with the game paused and costs nothing.

local settings = require('acpe.settings').values
local theme = require('acpe.theme')
local i18n = require('acpe.i18n')
local layout = require('acpe.layout')
local frame = require('acpe.frame')
local blocks = require('acpe.blocks')

local COLOR = theme.COLOR
local tr = i18n.tr
local pushRole = layout.pushRole

return function(dt)
  if not frame.live() then
    blocks.waitingForApp()
    return
  end

  if not frame.hasFlag(frame.FLAG_CONNECTED) then
    blocks.waitingForCar()
    return
  end

  -- Turned off on the application's side means no laps arrive at all, which
  -- otherwise looks exactly like a session where nobody has finished one.
  if settings.debriefLines == 0 then
    pushRole('caption')
    ui.textColored(tr('the debrief is switched off'), COLOR.dim)
    ui.popFont()
    return
  end

  local styles, colors = layout.push()
  -- Scrolled, because eight lines plus a header plus sectors do not fit a
  -- window sized for four and the overflow was simply cut off at the bottom
  -- edge with nothing to say it had been. `ui.childWindow` is the wrapper that
  -- survives a script error midway, which matters here: this draws advice text
  -- whose length the panel does not control.
  if type(ui.childWindow) == 'function' and settings.debriefScroll then
    ui.childWindow('acpeDebriefScroll', ui.availableSpace(), false, function()
      blocks.debrief(false)
    end)
  else
    blocks.debrief(false)
  end
  layout.pop(styles, colors)
end
