-- Wheel and keyboard bindings for the panel, held by the game.
--
-- The panel wants two buttons — page back through the debrief, page forward —
-- and a driver with their hands on a wheel cannot reach a mouse to press them.
--
-- `ac.ControlButton` is how that is done without inventing an input system:
-- each binding is a section in Assetto Corsa's own `controls.ini`, read by the
-- game's input layer, so it takes wheel buttons, a gamepad or the keyboard with
-- no idea here of which is which. The panel asks whether the button was pressed
-- and never touches a device. Assigning one is `:control()`, a widget drawn in
-- the settings window: the binding is made here, in the app, and handled by the
-- game — which is the whole point of doing it this way rather than watching
-- keys ourselves.
--
-- The ids are namespaced. `controls.ini` is shared with AC and every other CSP
-- app, and an unprefixed name is a collision waiting to happen with somebody
-- else's app.

local M = {}

-- Nothing bound by default, deliberately.
--
-- A default that happens to be free on one wheel is a default that steals a
-- button somebody uses for the pit limiter on another. The settings window
-- says the buttons are unbound and offers the widget to set them; a driver who
-- never opens it loses nothing, because the on-screen arrows still work.
local DEFAULTS = nil

local buttons = nil

--- Create the bindings, once.
---
--- Called lazily rather than at load: `ac.ControlButton` only exists inside a
--- session under CSP, and the harnesses load this file with a stub. Building
--- them on first use keeps the module loadable either way.
local function ensure()
  if buttons ~= nil then return buttons end
  if type(ac) ~= 'table' or type(ac.ControlButton) ~= 'function' then
    return nil
  end
  -- Checked, not assumed. A CSP old enough to lack `ac.ControlButton` is one
  -- case; a stub that answers every name with something that is not a control
  -- button is another, and the panel meets the second every time it runs under
  -- a harness. Either way the arrows on screen still work, so a missing
  -- binding API costs a feature and not the panel.
  local built = {
    previousLap = ac.ControlButton('acpe/Debrief: previous lap', DEFAULTS),
    nextLap = ac.ControlButton('acpe/Debrief: next lap', DEFAULTS),
  }
  for _, button in pairs(built) do
    if type(button) ~= 'table' and type(button) ~= 'userdata' then return nil end
    if type(button.pressed) ~= 'function' then return nil end
  end

  buttons = built
  return buttons
end

--- Poll the bindings and report which way the driver asked to go.
---
--- The value is what `blocks.debriefStep` takes, said in indices rather than in
--- words: the laps are newest-first, so **+1 is a step back in time** and -1 is
--- a step towards the newest. Returning "-1 for the previous lap" would read
--- fine and page the wrong way.
---
--- Read once per update rather than per window: `:pressed()` is true for a
--- single frame, so two windows both asking would leave the second seeing
--- nothing.
function M.debriefStep()
  local held = ensure()
  if held == nil then return 0 end
  if held.previousLap:pressed() then return 1 end
  if held.nextLap:pressed() then return -1 end
  return 0
end

--- The rebinding widgets, for the settings window.
---
--- `:control()` draws the current binding and takes a new one when clicked, so
--- the driver assigns a wheel button from inside the app and AC stores it.
function M.drawControls()
  local held = ensure()
  if held == nil then return false end
  local changed = held.previousLap:control()
  changed = held.nextLap:control() or changed
  return changed
end

--- What each button is bound to, or nil. For showing state without the widget.
function M.boundTo()
  local held = ensure()
  if held == nil then return nil, nil end
  return held.previousLap:boundTo(), held.nextLap:boundTo()
end

M.available = function() return ensure() ~= nil end

return M
