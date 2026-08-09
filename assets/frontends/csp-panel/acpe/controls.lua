-- The widgets the settings window is built from.
--
-- Each one is bound to a settings key and saves when it moves. The box or grab
-- carries no label of its own: CSP draws widget text at its own font size,
-- which cannot be scaled, and on a 4K screen that is a settings window nobody
-- can read. The label is drawn beside it at the panel's size instead.

local store = require('acpe.settings')
local settings = store.values
local theme = require('acpe.theme')
local i18n = require('acpe.i18n')
local layout = require('acpe.layout')

local COLOR = theme.COLOR
local tr = i18n.tr
local say = layout.say
local contentWidth = layout.contentWidth
local saveSettings = store.save

local M = {}

--- A checkbox bound to a settings field. `ui.checkbox` reports the click, not
--- the new value, so the flip happens here.
---
--- Declared up here because every tab uses it, and a local declared after its
--- callers is a global to them — which is how the developer tab came out as a
--- heading with nothing underneath it.
local function settingToggle(label, key)
  label = tr(label)
  -- The box carries no label of its own: CSP draws widget text at its own font
  -- size, which cannot be scaled, and on a 4K screen that is a settings window
  -- nobody can read. The label is drawn beside it at the panel's size instead.
  if ui.checkbox('##' .. key, settings[key]) then
    settings[key] = not settings[key]
    saveSettings()
  end
  ui.sameLine()
  say('body', label, COLOR.text)
end

--- A slider bound to a settings field, labelled at the panel's size and saved
--- when it moves.
local function settingSlider(id, key, low, high, format, integer)
  ui.setNextItemWidth(contentWidth())
  local value, changed = ui.slider('##' .. id, settings[key], low, high, format, integer)
  if changed then
    settings[key] = value
    saveSettings()
  end
  return changed
end

--- A radio button with the same treatment.
local function settingRadio(label, id, selected)
  label = tr(label)
  local clicked = ui.radioButton('##' .. id, selected)
  ui.sameLine()
  say('body', label, selected and COLOR.text or COLOR.dim)
  return clicked
end

M.toggle = settingToggle
M.slider = settingSlider
M.radio = settingRadio

return M
