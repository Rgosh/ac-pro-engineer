-- Where the harness gets its telemetry.
--
-- Three sources, all producing the same table the overlay reads through
-- `ac.readMemoryMappedFile`:
--
--   sim     a lap that drives itself, so the panel can be judged in motion
--   shm     the real `OverlayFrame` the desktop application publishes
--   manual  whatever the sliders in the Telemetry tab are set to
--
-- The frame is allocated once and overwritten in place, for the same reason
-- the overlay does it: this is the code path that would otherwise produce
-- garbage sixty times a second.

local ffi = require('ffi')

local sim = {}

-- Must match ac_core::overlay::frame::OverlayFrame.
ffi.cdef [[
typedef struct {
  uint32_t version, sequence;
  float speed_kmh, fuel_litres, fuel_laps_remaining, fuel_per_lap;
  float delta_seconds, air_temp_c, road_temp_c, surface_grip;
  float tyre_pressure_psi[4], tyre_temp_c[4], tyre_wear_percent[4], brake_temp_c[4];
  int32_t rpm, max_rpm, gear, lap_count, last_lap_ms, best_lap_ms, current_lap_ms, position;
  uint32_t flags, message_count;
  char messages[4][64];
} AcpeFrame;
]]

-- Must match ac_core::overlay::frame::OVERLAY_MMF_NAME.
sim.MMF_NAME = 'AcTools.CSP.Limited.ACPE.v1'
sim.SHM_PATH = '/dev/shm/' .. sim.MMF_NAME

local FLAG = {
  PIT_LIMITER = 1,
  CONNECTED = 2,
  SHOW_TELEMETRY = 4,
  SHOW_ENGINEER = 8,
  FUEL_WARNING = 16,
}

sim.FLAG = FLAG

--- The frame the overlay reads. Arrays are zero-based because that is how the
--- app indexes them — it speaks the struct's dialect, not Lua's.
local frame = {
  version = 1,
  sequence = 2,
  speed_kmh = 0,
  fuel_litres = 45,
  fuel_laps_remaining = 12,
  fuel_per_lap = 3.2,
  delta_seconds = 0,
  air_temp_c = 22,
  road_temp_c = 31,
  surface_grip = 0.98,
  rpm = 0,
  max_rpm = 8500,
  gear = 1,
  lap_count = 3,
  last_lap_ms = 92450,
  best_lap_ms = 91380,
  current_lap_ms = 0,
  position = 4,
  flags = FLAG.CONNECTED + FLAG.SHOW_TELEMETRY + FLAG.SHOW_ENGINEER,
  message_count = 2,
  tyre_pressure_psi = { [0] = 27.4, 27.6, 26.9, 27.1 },
  tyre_temp_c = { [0] = 82, 84, 88, 90 },
  tyre_wear_percent = { [0] = 98, 97, 95, 94 },
  brake_temp_c = { [0] = 420, 430, 380, 375 },
  messages = { [0] = 'Fuel is fine for the stint', 'Rear tyres are going off', '', '' },
}

sim.frame = frame

--- Advice lines, so the engineer section has something to cycle through.
local ADVICE = {
  'Fuel is fine for the stint',
  'Rear tyres are going off',
  'Brakes running cool, use them harder',
  'Front-left pressure is 0.4 psi low',
  'Box this lap',
  'Traffic ahead in sector 2',
}

local t = 0
local lapTime = 0

--- One tick of the self-driving lap.
local function advanceSimulation(dt, speedFactor)
  dt = dt * (speedFactor or 1)
  t = t + dt
  lapTime = lapTime + dt

  -- Speed and revs: a corner-straight-corner rhythm rather than a sine, so the
  -- gear indicator and the redline colours both get exercised.
  local phase = (t % 12) / 12
  local throttle = phase < 0.55 and (phase / 0.55) or (1 - (phase - 0.55) / 0.45)
  frame.speed_kmh = 60 + throttle * 210
  frame.gear = math.max(1, math.min(6, math.floor(frame.speed_kmh / 45) + 1))
  local ratio = 0.35 + throttle * 0.68
  frame.rpm = math.floor(math.min(frame.max_rpm * 1.0, frame.max_rpm * ratio))

  -- Tyres and brakes drift with load; wear only ever goes one way.
  for i = 0, 3 do
    local load = (i >= 2) and (throttle * 1.15) or throttle
    frame.tyre_temp_c[i] = 70 + load * 38 + math.sin(t * 0.7 + i) * 3
    frame.tyre_pressure_psi[i] = 26.5 + (frame.tyre_temp_c[i] - 80) * 0.03
    frame.tyre_wear_percent[i] = math.max(0, frame.tyre_wear_percent[i] - dt * (0.02 + i * 0.004))
    frame.brake_temp_c[i] = 250 + (1 - throttle) * 520 + math.sin(t * 1.3 + i) * 25
  end

  -- Fuel, and the warning that follows from it.
  frame.fuel_litres = math.max(0, frame.fuel_litres - dt * 0.06)
  frame.fuel_per_lap = 3.2
  frame.fuel_laps_remaining = frame.fuel_litres / frame.fuel_per_lap
  if frame.fuel_laps_remaining < 3 then
    frame.flags = bit.bor(frame.flags, FLAG.FUEL_WARNING)
  else
    frame.flags = bit.band(frame.flags, bit.bnot(FLAG.FUEL_WARNING))
  end

  -- Pit limiter whenever the car is crawling: the flag has to be seen to be
  -- trusted, and this is the only place it can be.
  if frame.speed_kmh < 80 then
    frame.flags = bit.bor(frame.flags, FLAG.PIT_LIMITER)
  else
    frame.flags = bit.band(frame.flags, bit.bnot(FLAG.PIT_LIMITER))
  end

  frame.delta_seconds = math.sin(t * 0.35) * 0.8
  frame.current_lap_ms = math.floor(lapTime * 1000)

  if lapTime > 92 then
    lapTime = 0
    frame.lap_count = frame.lap_count + 1
    frame.last_lap_ms = 90000 + math.floor(math.random() * 4000)
    if frame.last_lap_ms < frame.best_lap_ms then frame.best_lap_ms = frame.last_lap_ms end
    -- Rotate the advice so the engineer block changes over a session.
    local first = math.floor(t / 12) % #ADVICE
    frame.messages[0] = ADVICE[first + 1]
    frame.messages[1] = ADVICE[(first + 2) % #ADVICE + 1]
    frame.message_count = 2
  end

  -- The writer bumps the sequence by two per update; the overlay uses that to
  -- decide the frame is settled, and its standing still is how it notices the
  -- application is gone.
  frame.sequence = frame.sequence + 2
end

-- ---------------------------------------------------------------------------
-- Shared memory
-- ---------------------------------------------------------------------------

local buffer = ffi.new('AcpeFrame[1]')
local structSize = ffi.sizeof('AcpeFrame')

sim.shmError = nil

--- Copy the published frame in, if it is there. Failure is reported once and
--- then left alone: the overlay's own liveness timeout handles the rest.
local function readSharedMemory(path)
  local fh = io.open(path, 'rb')
  if fh == nil then
    sim.shmError = 'not published: ' .. path
    return false
  end

  local data = fh:read('*a')
  fh:close()
  if data == nil or #data < structSize then
    sim.shmError = string.format('short read: %d of %d bytes', data and #data or 0, structSize)
    return false
  end

  ffi.copy(buffer, data, structSize)
  local raw = buffer[0]

  frame.version = raw.version
  frame.sequence = raw.sequence
  frame.speed_kmh = raw.speed_kmh
  frame.fuel_litres = raw.fuel_litres
  frame.fuel_laps_remaining = raw.fuel_laps_remaining
  frame.fuel_per_lap = raw.fuel_per_lap
  frame.delta_seconds = raw.delta_seconds
  frame.air_temp_c = raw.air_temp_c
  frame.road_temp_c = raw.road_temp_c
  frame.surface_grip = raw.surface_grip
  frame.rpm = raw.rpm
  frame.max_rpm = raw.max_rpm
  frame.gear = raw.gear
  frame.lap_count = raw.lap_count
  frame.last_lap_ms = raw.last_lap_ms
  frame.best_lap_ms = raw.best_lap_ms
  frame.current_lap_ms = raw.current_lap_ms
  frame.position = raw.position
  frame.flags = raw.flags
  frame.message_count = raw.message_count

  for i = 0, 3 do
    frame.tyre_pressure_psi[i] = raw.tyre_pressure_psi[i]
    frame.tyre_temp_c[i] = raw.tyre_temp_c[i]
    frame.tyre_wear_percent[i] = raw.tyre_wear_percent[i]
    frame.brake_temp_c[i] = raw.brake_temp_c[i]
    frame.messages[i] = ffi.string(raw.messages[i])
  end

  sim.shmError = nil
  return true
end

--- Advance whichever source is selected. `manual` deliberately does nothing:
--- the sliders already wrote straight into the frame.
function sim.update(dt, source, options)
  options = options or {}
  if source == 'sim' then
    advanceSimulation(dt, options.speed or 1)
  elseif source == 'shm' then
    readSharedMemory(options.path or sim.SHM_PATH)
  elseif source == 'manual' then
    frame.sequence = frame.sequence + 2
  end
end

--- Fields the Telemetry tab offers as sliders: label, key, min, max, format.
sim.controls = {
  { 'Speed', 'speed_kmh', 0, 320, '%.0f km/h' },
  { 'RPM', 'rpm', 0, 9000, '%.0f', true },
  { 'Max RPM', 'max_rpm', 4000, 12000, '%.0f', true },
  { 'Gear', 'gear', -1, 7, '%.0f', true },
  { 'Fuel', 'fuel_litres', 0, 100, '%.1f L' },
  { 'Laps left', 'fuel_laps_remaining', 0, 40, '%.1f' },
  { 'Per lap', 'fuel_per_lap', 0, 8, '%.2f L' },
  { 'Delta', 'delta_seconds', -3, 3, '%+.3f s' },
}

--- Per-corner fields, driven as a set of four.
sim.cornerControls = {
  { 'Pressure', 'tyre_pressure_psi', 15, 40, '%.1f psi' },
  { 'Tyre temp', 'tyre_temp_c', 20, 140, '%.0f C' },
  { 'Wear', 'tyre_wear_percent', 0, 100, '%.0f %%' },
  { 'Brake temp', 'brake_temp_c', 0, 1000, '%.0f C' },
}

function sim.setFlag(flag, on)
  if on then
    frame.flags = bit.bor(frame.flags, flag)
  else
    frame.flags = bit.band(frame.flags, bit.bnot(flag))
  end
end

function sim.hasFlag(flag)
  return bit.band(frame.flags, flag) ~= 0
end

--- Freeze the sequence, which is exactly what a killed application looks like
--- from the overlay's side — the fastest way to check the idle state.
function sim.freeze() frame.sequence = frame.sequence end

return sim
