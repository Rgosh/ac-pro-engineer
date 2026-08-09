-- Colours, and the two ways the driver changes them: an accent preset and a
-- palette they can edit.
--
-- Preallocated. `rgbm()` allocates and the draw path must not, so every colour
-- here is a table written in place when the palette changes, never replaced —
-- a reference taken once stays correct for the life of the script.

local store = require('acpe.settings')
local settings = store.values
local DEFAULTS = store.DEFAULTS

local M = {}

-- Preallocated constants
local COLOR = {
  dim       = rgbm(0.45, 0.48, 0.52, 1),
  label     = rgbm(0.62, 0.66, 0.72, 1),
  text      = rgbm(0.88, 0.90, 0.94, 1),
  accent    = rgbm(0.20, 0.72, 1.00, 1),
  good      = rgbm(0.35, 0.85, 0.45, 1),
  warn      = rgbm(1.00, 0.76, 0.20, 1),
  bad       = rgbm(1.00, 0.34, 0.34, 1),
  cold      = rgbm(0.38, 0.60, 1.00, 1),
  purple    = rgbm(0.76, 0.44, 1.00, 1),
  barBack   = rgbm(0.16, 0.17, 0.20, 1),
  panel     = rgbm(0.10, 0.11, 0.13, 0.55),
  limiter   = rgbm(1.00, 0.62, 0.10, 1),
}

-- CSP's theme is whatever the driver picked for CSP — red accents, in the
-- default one — and it repainted every tab, checkbox and slider in this panel.
-- The colours below are the harness's, so the thing being designed and the
-- thing being driven are the same colour.
-- Accent presets. The panel is looked at through a windscreen, so this is not
-- decoration: a colour that disappears against the track is a readout nobody
-- reads.
local ACCENTS = {
  blue   = rgbm(0.20, 0.72, 1.00, 1),
  teal   = rgbm(0.20, 0.85, 0.75, 1),
  amber  = rgbm(1.00, 0.72, 0.20, 1),
  violet = rgbm(0.76, 0.44, 1.00, 1),
  green  = rgbm(0.40, 0.90, 0.45, 1),
}
local ACCENT_NAMES = { 'blue', 'teal', 'amber', 'violet', 'green' }

-- Starting points by screen, so the first thing a driver sees is legible.
-- The third field keeps two buttons on a line.
-- The windows the manifest declares, and the sizes a script can pin them to.
local PRESETS = {
  { '1080p', { fontScale = 1.0, contentWidth = 360, barHeight = 6, textSize = 'normal' }, true },
  { '1440p', { fontScale = 1.35, contentWidth = 460, barHeight = 8, textSize = 'normal' }, false },
  { '4K', { fontScale = 2.0, contentWidth = 680, barHeight = 12, textSize = 'normal' }, true },
  { 'VR', { fontScale = 1.6, contentWidth = 520, barHeight = 14, vrMode = true }, false },
}

local function accentColor()
  return ACCENTS[settings.accent] or ACCENTS.blue
end

local STYLE_COLORS = {
  { 'Tab', rgbm(0.13, 0.16, 0.20, 1) },
  { 'TabHovered', rgbm(0.20, 0.42, 0.60, 1) },
  { 'TabActive', rgbm(0.16, 0.34, 0.50, 1) },
  { 'Header', rgbm(0.16, 0.34, 0.50, 0.8) },
  { 'HeaderHovered', rgbm(0.20, 0.48, 0.70, 0.9) },
  { 'HeaderActive', rgbm(0.20, 0.55, 0.80, 1) },
  { 'CheckMark', rgbm(0.20, 0.72, 1.00, 1) },
  { 'SliderGrab', rgbm(0.20, 0.60, 0.90, 1) },
  { 'SliderGrabActive', rgbm(0.20, 0.72, 1.00, 1) },
  { 'Button', rgbm(0.16, 0.17, 0.21, 1) },
  { 'ButtonHovered', rgbm(0.22, 0.40, 0.56, 1) },
  { 'ButtonActive', rgbm(0.20, 0.55, 0.80, 1) },
  { 'FrameBg', rgbm(0.16, 0.17, 0.21, 1) },
  { 'FrameBgHovered', rgbm(0.22, 0.24, 0.29, 1) },
  { 'FrameBgActive', rgbm(0.20, 0.42, 0.60, 1) },
  { 'Separator', rgbm(1.00, 1.00, 1.00, 0.10) },
}

-- The palette is settings, not constants: a colour that works on a grey wall
-- at Nordschleife is not the one that works against Bahrain at noon.
local PALETTE = {
  { 'text', 'colorText' },
  { 'label', 'colorLabel' },
  { 'dim', 'colorDim' },
  { 'good', 'colorGood' },
  { 'warn', 'colorWarn' },
  { 'bad', 'colorBad' },
}

--- Copy the chosen colours into the preallocated table the draw path uses.
--- Values, not references: `rgbm()` allocates, and the draw path must not.
-- No colour may be picked into invisibility. A settings screen that can hide
-- itself is a trap: the way back out is a button nobody can see.
local MIN_ALPHA = 0.35

--- Read "r,g,b" into an existing colour. No allocation, no new table, and a
--- malformed value leaves the colour alone rather than blanking it.
local function readColorInto(text, target)
  if type(text) ~= 'string' or target == nil then return end
  local r, g, b = text:match('^%s*([%d%.]+)%s*,%s*([%d%.]+)%s*,%s*([%d%.]+)%s*$')
  if r == nil then return end
  target.r, target.g, target.b = tonumber(r), tonumber(g), tonumber(b)
  target.mult = 1
end

local appliedPalette = {}
local paletteSelection = 1

local function applyPalette()
  for _, entry in ipairs(PALETTE) do
    local stored = settings[entry[2]]
    if appliedPalette[entry[1]] ~= stored then
      readColorInto(stored, COLOR[entry[1]])
      appliedPalette[entry[1]] = stored
    end
  end
  local accent = accentColor()
  COLOR.accent.r, COLOR.accent.g = accent.r, accent.g
  COLOR.accent.b, COLOR.accent.mult = accent.b, accent.mult
end

M.COLOR = COLOR
M.ACCENTS = ACCENTS
M.ACCENT_NAMES = ACCENT_NAMES
M.PALETTE = PALETTE
M.PRESETS = PRESETS
M.STYLE_COLORS = STYLE_COLORS
M.accentColor = accentColor
M.apply = applyPalette

return M
