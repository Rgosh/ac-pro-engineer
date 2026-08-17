-- Harness settings: defaults, the command line that overrides them, and the
-- file they are remembered in.
--
-- Every setting is reachable from three places — the Settings tab, a flag on
-- `run.sh`, and the saved file — and they resolve in that order of authority
-- for the current run: saved values load first, flags override them, and
-- anything changed in the UI is written back.

local config = {}

local SAVE_FILE = 'harness.lua'

config.defaults = {
  source = 'sim',            -- sim | shm | manual
  shmPath = '/dev/shm/AcTools.CSP.Limited.ACPE.v1',
  simSpeed = 1.0,            -- how fast the self-driving lap runs
  paused = false,
  uiScale = 1.0,             -- font scale inside the emulated CSP window
  panelWidth = 300,          -- the overlay window, matching manifest SIZE
  panelHeight = 380,
  background = 'dark',       -- dark | checker | green
  showBounds = false,        -- outline the panel's content rectangle
  showFps = true,
  settingsOpen = false,      -- the app's settings window, as the gear opens it
  engineerOpen = true,       -- the advice window, its own window in game too
  telemetryOpen = false,     -- the raw-frame window, also its own in game
  statusOpen = false,        -- the link window, the fifth one the manifest declares

  -- Where each window sits. Flat keys because the saved file is a table of
  -- scalars, and a window position is the one setting nobody wants to retype
  -- after every run.
  mainX = 24, mainY = 46,
  engineerX = 24, engineerY = 440,
  telemetryX = 336, telemetryY = 46,
  settingsX = 336, settingsY = 46,
  statusX = 336, statusY = 580,

  -- Sizes too: in game the driver drags a window's corner and CSP remembers
  -- it, so the harness has to be able to answer "does it still read at that
  -- size" without a rebuild.
  engineerWidth = 280, engineerHeight = 150,
  telemetryWidth = 340, telemetryHeight = 480,
  settingsWidth = 420, settingsHeight = 520,
  statusWidth = 340, statusHeight = 300,
  tab = 'Telemetry',
  devMode = false,           -- unlocks the simulation controls and the console
}

local function copy(t)
  local out = {}
  for k, v in pairs(t) do out[k] = v end
  return out
end

config.values = copy(config.defaults)

function config.load()
  local ok, chunk = pcall(love.filesystem.load, SAVE_FILE)
  if not ok or chunk == nil then return end
  local good, saved = pcall(chunk)
  if not good or type(saved) ~= 'table' then return end
  for k, v in pairs(saved) do
    if config.defaults[k] ~= nil and type(v) == type(config.defaults[k]) then
      config.values[k] = v
    end
  end
end

function config.save()
  local out = { '-- Written by the Pro Engineer LÖVE harness.', 'return {' }
  for k, v in pairs(config.values) do
    if type(v) == 'string' then
      out[#out + 1] = string.format('  [%q] = %q,', k, v)
    else
      out[#out + 1] = string.format('  [%q] = %s,', k, tostring(v))
    end
  end
  out[#out + 1] = '}'
  love.filesystem.write(SAVE_FILE, table.concat(out, '\n'))
end

function config.reset()
  config.values = copy(config.defaults)
  config.save()
end

config.usage = [[
Pro Engineer overlay harness

  run.sh [options]

Options:
  --source sim|shm|manual   where telemetry comes from (default: sim)
  --frame PATH              shared-memory file for --source shm
  --speed N                 simulation rate, 0.25 to 4 (default: 1)
  --scale N                 font scale inside the overlay window
  --size WxH                overlay window size (default: 300x380)
  --background dark|checker|green
  --paused                  start with the simulation stopped
  --bounds                  outline the overlay's content rectangle
  --settings                start with the app's settings window open
  --status                  start with the link window open
  --no-engineer             start with the advice window closed
  --tab NAME                open on Telemetry, App settings, Harness, Dev or Log
  --dev-mode                unlock the simulation controls and the console
  --no-dev-mode             lock them again
  --reset                   discard saved settings and start from defaults
  --test                    run headless for a few seconds, then exit
  --shot NAME               save a screenshot into the save directory and exit
  --portrait ID             draw one window alone, cropped to it, for --shot:
                            main, engineer, telemetry, status or settings
  --app-tab PATH            open the app on a tab, e.g. Look/Colour
  --app-dev                 turn the panel's own developer mode on
  --app-stopped             freeze the feed, as a closed application looks
  --help                    this text

Settings changed in the window are saved and reused next time; a flag wins for
the run it is given on. Every flag above can also be typed into the console on
the Advanced tab while the harness is running — it takes effect immediately and
nothing needs restarting.
]]

--- Apply command-line arguments over the loaded settings.
-- Returns `false, message` for `--help`, so the caller can print and quit.
function config.applyArguments(args)
  local i = 1
  local testMode = false

  local function next_()
    i = i + 1
    return args[i]
  end

  while i <= #args do
    local a = args[i]
    if a == '--help' or a == '-h' then
      return false, config.usage
    elseif a == '--source' then
      local v = next_()
      if v == 'sim' or v == 'shm' or v == 'manual' then config.values.source = v end
    elseif a == '--frame' then
      config.values.shmPath = next_() or config.values.shmPath
      config.values.source = 'shm'
    elseif a == '--speed' then
      config.values.simSpeed = tonumber(next_()) or config.values.simSpeed
    elseif a == '--scale' then
      config.values.uiScale = tonumber(next_()) or config.values.uiScale
    elseif a == '--size' then
      local v = next_() or ''
      local w, h = v:match('^(%d+)[xX](%d+)$')
      if w then
        config.values.panelWidth = tonumber(w)
        config.values.panelHeight = tonumber(h)
        -- Kept separately so `--portrait` can apply it to whichever window it
        -- is drawing, rather than only ever to the main panel.
        config.sizeOverride = { tonumber(w), tonumber(h) }
      end
    elseif a == '--background' then
      local v = next_()
      if v == 'dark' or v == 'checker' or v == 'green' then config.values.background = v end
    elseif a == '--tab' then
      config.values.tab = next_() or config.values.tab
    elseif a == '--paused' then
      config.values.paused = true
    elseif a == '--bounds' then
      config.values.showBounds = true
    elseif a == '--settings' then
      config.values.settingsOpen = true
    elseif a == '--status' then
      config.values.statusOpen = true
    elseif a == '--no-engineer' then
      config.values.engineerOpen = false
    elseif a == '--dev-mode' then
      config.values.devMode = true
    elseif a == '--no-dev-mode' then
      config.values.devMode = false
    elseif a == '--reset' then
      config.values = copy(config.defaults)
    elseif a == '--test' then
      testMode = true
    elseif a == '--shot' then
      config.shot = next_() or 'harness.png'
    -- The three below are for generating the README's pictures, so they are
    -- deliberately not saved: a run that took a portrait should not leave the
    -- next one drawing one window on a black square.
    elseif a == '--portrait' then
      config.portrait = next_()
    elseif a == '--app-tab' then
      config.appTab = next_()
    elseif a == '--app-dev' then
      config.appDev = true
    elseif a == '--app-stopped' then
      config.appStopped = true
    end
    i = i + 1
  end

  return true, nil, testMode
end

return config
