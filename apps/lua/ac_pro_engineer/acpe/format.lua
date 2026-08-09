-- Numbers into strings, once per settled frame.
--
-- The application publishes about sixty times a second; AC draws at whatever
-- the car is running at, 165 or more. Formatting in the draw path meant the
-- same numbers were turned into the same strings three times over, and every
-- one of those strings is garbage for a collector that runs mid-frame.
--
-- Colours are decided from the real Celsius and psi values elsewhere; only the
-- text that reaches the screen is converted. A warning that changed meaning
-- with the unit setting would be a bug, not a feature.

local settings = require('acpe.settings').values

local M = {}

local function tempText(celsius)
  if settings.celsius then
    return string.format('%.0f°C', celsius)
  end
  return string.format('%.0f°F', celsius * 1.8 + 32)
end

local function gearText(gear)
  if gear < 0 then return 'R' end
  if gear == 0 then return 'N' end
  return tostring(gear)
end

local function lapTimeText(ms)
  if ms <= 0 then return settings.shortLapTimes and '--.--' or '--:--.---' end
  local minutes = math.floor(ms / 60000)
  local seconds = math.floor((ms % 60000) / 1000)
  local millis = ms % 1000
  if settings.shortLapTimes then
    return string.format('%d:%02d.%01d', minutes, seconds, math.floor(millis / 100))
  end
  return string.format('%d:%02d.%03d', minutes, seconds, millis)
end

--- Speed and volume in whichever unit the driver thinks in.
local function speedText(kmh)
  if settings.mph then
    return string.format(settings.unitSuffix and '%.0f mph' or '%.0f', kmh * 0.621371)
  end
  return string.format('%.0f', kmh)
end

local function volumeText(litres)
  if settings.gallons then
    return string.format(settings.unitSuffix and '%.1f gal' or '%.1f', litres * 0.264172)
  end
  return string.format(settings.unitSuffix and '%.1f L' or '%.1f', litres)
end

local PRESSURE_FORMAT = { [0] = '%.0f psi', '%.1f psi', '%.2f psi' }
local BAR_FORMAT = { [0] = '%.1f bar', '%.2f bar', '%.3f bar' }
local PLAIN_FORMAT = { [0] = '%.0f', '%.1f', '%.2f' }

local function pressureText(psi)
  local decimals = settings.pressureDecimals or 1
  if not settings.unitSuffix then
    return string.format(PLAIN_FORMAT[decimals] or '%.1f',
      settings.psi and psi or psi * 0.0689476)
  end
  if settings.psi then
    return string.format(PRESSURE_FORMAT[decimals] or '%.1f psi', psi)
  end
  return string.format(BAR_FORMAT[decimals] or '%.2f bar', psi * 0.0689476)
end

local text = {
  speed = '0', gear = 'N',
  pressure = { '', '', '', '' },
  tyreTemp = { '', '', '', '' },
  tyreEdges = { '', '', '', '' },
  brakeTemp = { '', '', '', '' },
  wear = { '', '', '', '' },
  delta = '+0.000', best = '', last = '', current = '',
  pressureDelta = { '', '', '', '' },
  fuel = '', lapsLeft = '', perLap = '',
  position = '', lap = '', conditions = '',
}

--- Turn the snapshot into the strings the panel draws.
---
--- Called once per settled frame and once when a setting that changes a format
--- is touched — never from the draw path.
function M.rebuild(shown)
  text.speed = speedText(shown.speed_kmh)
  text.gear = gearText(shown.gear)

  for i = 1, 4 do
    text.pressure[i] = pressureText(shown.tyre_pressure_psi[i])

    local target = i <= 2 and shown.target_pressure_front or shown.target_pressure_rear
    -- A frame from an older application has something else at this offset, and
    -- it comes out as -1.7e27. Anything outside the range a tyre is ever run at
    -- is not a target, it is a mismatch, and the panel says nothing rather than
    -- showing nonsense next to a real reading.
    if target > 10 and target < 45 then
      local difference = shown.tyre_pressure_psi[i] - target
      text.pressureDelta[i] = string.format('%+.1f', difference)
    else
      text.pressureDelta[i] = ''
    end
    text.tyreTemp[i] = 'T: ' .. tempText(shown.tyre_temp_c[i])
    -- Inner / middle / outer, when the driver asks for it. The middle one on
    -- its own says how hot the tyre is; these three say whether it is leaning
    -- the right way, which is the reading the camber advice is made of and the
    -- one the panel could not show.
    if settings.showTyreEdges then
      text.tyreEdges[i] = string.format('%s|%s|%s',
        tempText(shown.tyre_temp_inner_c[i]):gsub('[^%d%-]', ''),
        tempText(shown.tyre_temp_c[i]):gsub('[^%d%-]', ''),
        tempText(shown.tyre_temp_outer_c[i]):gsub('[^%d%-]', ''))
    else
      text.tyreEdges[i] = ''
    end
    text.brakeTemp[i] = 'B: ' .. tempText(shown.brake_temp_c[i])
    text.wear[i] = string.format('Wear: %.0f%%', shown.tyre_wear_percent[i])
  end

  text.delta = string.format('%+.3f', shown.delta_seconds)
  text.best = lapTimeText(shown.best_lap_ms)
  text.last = lapTimeText(shown.last_lap_ms)
  text.current = lapTimeText(shown.current_lap_ms)

  text.fuel = volumeText(shown.fuel_litres)
  text.lapsLeft = shown.fuel_laps_remaining > 0
    and string.format('%.1f', shown.fuel_laps_remaining) or '--'
  text.perLap = shown.fuel_per_lap > 0 and volumeText(shown.fuel_per_lap) or '--'

  text.position = shown.position > 0 and string.format('P%d', shown.position) or '--'
  text.lap = tostring(shown.lap_count)
  text.conditions = string.format('AIR %s   ROAD %s   GRIP %.0f%%',
    tempText(shown.air_temp_c), tempText(shown.road_temp_c), shown.surface_grip * 100)
end

M.text = text
M.tempText = tempText
M.gearText = gearText
M.lapTimeText = lapTimeText
M.speedText = speedText
M.volumeText = volumeText
M.pressureText = pressureText

return M
