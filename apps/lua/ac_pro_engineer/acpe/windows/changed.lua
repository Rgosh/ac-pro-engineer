-- What the driver changed, and the way back.
--
-- Eighty-five settings across six tabs. Finding the one you moved last week is
-- a hunt, and finding the one that is making the panel look wrong is a worse
-- hunt, because you are looking for something you do not remember doing.
--
-- So: one list of everything that differs from the defaults, with what it is
-- now and what it was, a search box, and a reset beside each line and one for
-- the lot. The names are the keys rather than the captions on purpose — they
-- are what `ac_pro_engineer_overlay.lua` uses, so this list and that file are
-- the same vocabulary, and "what do I edit" has one answer.

local store = require('acpe.settings')
local settings = store.values
local DEFAULTS = store.DEFAULTS
local SETTING_KEYS = store.KEYS
local theme = require('acpe.theme')
local i18n = require('acpe.i18n')
local layout = require('acpe.layout')

local COLOR = theme.COLOR
local tr = i18n.tr
local say = layout.say
local contentWidth = layout.contentWidth

local M = {}

local search = ''

--- Every key whose value is not the one it ships with.
---
--- Sorted, because `SETTING_KEYS` is, and a list that reorders itself as you
--- change things is a list you cannot look away from and back to.
local function changedKeys()
  local keys = {}
  for _, key in ipairs(SETTING_KEYS) do
    if settings[key] ~= DEFAULTS[key] then keys[#keys + 1] = key end
  end
  return keys
end

--- How many, for the strip above the tabs.
function M.count()
  return #changedKeys()
end

local function shown(value)
  if type(value) == 'boolean' then return value and 'on' or 'off' end
  if type(value) == 'number' then
    -- Integers without a trailing `.0`, everything else to two places: these
    -- are read side by side and a column of `1.000000` reads as noise.
    if value == math.floor(value) then return string.format('%d', value) end
    return string.format('%.2f', value)
  end
  return tostring(value)
end

function M.body()
  local keys = changedKeys()

  if #keys == 0 then
    say('body', tr('Everything is as it ships.'), COLOR.dim)
    say('caption', tr('Anything you change appears here, with a way back.'), COLOR.dim)
    return
  end

  -- `ui.inputText`, not `inputTextBox`: the latter is the LÖVE harness's own
  -- invention and CSP has no such function, which
  -- `the_overlay_app_only_calls_ui_functions_csp_provides` catches — it would
  -- have been a nil call in the game and an error where the list should be.
  ui.setNextItemWidth(contentWidth())
  local typed = ui.inputText('##acpeChangedSearch', search)
  search = typed or search

  local needle = search:lower()
  local matched = 0

  for _, key in ipairs(keys) do
    if needle == '' or key:lower():find(needle, 1, true) ~= nil then
      matched = matched + 1
      -- The reset first, so the buttons line up down the left edge and the
      -- eye can run down them without reading the names.
      if ui.button(tr('reset') .. '##acpeReset' .. key) then
        settings[key] = DEFAULTS[key]
        store.save()
      end
      ui.sameLine()
      say('body', key, COLOR.text)
      ui.sameLine(contentWidth() * 0.62)
      say('body', shown(settings[key]), COLOR.good)
      ui.sameLine(contentWidth() * 0.82)
      say('caption', tr('was') .. ' ' .. shown(DEFAULTS[key]), COLOR.dim)
    end
  end

  if matched == 0 then
    say('caption', string.format(tr('nothing matching "%s" among the %d changed'),
      search, #keys), COLOR.dim)
  end

  ui.separator()
  if ui.button(tr('Reset everything') .. '##acpeResetAll') then
    for key, value in pairs(DEFAULTS) do settings[key] = value end
    store.save()
  end
  ui.sameLine()
  say('caption', string.format(tr('%d of %d settings changed'),
    #keys, #SETTING_KEYS), COLOR.dim)
end

return M
