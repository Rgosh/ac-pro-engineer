-- AC Pro Engineer — the in-game panel.
--
-- This script computes nothing. Every value it draws was calculated by the
-- desktop application and published into shared memory; all that happens here
-- is reading fields out of a struct and handing them to ImGui.
--
-- That split is deliberate. This code runs on Assetto Corsa's render thread,
-- where a millisecond is a sixth of the frame budget at 165 Hz, and LuaJIT
-- collects garbage mid-frame, so anything that parses text, builds tables or
-- formats strings every frame shows up as a stutter rather than as a lower
-- average frame rate. The rules that keep it cheap:
--
--   * nothing is allocated per frame that can be allocated once
--   * `script.update` copies only the fields drawn, and only when the writer
--     says the struct is settled
--   * colours are picked from preallocated constants, never built with rgbm()
--     inside the draw path
--
-- This file is the entry point CSP loads, and nothing else. The panel itself
-- lives under `acpe/`:
--
--   acpe/settings.lua        what the driver chose, and making it stick
--   acpe/i18n.lua            the panel's own words, in two languages
--   acpe/theme.lua           colours, accents, the editable palette
--   acpe/layout.lua          text sizes, spacing, the measured window
--   acpe/format.lua          numbers into strings, once per settled frame
--   acpe/frame.lua           the shared block and the snapshot drawn from it
--   acpe/blocks.lua          one function per thing on screen
--   acpe/controls.lua        the widgets the settings window is built from
--   acpe/console.lua         typed commands, for what has no widget
--   acpe/windows/*.lua       one file per window CSP opens
--
-- `frame_layout.lua` beside this file is generated — see the note at its top.

-- Must match ac_core::overlay::frame::OVERLAY_VERSION.
--
-- The shape of the struct, not the release. It changes when a field moves and
-- stays put when everything else does, which is why it is a small integer and
-- not a version string: the only question it answers is whether this panel and
-- the application that wrote the frame agree about where the fields are.
--
-- Declared here rather than in `acpe/frame.lua` because this is the file the
-- installer reads to report what is installed, and the file
-- `cargo test -p ac_core the_panel_reads_the_frame` checks.
local EXPECTED_VERSION = 6

-- The release this panel was shipped in, matching the workspace's Cargo
-- version and the manifest's VERSION.
--
-- Separate from EXPECTED_VERSION on purpose. Most releases do not touch the
-- struct, so the frame version alone cannot tell a panel from January apart
-- from one built this morning — and "the panel is old" is the first thing worth
-- ruling out when something in the game looks wrong. Checked against the crate
-- by `cargo test -p ac_core the_panel_announces`, so it cannot be left behind
-- at release time.
local PANEL_VERSION = '0.3.5'

local frame = require('acpe.frame')
frame.configure(EXPECTED_VERSION, PANEL_VERSION)

local blocks = require('acpe.blocks')
local windowMain = require('acpe.windows.main')
local windowEngineer = require('acpe.windows.engineer')
local windowDebrief = require('acpe.windows.debrief')
local binds = require('acpe.binds')
local windowSettings = require('acpe.windows.settings')
local telemetry = require('acpe.windows.telemetry')
local status = require('acpe.windows.status')

function script.update(dt)
  frame.update(dt)

  -- Polled here and not in the debrief window: `:pressed()` is true for a
  -- single frame, so whichever window asked first would consume it and the
  -- other would always see false. It also means a wheel button still pages the
  -- debrief while the window is behind another one.
  local step = binds.debriefStep()
  if step ~= 0 then blocks.debriefStep(step) end
end

function script.windowMain(dt)
  windowMain(dt)
end

function script.windowEngineer(dt)
  windowEngineer(dt)
end

function script.windowDebrief(dt)
  windowDebrief(dt)
end

function script.windowSettings(dt)
  windowSettings(dt)
end

function script.windowTelemetry(dt)
  telemetry.window(dt)
end

function script.windowStatus(dt)
  status.window(dt)
end

-- CSP's own settings list, which is where a settings window is supposed to
-- come from.
--
-- The gear in a window's title bar opens a window CSP builds from
-- `FUNCTION_SETTINGS`, and its geometry is CSP's: the script never gets a grip
-- on it, which is why that one cannot be dragged. `ui.addSettings` is the
-- supported way to ask for one — with a default size, a minimum and a maximum
-- of our choosing, so it opens big enough to read and still takes a drag.
--
-- Guarded: an older CSP without the call simply keeps the manifest's window.
--
-- The icon is resolved before the call, and carefully, because the table below
-- is built *before* `pcall` receives it — an error while constructing an
-- argument is not caught by the pcall it is being passed to. This runs at load
-- time, at file scope, so anything thrown here takes the whole script down and
-- every window draws the error instead of the panel. That is what
-- `ui.Icons and ui.Icons.Settings` did: `ui.Icons` only has to be *truthy* for
-- that to index it, and a build where it is a function rather than a table
-- makes it "attempt to index field 'Icons' (a function value)".
--
-- `ui.Icons` is a table in the CSP this was written against. The panel does not
-- own that name, so it checks rather than assumes.
local settingsIcon = nil
if type(ui) == 'table' and type(ui.Icons) == 'table' then
  settingsIcon = ui.Icons.Settings
end

if type(ui) == 'table' and type(ui.addSettings) == 'function' then
  pcall(ui.addSettings, {
    icon = settingsIcon,
    name = 'AC Pro Engineer',
    id = 'acpe.settings',
    -- Twice the old default: this is read through on a 4K screen, where a
    -- 560-wide window is a column of text in the corner of a wall.
    size = {
      default = vec2(1120, 1240),
      min = vec2(300, 240),
      max = vec2(2400, 2000),
      automatic = false,
    },
  }, function() script.windowSettings(0) end)
end

-- Exported for the LÖVE harness, which draws these on their own to compare
-- pieces of the layout side by side.
script.drawHeader = blocks.header
script.drawTyres = blocks.tyres
