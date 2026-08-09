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
-- Tried once. Rebuilding these every frame would ask CSP to re-register the
-- same two inputs sixty times a second.
local attempted = false
-- Why there are none, for the settings window to show. A dead end that says
-- "needs CSP in a session" to somebody who is *in* a session is worse than no
-- message: it names the one thing that is definitely not the problem.
local reason = 'not tried yet'

--- Create the bindings, once.
---
--- Called lazily rather than at load: `ac.ControlButton` only exists inside a
--- session under CSP, and the harnesses load this file with a stub. Building
--- them on first use keeps the module loadable either way.
local function ensure()
  if buttons ~= nil then return buttons end
  if attempted then return nil end
  attempted = true

  if type(ac) ~= 'table' or type(ac.ControlButton) ~= 'function' then
    reason = 'this CSP has no ac.ControlButton'
    return nil
  end

  -- Built and then *used*, both inside pcall.
  --
  -- The first version of this checked the shape of what came back — is it a
  -- table, does it have a `pressed` field — and refused the real thing in the
  -- real game while accepting nothing. CSP hands back objects whose methods
  -- live behind a metatable, so `type(button.pressed)` is a question about
  -- indexing rather than about whether the button works. The only honest test
  -- of "can I use this" is to use it.
  local ok, built = pcall(function()
    local made = {
      previousLap = ac.ControlButton('acpe/Debrief: previous lap', DEFAULTS),
      nextLap = ac.ControlButton('acpe/Debrief: next lap', DEFAULTS),
    }
    -- Calling it is the test. A stub that answers every name with something
    -- useless fails here; a real control button returns a boolean.
    local _ = made.previousLap:pressed()
    local _ = made.nextLap:pressed()
    return made
  end)

  if not ok then
    reason = 'ac.ControlButton failed: ' .. tostring(built)
    return nil
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
--- `AlterRealConfig`, value 64.
---
--- Without it a change is written to whichever controls preset is derived for
--- the current car or mode, and a driver using car-specific controls or
--- presets-per-mode watches the widget accept a button and keep saying it is
--- unbound. The SDK says to use it for buttons that are "more of a global one,
--- not relating to currently selected car", and paging a debrief is about as
--- global as a binding gets.
---
--- Written as a number rather than read from `ui.ControlButtonControlFlags`,
--- because that table is one more thing that has to exist for the widget to
--- draw at all, and the value is part of the API.
local ALTER_REAL_CONFIG = 64

--- The rebinding widgets, for the settings window.
---
--- `:control()` draws the current binding and takes a new one when clicked, so
--- the driver assigns a wheel button from inside the app and AC stores it.
---
--- The size is given rather than left to default. Unset, it takes "next item
--- width", which is whatever the last widget left behind — and in a column laid
--- out by this panel that has been zero, which draws a label with nothing
--- clickable under it: a binding widget that looks right and cannot be pressed.
function M.drawControls(width)
  local held = ensure()
  if held == nil then return false end

  local size = vec2(width or 220, 0)
  local changed = held.previousLap:control(size, ALTER_REAL_CONFIG)
  changed = held.nextLap:control(size, ALTER_REAL_CONFIG) or changed
  return changed
end

--- What each binding is, and whether it is being pressed *right now*.
---
--- Returned as data rather than drawn here, so the settings window can lay it
--- out at the panel's own text size.
---
--- The live half is the point. A binding that does not work is otherwise
--- invisible from both ends: the widget looks the same whether or not the panel
--- ever receives the button, and the only symptom is that nothing happens —
--- which is also what a wrong lap index, a closed window and a missing frame
--- look like. Pressing the button and watching this line tells you in one
--- second which side is at fault.
function M.state()
  local held = ensure()
  if held == nil then return nil end

  local read = function(button)
    local ok, bound = pcall(function() return button:boundTo() end)
    local held_ok, down = pcall(function() return button:down() end)
    return {
      bound = ok and bound or nil,
      down = held_ok and down or false,
    }
  end

  return {
    previousLap = read(held.previousLap),
    nextLap = read(held.nextLap),
  }
end

--- What each button is bound to, or nil. For showing state without the widget.
function M.boundTo()
  local held = ensure()
  if held == nil then return nil, nil end
  return held.previousLap:boundTo(), held.nextLap:boundTo()
end

M.available = function() return ensure() ~= nil end

--- Why the bindings are unavailable, when they are.
function M.reason()
  return reason
end

return M
