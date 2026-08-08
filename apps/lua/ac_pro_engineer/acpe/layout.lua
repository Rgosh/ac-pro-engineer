-- Text sizes, spacing, and the measurements every block is laid out against.
--
-- CSP has five font tiers and no way to scale them, which on a 4K screen means
-- a panel nobody can read. DirectWrite draws at any size, so `say` is what the
-- panel draws text with, and `ui.text` is what it does not.

local settings = require('acpe.settings').values
local theme = require('acpe.theme')
local i18n = require('acpe.i18n')

local COLOR = theme.COLOR
local STYLE_COLORS = theme.STYLE_COLORS
local applyPalette = theme.apply
local tr = i18n.tr

local M = {}

-- Text size, as font tiers rather than a scale factor: CSP has no API for
-- scaling a font, but it does give five of them, and stepping between tiers is
-- what "bigger text" has to mean here.
local FONT_TIERS = {
  compact = { caption = 'Tiny',  body = 'Tiny',   hero = 'Title', gear = 'Small' },
  normal  = { caption = 'Tiny',  body = 'Main',   hero = 'Huge',  gear = 'Title' },
  large   = { caption = 'Small', body = 'Title',  hero = 'Huge',  gear = 'Huge' },
}

local TEXT_SIZES = { 'compact', 'normal', 'large' }
local BULLETS = { ['>'] = '> ', dot = '• ', none = '' }
local BULLET_NAMES = { 'severity', '>', 'dot', 'none' }

-- Severity as the desktop application shows it: red, yellow, green, with the
-- marker carrying the colour and the sentence staying readable. The emoji it
-- uses in the terminal are not in CSP's font, so the marker is the two
-- characters that read the same at a glance.
local SEVERITY_MARK = { [0] = 'i  ', [1] = '!  ', [2] = '!! ' }
local SEVERITY_WORD = { [0] = 'INFO ', [1] = 'WARN ', [2] = 'CRIT ' }
local SEVERITY_COLOR = { [0] = COLOR.good, [1] = COLOR.warn, [2] = COLOR.bad }

-- Base text sizes, multiplied by the driver's scale. CSP has five font tiers
-- and no way to scale them, which on a 4K screen means a panel nobody can
-- read; DirectWrite draws at any size, so that is what the panel uses.
local TEXT_BASE = { caption = 11, body = 15, hero = 38, gear = 22 }
local VR_BOOST = 1.35

-- What the layout was drawn against. A window twice that wide should show
-- everything twice the size rather than the same numbers with a field of empty
-- pixels beside them, which is what stretching used to do.
local DESIGN_WIDTH = 360

-- Measured once per window, not per item: `availableSpaceX` shrinks as the
-- cursor moves right, so asking it inside a column made the second column half
-- the size of the first.
local frameScale = 1
local frameWidth = 360

local function measureWindowScale()
  if not settings.autoScale then
    frameScale = 1
    return
  end
  local available = ui.availableSpaceX()
  if available <= 0 then
    frameScale = 1
    return
  end
  frameScale = math.max(0.75, math.min(2.5, available / DESIGN_WIDTH))
end

local function measureWindowWidth()
  frameWidth = math.max(120, ui.availableSpaceX())
end

local function windowScale()
  return frameScale
end

local function textSize(role)
  local base = TEXT_BASE[role] or TEXT_BASE.body
  local scale = (settings.fontScale or 1) * windowScale()
  if settings.vrMode then scale = scale * VR_BOOST end
  if settings.textSize == 'compact' then
    scale = scale * 0.85
  elseif settings.textSize == 'large' then
    scale = scale * 1.25
  end
  return base * scale
end

--- Draw a piece of the panel's text. One call instead of push/draw/pop, and
--- one place where the size of everything is decided.
local function say(role, text, color)
  ui.dwriteText(text, textSize(role), color)
end

--- Say it in the application's language.
local function sayTr(role, text, color)
  ui.dwriteText(tr(text), textSize(role), color)
end

local function pushRole(role)
  -- In a headset the panel is a metre away through lenses that blur the edges,
  -- so VR does not get its own layout — it gets the largest one, unless the
  -- driver has already asked for something larger still.
  local size = settings.textSize
  if settings.vrMode and size ~= 'large' then size = 'large' end
  local tier = FONT_TIERS[size] or FONT_TIERS.normal
  ui.pushFont(ui.Font[tier[role]] or ui.Font.Main)
end

--- Vertical air between blocks. Doubled in VR, where things that touch each
--- other read as one thing.
local function gap(pixels)
  ui.offsetCursorY(settings.vrMode and pixels * 2 or pixels)
end

--- A section caption, or nothing when the panel is set to run without them.
local function sectionLabel(text)
  if not settings.sectionLabels then return end
  say('caption', tr(text), COLOR.label)
end

--- A label above a value, as its own column.
local function stat(label, value, color)
  ui.beginGroup()
  say('caption', tr(label), COLOR.label)
  say('body', value, color or COLOR.text)
  ui.endGroup()
end

-- Spacing, pinned rather than inherited.
--
-- The LÖVE harness lays the panel out with these exact numbers, so the layout
-- judged there is the layout that ships. Left to CSP's theme, the same code
-- comes out with different gaps in game and every decision made in the harness
-- has to be made again.
local ITEM_SPACING = vec2(6, 3)
local ITEM_SPACING_VR = vec2(8, 8)
local FRAME_PADDING = vec2(6, 3)

--- Apply the panel's spacing and colours, and take the window's measure.
--- Returns what to pop.
local function pushLayoutStyle()
  measureWindowScale()
  measureWindowWidth()
  -- Guarded: a palette is a setting, and no setting is worth an empty panel.
  pcall(applyPalette)
  ui.pushStyleVar(ui.StyleVar.ItemSpacing,
    settings.vrMode and ITEM_SPACING_VR or ITEM_SPACING)
  ui.pushStyleVar(ui.StyleVar.FramePadding, FRAME_PADDING)
  ui.pushStyleVar(ui.StyleVar.FrameRounding, 3)
  ui.pushStyleVar(ui.StyleVar.GrabRounding, 3)
  ui.pushStyleVar(ui.StyleVar.TabRounding, 3)
  ui.pushStyleVar(ui.StyleVar.ScrollbarRounding, 3)
  ui.pushStyleVar(ui.StyleVar.ItemInnerSpacing, vec2(6, 3))

  local colors = 0
  for _, entry in ipairs(STYLE_COLORS) do
    local slot = ui.StyleColor[entry[1]]
    if slot ~= nil then
      ui.pushStyleColor(slot, entry[2])
      colors = colors + 1
    end
  end
  return 7, colors
end

local function popLayoutStyle(vars, colors)
  if colors ~= nil and colors > 0 then ui.popStyleColor(colors) end
  ui.popStyleVar(vars)
end

local MAX_CONTENT = 360
local MAX_CONTENT_VR = 520

local function contentWidth()
  -- With auto scale the content fills the window: the size grew with it, so
  -- capping the width would only put the extra room back as emptiness. Taken
  -- once for the same reason the scale is.
  if settings.autoScale then return frameWidth end
  local limit = settings.contentWidth or MAX_CONTENT
  if settings.vrMode then limit = math.max(limit, MAX_CONTENT_VR) end
  return math.min(ui.availableSpaceX(), limit)
end

--- Where column `index` of a row of stats begins.
---
--- Three across normally. Two in VR, where the text is large enough that a
--- third column runs into its neighbour — the wrap happens by itself, since a
--- stat leaves the cursor at the start of the next line.
local function nextColumn(width, index)
  local perRow = settings.columnsPerRow or 0
  if perRow < 2 then perRow = settings.vrMode and 2 or 3 end
  local column = index % perRow
  if column > 0 then ui.sameLine(width / perRow * column) end
end

--- A label on the left, a value on the right, filling the width.
local function row(label, value, color)
  local width = contentWidth()
  say('caption', label, COLOR.dim)
  ui.sameLine(width * 0.46)
  say('body', value, color or COLOR.text)
end

--- A row whose value is four numbers rather than one.
---
--- `row` is measured for "44.82 L": a two-character label and the value at
--- 46% of the width in body text. The telemetry window's corner readouts are
--- "26.8 psi  90°C  521°C  98%", and they ran off the right-hand edge at every
--- window size — widening the window does not help, because the text scales
--- with the width, so the overflow is the same fraction of the line however
--- big the window is. Narrow label column, caption-sized value.
local function denseRow(label, value, color)
  local width = contentWidth()
  say('caption', label, COLOR.dim)
  ui.sameLine(width * 0.20)
  say('caption', value, color or COLOR.text)
end

M.TEXT_SIZES = TEXT_SIZES
M.BULLETS = BULLETS
M.BULLET_NAMES = BULLET_NAMES
M.SEVERITY_MARK = SEVERITY_MARK
M.SEVERITY_WORD = SEVERITY_WORD
M.SEVERITY_COLOR = SEVERITY_COLOR
M.textSize = textSize
M.windowScale = windowScale
M.say = say
M.sayTr = sayTr
M.pushRole = pushRole
M.gap = gap
M.sectionLabel = sectionLabel
M.stat = stat
M.row = row
M.denseRow = denseRow
M.contentWidth = contentWidth
M.nextColumn = nextColumn
M.push = pushLayoutStyle
M.pop = popLayoutStyle

return M
