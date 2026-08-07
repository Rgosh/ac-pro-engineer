-- What is only worth seeing while working on the panel.
--
-- The numbers behind the numbers, and the switches that make the panel lie on
-- purpose. Reached from the red Dev tab in the settings window, which only
-- appears once developer mode is on.

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
local text = format.text
local shown = frame.shown
local hasFlag = frame.hasFlag
local store = require('acpe.settings')
local SETTING_KEYS = store.KEYS
local controls = require('acpe.controls')
local console = require('acpe.console')

local settingToggle = controls.toggle

local M = {}

local function drawDevBody()
  say('caption', tr('DRAW WITHOUT A SESSION'), COLOR.label)
  settingToggle('Demo numbers', 'devDemo')
  settingToggle('Sample advice, all severities', 'devSampleAdvice')
  settingToggle('Ignore what the app asked for', 'devIgnoreFlags')
  settingToggle('Ignore version mismatch', 'devIgnoreVersion')

  ui.separator()
  say('caption', tr('INSPECT'), COLOR.label)
  settingToggle('Freeze the display', 'freezeDisplay')
  settingToggle('Outline the content', 'showDebugBounds')

  ui.separator()
  if ui.button('Everything on') then
    for _, key in ipairs(SETTING_KEYS) do
      if key:match('^show') and type(settings[key]) == 'boolean' then
        settings[key] = true
      end
    end
    settings.devIgnoreFlags = true
  end
  ui.sameLine()
  if ui.button('Leave developer mode') then
    settings.devMode = false
    settings.devDemo = false
    settings.devSampleAdvice = false
    settings.devIgnoreFlags = false
    settings.freezeDisplay = false
    settings.showDebugBounds = false
  end

  ui.separator()
  say('caption', tr('FRAME'), COLOR.label)
  row('sequence', tostring(shown.sequence))
  row('since change', string.format('%.2f s', frame.secondsSinceChange()))
  row('flags', string.format('0x%02X', shown.flags))
  row('frame version', string.format('%d / %d', shown.version, frame.expectedVersion()))
  row('panel version', frame.panelVersion())

  say('caption', tr('LAYOUT'), COLOR.label)
  row('window scale', string.format('%.2fx', layout.windowScale()))
  row('content width', string.format('%.0f px', contentWidth()))
  row('body text', string.format('%.1f px', layout.textSize('body')))
  row('advice text', string.format('%.1f px',
    layout.textSize('body') * (settings.engineerScale or 1)))

  ui.separator()
  if ui.button('Dump settings to console') then
    -- Three to a line: one per line is seventy lines into a buffer that keeps
    -- forty, so the first two thirds of the alphabet scrolled away unread.
    local group = {}
    for _, key in ipairs(SETTING_KEYS) do
      group[#group + 1] = key .. '=' .. tostring(settings[key])
      if #group == 3 then
        console.say(table.concat(group, '   '))
        group = {}
      end
    end
    if #group > 0 then console.say(table.concat(group, '   ')) end
  end
end

M.body = drawDevBody

return M
