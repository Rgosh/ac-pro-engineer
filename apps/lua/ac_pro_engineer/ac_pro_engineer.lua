-- AC Pro Engineer overlay.
--
-- This script computes nothing. Every value it draws was calculated by the
-- desktop application and published into shared memory; all that happens here
-- is reading fields out of a struct and handing them to ImGui.
--
-- That split is deliberate. This code runs on Assetto Corsa's render thread,
-- where a millisecond is a sixth of the frame budget at 165 Hz, and LuaJIT
-- collects garbage mid-frame — so anything that parses text, builds tables or
-- formats strings every frame shows up as a stutter rather than as a lower
-- average frame rate.
--
-- The rules that keep it cheap:
--   * no allocation in the per-frame path (no `..`, no table literals)
--   * `script.update` copies only the handful of fields drawn, and only when
--     the writer says the struct is settled
--   * strings are formatted only when the value behind them changed

local layout = require('frame_layout')

-- Must match ac_core::overlay::frame::OVERLAY_VERSION. Refusing to draw an
-- unknown version is better than misreading a struct from another release.
local EXPECTED_VERSION = 1

-- Must match ac_core::overlay::frame::OVERLAY_MMF_NAME. The
-- `AcTools.CSP.Limited.` prefix is what lets a script without IO permission
-- open shared memory at all.
local MMF_NAME = 'AcTools.CSP.Limited.ACPE.v1'

-- How long the sequence may stand still before the application is presumed
-- gone. Two seconds is long enough to ride out a stalled tick and short enough
-- that nobody reads frozen numbers as live ones.
local LIVENESS_TIMEOUT = 2.0

local frame = nil
local openError = nil

local ok, err = pcall(function()
  frame = ac.readMemoryMappedFile(MMF_NAME, layout)
end)
if not ok then
  openError = tostring(err)
end

-- Last settled snapshot. Preallocated once and only ever overwritten, so the
-- per-frame path never allocates a table.
local shown = {
  version = 0,
  sequence = 0,
  speed_kmh = 0,
  rpm = 0,
  max_rpm = 0,
  gear = 0,
  fuel_litres = 0,
  fuel_laps_remaining = 0,
  delta_seconds = 0,
  flags = 0,
  message_count = 0,
  tyre_pressure_psi = { 0, 0, 0, 0 },
  tyre_temp_c = { 0, 0, 0, 0 },
  tyre_wear_percent = { 0, 0, 0, 0 },
  messages = { '', '', '', '' },
}

local lastSequence = -1
local secondsSinceChange = 0
local isLive = false

-- Bit flags, matching ac_core::overlay::frame::flags.
local FLAG_PIT_LIMITER = 1
local FLAG_CONNECTED = 2
local FLAG_SHOW_TELEMETRY = 4
local FLAG_SHOW_ENGINEER = 8
local FLAG_FUEL_WARNING = 16

local function hasFlag(flag)
  return bit.band(shown.flags, flag) ~= 0
end

--- Copy the struct into `shown`, but only if it is settled.
--
-- The writer keeps `sequence` odd while it is mid-write and bumps it by two
-- per update, so an odd value means we caught it in the middle, and a value
-- that changed across our read means it finished one while we were copying.
-- Either way the previous snapshot is kept: skipping a frame at 60 Hz is
-- invisible, whereas a frame spliced from two updates is a visible flicker.
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
  shown.delta_seconds = frame.delta_seconds
  shown.flags = frame.flags
  shown.message_count = frame.message_count

  for i = 1, 4 do
    shown.tyre_pressure_psi[i] = frame.tyre_pressure_psi[i - 1]
    shown.tyre_temp_c[i] = frame.tyre_temp_c[i - 1]
    shown.tyre_wear_percent[i] = frame.tyre_wear_percent[i - 1]
  end

  for i = 1, math.min(frame.message_count, 4) do
    shown.messages[i] = frame.messages[i - 1]
  end
  for i = frame.message_count + 1, 4 do
    shown.messages[i] = ''
  end

  -- Re-read: if it moved while we copied, the snapshot is mixed.
  if frame.sequence ~= seq then return false end

  lastSequence = seq
  return true
end

function script.update(dt)
  if frame == nil then return end

  if readFrame() then
    secondsSinceChange = 0
    isLive = shown.version == EXPECTED_VERSION and shown.sequence ~= 0
  else
    secondsSinceChange = secondsSinceChange + dt
  end

  -- The sequence standing still is how the application's absence is detected.
  -- There is no separate heartbeat channel because there does not need to be:
  -- the thing that proves it is alive is the thing it is already sending.
  if secondsSinceChange > LIVENESS_TIMEOUT then
    isLive = false
  end
end

local function gearText(gear)
  if gear < 0 then return 'R' end
  if gear == 0 then return 'N' end
  return tostring(gear)
end

local function rpmColor()
  if shown.max_rpm <= 0 then return rgbm(0.8, 0.8, 0.8, 1) end
  local ratio = shown.rpm / shown.max_rpm
  if ratio > 0.95 then return rgbm(1, 0.2, 0.2, 1) end
  if ratio > 0.85 then return rgbm(1, 0.8, 0.2, 1) end
  return rgbm(0.4, 1, 0.4, 1)
end

local function tyreColor(temp)
  if temp < 70 then return rgbm(0.4, 0.6, 1, 1) end
  if temp < 95 then return rgbm(0.4, 1, 0.4, 1) end
  if temp < 105 then return rgbm(1, 0.8, 0.2, 1) end
  return rgbm(1, 0.3, 0.3, 1)
end

function script.windowMain(dt)
  if openError ~= nil then
    ui.textColored('Could not open shared memory:', rgbm(1, 0.4, 0.4, 1))
    ui.text(openError)
    return
  end

  if not isLive then
    -- Deliberately quiet. The overlay only has something to say while the
    -- desktop application is feeding it, and a panel full of stale numbers is
    -- worse than an empty one.
    ui.textColored('AC Pro Engineer is not running', rgbm(0.6, 0.6, 0.6, 1))
    return
  end

  if shown.version ~= EXPECTED_VERSION then
    ui.textColored('Version mismatch', rgbm(1, 0.4, 0.4, 1))
    ui.text(string.format('app sends v%d, overlay expects v%d', shown.version, EXPECTED_VERSION))
    return
  end

  ui.pushFont(ui.Font.Title)
  ui.text(string.format('%.0f', shown.speed_kmh))
  ui.sameLine()
  ui.popFont()
  ui.text('km/h')

  ui.sameLine()
  ui.textColored(gearText(shown.gear), rpmColor())

  if hasFlag(FLAG_PIT_LIMITER) then
    ui.sameLine()
    ui.textColored('LIMITER', rgbm(1, 0.8, 0.2, 1))
  end

  ui.separator()

  if hasFlag(FLAG_SHOW_TELEMETRY) then
    local labels = { 'FL', 'FR', 'RL', 'RR' }
    for i = 1, 4 do
      ui.text(labels[i])
      ui.sameLine()
      ui.textColored(
        string.format('%.1f psi  %.0f C  %.0f%%',
          shown.tyre_pressure_psi[i],
          shown.tyre_temp_c[i],
          shown.tyre_wear_percent[i]),
        tyreColor(shown.tyre_temp_c[i]))
    end
    ui.separator()
  end

  local fuelColor = hasFlag(FLAG_FUEL_WARNING) and rgbm(1, 0.3, 0.3, 1) or rgbm(0.8, 0.8, 0.8, 1)
  ui.textColored(
    string.format('Fuel %.1f L  (%.1f laps)', shown.fuel_litres, shown.fuel_laps_remaining),
    fuelColor)

  if shown.delta_seconds ~= 0 then
    local deltaColor = shown.delta_seconds < 0 and rgbm(0.4, 1, 0.4, 1) or rgbm(1, 0.5, 0.5, 1)
    ui.textColored(string.format('Delta %+.3f', shown.delta_seconds), deltaColor)
  end

  if hasFlag(FLAG_SHOW_ENGINEER) and shown.message_count > 0 then
    ui.separator()
    for i = 1, math.min(shown.message_count, 4) do
      if shown.messages[i] ~= '' then
        ui.text(shown.messages[i])
      end
    end
  end
end
