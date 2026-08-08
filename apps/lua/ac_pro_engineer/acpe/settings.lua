-- AC Pro Engineer overlay — persistent settings.
--
-- What the panel shows and in which units. These are read in the draw path, so
-- they are plain fields on a table that is written only when the settings
-- window is open — never computed per frame.
--
-- CSP keeps them across sessions through `ac.storage`. Everywhere else — the
-- LuaJIT harness before it stubs one, the LÖVE harness — the defaults simply
-- stand, which is why the storage call is guarded rather than assumed.
--
-- Everything else requires this module and takes `.values` as its local
-- `settings`, so every `settings.foo` in the panel reads the one table.

local persist = require('acpe.persist')

local M = {}

local DEFAULTS = {
  textSize = 'normal',   -- compact | normal | large
  vrMode = false,        -- bigger text, thicker bar, more air between blocks
  showHeader = true,
  showRpmBar = true,
  showTyres = true,
  showTiming = true,
  showFuel = true,
  showSession = true,
  showEngineer = true,
  showLimiter = true,
  shiftLight = true,
  shiftAt = 0.95,          -- share of the rev range where the bar goes red
  hudMode = false,         -- one line: speed, gear, delta, fuel
  showTyreTemp = true,
  showBrakeTemp = true,
  showWear = true,
  showPressureTarget = true,
  showDelta = true,
  showBest = true,
  showLast = true,
  showFuelLitres = true,
  showLapsLeft = true,
  showPerLap = true,
  showPosition = true,
  showLapNumber = true,
  showCurrentLap = true,
  showConditions = true,
  pressureDecimals = 1,
  tyreCold = 70, tyreHot = 95, tyreOver = 105,
  brakeCold = 150, brakeHot = 550, brakeOver = 750,
  wearWarn = 96, wearBad = 85,
  columnsPerRow = 0,       -- 0 follows the layout, 2 or 3 force it
  mainWidth = 320, mainHeight = 420,
  engineerWidth = 300, engineerHeight = 170,
  settingsWidth = 560, settingsHeight = 680,

  -- How the advice reads. The application decides how many lines it publishes;
  -- these decide how many of them are drawn and what they look like.
  engineerLines = 4,
  engineerBullet = 'severity',   -- severity | > | dot | none
  engineerWrap = true,
  engineerHighlight = true,
  -- A line that arrived a second ago and one that has been there four laps are
  -- the same three words in the same colour, and the eye stops going to them.
  engineerEmphasiseNew = true,

  sectionLabels = true,
  -- A newer panel sitting on disk is worth one line, and worth being
  -- able to turn off: a driver who cannot restart mid-session should not
  -- be told about it every lap.
  showUpdateNotice = true,
  devMode = false,
  contentWidth = 360,
  barHeight = 6,
  fontScale = 1.0,        -- everything the panel draws, in one number
  autoScale = true,       -- and it follows the window, so stretching grows it
  engineerScale = 1.0,    -- advice, sized on its own
  engineerMinSeverity = 0,-- 0 everything, 1 warnings up, 2 critical only
  engineerSpacing = false,
  engineerLineGap = 4,
  engineerSeparator = false,
  engineerBackground = 0.0,
  engineerShowCount = false,
  engineerMaxChars = 64,   -- a line longer than this is cut, not wrapped away
  engineerUppercase = false,
  engineerNumbered = false,
  engineerSeverityWord = false,
  showDebugBounds = false,
  freezeDisplay = false,
  devDemo = false,          -- plausible numbers with no application running
  devIgnoreFlags = false,   -- draw every section whatever the app asked for
  devIgnoreVersion = false, -- draw anyway when the versions disagree
  devSampleAdvice = false,  -- four lines, one of each severity, one very long
  background = 0.0,       -- panel backing, 0 for none
  accent = 'blue',
  colorText = '0.88,0.90,0.94',
  colorLabel = '0.62,0.66,0.72',
  colorDim = '0.45,0.48,0.52',
  colorGood = '0.35,0.85,0.45',
  colorWarn = '1.00,0.76,0.20',
  colorBad = '1.00,0.34,0.34',
  celsius = true,
  psi = true,
  mph = false,
  gallons = false,
  shortLapTimes = false,
  unitSuffix = true,
}

local settings = {}
local SETTING_KEYS = {}
for key, value in pairs(DEFAULTS) do
  settings[key] = value
  SETTING_KEYS[#SETTING_KEYS + 1] = key
end
table.sort(SETTING_KEYS)

local storageActive = false
local storageError = nil

--- CSP's storage, kept beside the settings rather than used as them.
---
--- `settings` used to *be* this proxy: `settings = stored`, and every read and
--- write in the panel went through its metatable. That is the bug behind "the
--- checkboxes do not stay where they are put". A proxy is free to accept an
--- assignment and do nothing with it, and on a build where it does, the write
--- never lands anywhere at all — not on disk and not even in memory, because
--- there was no memory, only the proxy. Reading the value back gave whatever
--- the proxy felt like returning, so the box drew itself unticked again on the
--- very next frame and there was nothing to inspect.
---
--- Now the panel owns a plain table, storage is read once into it and written
--- on save, and a storage that forgets costs the *file* nothing. It is also
--- cheaper: these are read every frame in the draw path, and a plain field is
--- not a metatable call.
local storage = nil

--- Ask CSP for persistent storage, both ways it is spelled.
---
--- The prefixed form is what the documentation shows; the bare form is what
--- older builds take. Trying one and giving up is how the panel ended up
--- announcing that settings last for this session only.
if type(ac) == 'table' and type(ac.storage) == 'function' then
  local ok, stored = pcall(ac.storage, DEFAULTS, 'acpe.')
  if not ok or stored == nil then
    storageError = tostring(stored)
    ok, stored = pcall(ac.storage, DEFAULTS)
  end
  if ok and stored ~= nil then
    storage = stored
    storageActive = true
    storageError = nil
    -- Read out, once. Anything storage cannot answer for keeps its default.
    for _, key in ipairs(SETTING_KEYS) do
      local ok2, value = pcall(function() return storage[key] end)
      if ok2 and value ~= nil and type(value) == type(DEFAULTS[key]) then
        settings[key] = value
      end
    end
  elseif storageError == nil then
    storageError = tostring(stored)
  end
end

--- The panel's own file, applied over whatever storage had.
---
--- It wins, and deliberately: the file is only ever written by a change the
--- driver made, so when the two disagree the file is the one that reflects
--- what they chose. Storage that quietly forgot everything hands back the
--- defaults, and defaults must not overwrite a real setting.
---
--- Values are checked against the defaults' types, so a file edited by hand
--- into nonsense costs one setting rather than the panel.
local fileValues = persist.load()
if fileValues ~= nil then
  for _, key in ipairs(SETTING_KEYS) do
    local value = fileValues[key]
    if value ~= nil and type(value) == type(DEFAULTS[key]) then
      settings[key] = value
    end
  end
end

--- What was last written, so a save can tell what actually changed.
---
--- Seeded from the values that survived both sources, so the first frame of
--- the settings window does not rewrite all seventy keys. Seeded whether or
--- not storage is active, because the file is written either way.
local lastSaved = {}
for _, key in ipairs(SETTING_KEYS) do lastSaved[key] = settings[key] end

--- How the last save went, for the line under the Save button.
local saveReport = { written = 0, failed = 0, ever = false }

--- Write back everything that has changed since the last write.
---
--- Storage persists on assignment, so a value reaches the disk when something
--- assigns it — and only then. This used to assign every key to *itself*,
--- which works only if the proxy writes a value it already holds; nothing on
--- this side can check that it does, and nothing ever did. Assigning only the
--- keys that differ is a real change however the proxy treats equal ones, and
--- reading each one back afterwards is the only proof available that it stuck.
---
--- Called from every control and once at the end of the settings frame, so a
--- value set directly — accents, palette entries, presets — is caught even
--- though nothing wired it to a save.
local fileError = nil

local function saveSettings()
  local written, failed, changed = 0, 0, false
  for _, key in ipairs(SETTING_KEYS) do
    local value = settings[key]
    if lastSaved[key] ~= value then
      changed = true
      if storageActive then
        -- Best effort. Whether it stuck is checked, but a storage that lies
        -- about it no longer costs the setting: the file below is the copy
        -- that has to survive.
        local ok = pcall(function() storage[key] = value end)
        if not ok then failed = failed + 1 end
      end
      lastSaved[key] = value
      written = written + 1
    end
  end

  -- The file, whenever anything moved, and regardless of what storage did.
  -- This is the copy that survives a CSP build whose storage does not.
  if changed then
    local path, why = persist.save(settings, SETTING_KEYS)
    fileError = path == nil and why or nil
    if path == nil then failed = failed + 1 end
  end

  if written > 0 or failed > 0 then
    saveReport.written, saveReport.failed, saveReport.ever = written, failed, true
  end
  return written, failed
end

--- Write everything, whether or not it looks changed.
---
--- What the Save button means. A driver presses it because they do not trust
--- that it saved, and answering "nothing needed writing" is not an answer.
local function saveEverySetting()
  lastSaved = {}
  local written, failed = saveSettings()
  saveReport.written, saveReport.failed, saveReport.ever = written, failed, true
  return written, failed
end

-- Anything but a string here is a palette saved by an older build, or one that
-- came back from storage as something the panel cannot read. Either way the
-- default is the only safe answer: a palette that throws takes every window
-- down with it, and the way back would be a button nobody can see.
for _, key in ipairs({ 'colorText', 'colorLabel', 'colorDim', 'colorGood', 'colorWarn',
    'colorBad' }) do
  if type(settings[key]) ~= 'string' then settings[key] = DEFAULTS[key] end
end
M.DEFAULTS = DEFAULTS
M.values = settings
M.KEYS = SETTING_KEYS
M.save = saveSettings
M.saveAll = saveEverySetting
M.report = saveReport

--- Whether CSP gave us somewhere to write, and why not if it did not.
function M.storage()
  return storageActive, storageError
end

--- The file the settings are kept in, and the last reason a write failed.
---
--- Shown in the Units tab, because "where did it save" is the question behind
--- "did it save", and until now the panel could not answer either.
function M.file()
  return persist.path(), fileError
end

return M
