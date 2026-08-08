-- A console, with the same vocabulary as the LÖVE harness's.
--
-- Settings that would otherwise need a mouse and four tabs can be typed, and
-- the ones with no widget at all — like --dev-mode — have somewhere to live.

local store = require('acpe.settings')
local settings = store.values
local DEFAULTS = store.DEFAULTS
local SETTING_KEYS = store.KEYS
local theme = require('acpe.theme')
local i18n = require('acpe.i18n')
local layout = require('acpe.layout')
local frame = require('acpe.frame')
local format = require('acpe.format')

local COLOR = theme.COLOR
local ACCENTS = theme.ACCENTS
local PALETTE = theme.PALETTE
local tr = i18n.tr
local say = layout.say
local contentWidth = layout.contentWidth
local MESSAGE_SLOTS = frame.MESSAGE_SLOTS

local M = {}

local consoleInput = ''
local consoleLines = { 'type --help' }

-- Forty rather than twelve. "Dump settings to console" writes one line per
-- setting — seventy of them — into this buffer, so with twelve the only keys
-- that survived were the last twelve alphabetically and the button was, in
-- practice, a way to look at `wearBad`. The dump packs three to a line below,
-- which brings it inside this.
local CONSOLE_LINES = 40

local function consoleSay(line)
  consoleLines[#consoleLines + 1] = line
  while #consoleLines > CONSOLE_LINES do table.remove(consoleLines, 1) end
end

local COMMANDS = {
  ['--help'] = function()
    consoleSay('--scale N   --width N   --bar N   --backing N')
    consoleSay('--accent blue|teal|amber|violet|green')
    consoleSay('--vr on|off   --dev-mode   --units c|f  --psi|--bar-units')
    consoleSay('--lines 1..' .. MESSAGE_SLOTS .. '   --palette   --reset')
  end,
  ['--dev-mode'] = function()
    settings.devMode = not settings.devMode
    consoleSay('developer mode: ' .. (settings.devMode and 'on' or 'off'))
  end,
  ['--reset'] = function()
    for key, value in pairs(DEFAULTS) do settings[key] = value end
    consoleSay('settings reset')
  end,
}

local NUMERIC = {
  ['--scale'] = { 'fontScale', 0.5, 4 },
  ['--width'] = { 'contentWidth', 200, 1200 },
  ['--bar'] = { 'barHeight', 2, 40 },
  ['--backing'] = { 'background', 0, 1 },
  ['--lines'] = { 'engineerLines', 1, MESSAGE_SLOTS },
}

local function runCommand(line)
  local words = {}
  for word in tostring(line):gmatch('%S+') do words[#words + 1] = word end
  if #words == 0 then return end
  consoleSay('> ' .. line)

  local index = 1
  while index <= #words do
    local word = words[index]
    local numeric = NUMERIC[word]
    if COMMANDS[word] ~= nil then
      COMMANDS[word]()
    elseif numeric ~= nil then
      local value = tonumber(words[index + 1])
      if value ~= nil then
        settings[numeric[1]] = math.max(numeric[2], math.min(numeric[3], value))
        consoleSay(numeric[1] .. ' = ' .. tostring(settings[numeric[1]]))
        index = index + 1
      else
        consoleSay(word .. ' needs a number')
      end
    elseif word == '--accent' then
      local name = words[index + 1]
      if ACCENTS[name] ~= nil then
        settings.accent = name
        index = index + 1
      else
        consoleSay('accents: blue teal amber violet green')
      end
    elseif word == '--vr' then
      settings.vrMode = words[index + 1] ~= 'off'
      index = index + 1
    elseif word == '--units' then
      settings.celsius = words[index + 1] ~= 'f'
      index = index + 1
    elseif word == '--palette' then
      for _, entry in ipairs(PALETTE) do settings[entry[2]] = DEFAULTS[entry[2]] end
      consoleSay('palette reset')
    elseif word == '--psi' then
      settings.psi = true
    elseif word == '--bar-units' then
      settings.psi = false
    else
      consoleSay('unknown: ' .. word)
    end
    index = index + 1
  end

  -- The same two things every control in the settings window does after it
  -- changes something. The console changed settings and did neither: units
  -- typed here did not reach the drawn strings until the next frame arrived
  -- from the application — so with the feed stopped, `--units f` appeared to
  -- do nothing at all — and nothing was written until some other control
  -- happened to trigger a save.
  format.rebuild(frame.shown)
  store.save()
end

-- One-press commands, so the common ones do not have to be typed at all. The
-- console is for the ones with no widget; these are the ones worth reaching in
-- a pit lane.
local QUICK = {
  { '4K', '--scale 2 --width 680 --bar 12' },
  { '1080p', '--scale 1 --width 360 --bar 6' },
  { 'VR', '--vr on --scale 1.6' },
  { 'Bigger', '--scale 1.2' },
  { 'Smaller', '--scale 0.85' },
  { 'Dev', '--dev-mode' },
  { 'Reset', '--reset' },
}

local lastCommand = ''

local function drawConsoleBody()
  say('caption', tr('QUICK'), COLOR.label)
  for index, entry in ipairs(QUICK) do
    if ui.button(entry[1]) then runCommand(entry[2]) end
    if index % 4 ~= 0 and index < #QUICK then ui.sameLine() end
  end

  ui.separator()
  say('caption', tr('COMMAND'), COLOR.label)
  ui.setNextItemWidth(contentWidth())
  local typed, _, entered = ui.inputText('##acpeConsole', consoleInput)
  consoleInput = typed or consoleInput
  if entered then
    lastCommand = consoleInput
    runCommand(consoleInput)
    consoleInput = ''
  end

  if ui.button('Run') then
    lastCommand = consoleInput
    runCommand(consoleInput)
    consoleInput = ''
  end
  ui.sameLine()
  if ui.button('Again') and lastCommand ~= '' then runCommand(lastCommand) end
  ui.sameLine()
  if ui.button('Help') then runCommand('--help') end
  ui.sameLine()
  if ui.button('Clear') then consoleLines = {} end

  ui.separator()
  for _, line in ipairs(consoleLines) do
    local color = COLOR.dim
    if line:sub(1, 1) == '>' then
      color = COLOR.accent
    elseif line:match('^unknown') or line:match('needs a number') then
      color = COLOR.bad
    end
    say('caption', line, color)
  end
end

M.say = consoleSay
M.run = runCommand
M.draw = drawConsoleBody

return M
