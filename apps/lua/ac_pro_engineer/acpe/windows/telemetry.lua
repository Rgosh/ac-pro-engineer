-- Every field in the frame, as it arrived.
--
-- The panel decides what is worth a driver's attention mid-corner; this is for
-- the other question — whether a number is reaching the game at all — and it
-- answers it without alt-tabbing. Drawn as its own window and again as a tab
-- under Dev, which is why the body is exported separately from the window.

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
local denseRow = layout.denseRow
local text = format.text
local shown = frame.shown
local hasFlag = frame.hasFlag
local pressureText = format.pressureText
local tempText = format.tempText
local lapTimeText = format.lapTimeText
local gearText = format.gearText
local TYRE_LABEL = { 'FL', 'FR', 'RL', 'RR' }

local M = {}

local function drawTelemetryBody()
  if not frame.live() then
    blocks.waitingForApp()
    return
  end

  sectionLabel('CAR')
  row('speed', string.format('%.1f km/h', shown.speed_kmh))
  row('rpm', string.format('%d / %d', shown.rpm, shown.max_rpm))
  row('gear', gearText(shown.gear))

  sectionLabel('FUEL')
  row('in tank', string.format('%.2f L', shown.fuel_litres))
  row('per lap', string.format('%.2f L', shown.fuel_per_lap))
  row('laps left', string.format('%.1f', shown.fuel_laps_remaining))

  sectionLabel('TIMING')
  row('delta', string.format('%+.3f s', shown.delta_seconds))
  row('best', lapTimeText(shown.best_lap_ms))
  row('last', lapTimeText(shown.last_lap_ms))
  row('current', lapTimeText(shown.current_lap_ms))

  sectionLabel('SESSION')
  row('position', string.format('P%d', shown.position))
  row('lap', tostring(shown.lap_count))
  row('air', tempText(shown.air_temp_c))
  row('road', tempText(shown.road_temp_c))
  row('grip', string.format('%.0f%%', shown.surface_grip * 100))

  sectionLabel('CORNERS')
  for i = 1, 4 do
    denseRow(TYRE_LABEL[i], string.format('%s  %s  %s  %.0f%%',
      pressureText(shown.tyre_pressure_psi[i]),
      tempText(shown.tyre_temp_c[i]),
      tempText(shown.brake_temp_c[i]),
      shown.tyre_wear_percent[i]))
  end

  sectionLabel('FLAGS')
  for _, entry in ipairs(frame.FLAG_NAMES) do
    local on = hasFlag(entry[2])
    row(entry[1], on and 'on' or 'off', on and COLOR.good or COLOR.dim)
  end
end

M.body = drawTelemetryBody

M.window = function(dt)
  local styles, colors = layout.push()
  drawTelemetryBody()
  layout.pop(styles, colors)
end

return M
