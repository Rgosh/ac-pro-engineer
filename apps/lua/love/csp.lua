-- Enough of CSP's app API to run the overlay under LÖVE.
--
-- The overlay script is written against Assetto Corsa's Lua environment: it
-- draws with `ui.*`, reads shared memory through `ac.readMemoryMappedFile` and
-- is driven by CSP calling `script.update` and `script.windowMain`. None of
-- that exists outside the game, which is why the only way to look at the panel
-- used to be launching AC.
--
-- This module puts the same globals in place on top of LÖVE. Layout follows
-- ImGui's rules closely enough that the app's `sameLine`/`beginGroup` code
-- lands where it does in game: items advance the cursor down a line, sameLine
-- pulls the next item back up beside the previous one, and a group is measured
-- as a single item so a sameLine after it clears the whole block.
--
-- Anything the app calls that is not implemented here resolves to a no-op that
-- records itself in `csp.unimplemented`, so a missing function shows up in the
-- harness's Log tab instead of taking the frame down.

local csp = {}

local gfx = love.graphics

-- CSP's font tiers. The sizes are picked to sit in the same proportions as the
-- in-game ones rather than to match them pixel for pixel.
local FONT_SIZES = {
  Tiny = 11,
  Small = 13,
  Monospace = 13,
  Main = 15,
  Italic = 15,
  Title = 22,
  Huge = 38,
}

csp.unimplemented = {}

-- Layout state. One window is drawn at a time, so a single set of these is
-- enough; `stack` holds the saved state of enclosing groups.
local L = {
  originX = 0, originY = 0, width = 0, height = 0,
  x = 0, y = 0,
  lineStartX = 0, lineY = 0, lineH = 0,
  prevLineY = 0, prevLineH = 0,
  lastX = 0, lastY = 0, lastW = 0, lastH = 0,
  maxX = 0, maxY = 0,
  spacingX = 6, spacingY = 3,
  fonts = {}, fontStack = {}, currentFont = nil,
  nextItemWidth = nil,
  stack = {},
}

-- Mouse state for the frame being drawn. The harness feeds this in; widgets
-- read it the way immediate-mode UI always does — no callbacks, no retained
-- widget objects.
csp.input = {
  x = 0, y = 0,
  down = false,        -- button held right now
  pressed = false,     -- went down during this frame
  released = false,    -- came up during this frame
  activeId = nil,      -- widget that grabbed the drag
}

local bit = require('bit')
local scale = 1

-- Style stacks. Declared up here because the text functions read them and the
-- push/pop pairs that fill them come further down.
local styleVars, varOrder = {}, {}
local styleColors, colorOrder = {}, {}

local function topOf(store, id)
  local stack = store[id]
  if stack == nil then return nil end
  return stack[#stack]
end

-- ---------------------------------------------------------------------------
-- Fonts and colours
-- ---------------------------------------------------------------------------

local function buildFonts()
  L.fonts = {}
  for name, size in pairs(FONT_SIZES) do
    L.fonts[name] = gfx.newFont(math.max(6, math.floor(size * scale + 0.5)))
  end
  L.currentFont = L.fonts.Main
end

--- Rebuild the fonts at a new scale. Cheap, and only ever called from settings.
function csp.setScale(value)
  scale = value
  buildFonts()
end

function csp.getScale() return scale end

local function font()
  return L.currentFont or L.fonts.Main
end

local function setColor(c)
  if c == nil then
    gfx.setColor(0.88, 0.90, 0.94, 1)
  else
    gfx.setColor(c.r or 1, c.g or 1, c.b or 1, c.mult or 1)
  end
end

csp.setColor = setColor

-- ---------------------------------------------------------------------------
-- Item placement
-- ---------------------------------------------------------------------------

--- Record an item of this size at the cursor and move to the next line.
local function itemSize(w, h)
  L.lastX, L.lastY, L.lastW, L.lastH = L.x, L.y, w, h
  L.prevLineY = L.lineY
  L.prevLineH = math.max(L.lineH, h)

  L.maxX = math.max(L.maxX, L.x + w)
  L.maxY = math.max(L.maxY, L.y + h)

  L.lineY = L.lineY + L.prevLineH + L.spacingY
  L.x = L.lineStartX
  L.y = L.lineY
  L.lineH = 0
end

local function contentRight() return L.originX + L.width end
local function contentBottom() return L.originY + L.height end

--- Begin drawing a CSP-style window into this rectangle.
-- How far each window's contents are scrolled. In game CSP scrolls a window
-- whose contents outgrow it; here the wheel does the same, so a settings list
-- that does not fit is a list you can still reach the bottom of.
local scrollOffsets = {}
local scrollTarget = nil
csp.wheel = 0

--- Scroll the window drawn under the pointer, and clamp it to its contents.
local function applyScroll(id, x, y, w, h)
  if id == nil then return 0 end

  local offset = scrollOffsets[id] or 0
  local input = csp.input
  local hovering = input.x >= x and input.x <= x + w and input.y >= y and input.y <= y + h
  if hovering and csp.wheel ~= 0 then
    offset = offset - csp.wheel * 28
  end

  local content = csp.contentHeight and csp.contentHeight[id] or 0
  local maximum = math.max(0, content - h)
  if offset > maximum then offset = maximum end
  if offset < 0 then offset = 0 end
  scrollOffsets[id] = offset
  return offset
end

csp.contentHeight = {}

function csp.beginWindow(x, y, w, h, id)
  local offset = applyScroll(id, x, y, w, h)
  scrollTarget = id

  L.originX, L.originY, L.width, L.height = x, y - offset, w, h + offset
  L.lineStartX = x
  L.x, L.y, L.lineY = x, y - offset, y - offset
  L.lineH, L.prevLineH, L.prevLineY = 0, 0, y
  L.lastX, L.lastY, L.lastW, L.lastH = x, L.y, 0, 0
  L.maxX, L.maxY = x, L.y
  L.scrollY = offset
  L.viewTop, L.viewHeight = y, h
  L.fontStack = {}
  L.currentFont = L.fonts.Main
  L.nextItemWidth = nil
  L.stack = {}
  gfx.setScissor(x, y, w, h)
end

function csp.endWindow()
  local used = L.maxY - L.originY

  -- A scrollbar, only when there is something to scroll to.
  if scrollTarget ~= nil then
    csp.contentHeight[scrollTarget] = used
    local view = L.viewHeight or L.height
    if used > view then
      local track = view
      local thumb = math.max(24, track * view / used)
      local travel = (track - thumb) * ((L.scrollY or 0) / math.max(1, used - view))
      gfx.setColor(1, 1, 1, 0.06)
      gfx.rectangle('fill', L.originX + L.width - 4, L.viewTop, 3, track, 2, 2)
      gfx.setColor(0.62, 0.66, 0.72, 0.45)
      gfx.rectangle('fill', L.originX + L.width - 4, L.viewTop + travel, 3, thumb, 2, 2)
    end
    scrollTarget = nil
  end

  gfx.setScissor()
  return used
end

-- ---------------------------------------------------------------------------
-- Widget plumbing
-- ---------------------------------------------------------------------------

local function hovered(x, y, w, h)
  local i = csp.input
  return i.x >= x and i.x <= x + w and i.y >= y and i.y <= y + h
end

--- A stable-enough identity for a widget: its label plus where it was drawn.
local function widgetId(label, x, y)
  return string.format('%s@%d,%d', label, math.floor(x), math.floor(y))
end

--- What ImGui actually draws for a label.
---
--- Everything from `##` onward is the widget's identity, not its caption — it
--- is how two checkboxes on the same screen can both say "Enabled" and still
--- be told apart. The panel leans on this heavily: nearly every control is
--- `ui.checkbox('##showHeader')` with the caption drawn beside it at a chosen
--- size, because CSP's font tiers cannot be scaled. Drawing the raw string put
--- `##showHeader` on screen in front of every setting.
local function caption(label)
  label = tostring(label)
  local cut = label:find('##', 1, true)
  if cut == nil then return label end
  return label:sub(1, cut - 1)
end

local function itemWidth(fallback)
  local w = L.nextItemWidth or fallback
  L.nextItemWidth = nil
  return w
end

-- ---------------------------------------------------------------------------
-- The API the overlay uses
-- ---------------------------------------------------------------------------

local ui = {}

ui.Font = {}
for name in pairs(FONT_SIZES) do ui.Font[name] = name end

function ui.pushFont(f)
  L.fontStack[#L.fontStack + 1] = L.currentFont
  L.currentFont = L.fonts[f] or L.fonts.Main
end

function ui.popFont()
  local n = #L.fontStack
  if n > 0 then
    L.currentFont = L.fontStack[n]
    L.fontStack[n] = nil
  end
end

function ui.text(s)
  ui.textColored(s, topOf(styleColors, ui.StyleColor.Text))
end

function ui.textColored(s, color)
  s = tostring(s)
  local f = font()
  setColor(color)
  gfx.setFont(f)
  gfx.print(s, L.x, L.y)
  itemSize(f:getWidth(s), f:getHeight())
end

function ui.dummy(size)
  itemSize(size.x, size.y)
end

function ui.offsetCursorY(dy)
  L.lineY = L.lineY + dy
  L.y = L.lineY
end

function ui.offsetCursorX(dx)
  L.x = L.x + dx
end

function ui.sameLine(offsetX, spacing)
  if offsetX ~= nil and offsetX > 0 then
    L.x = L.lineStartX + offsetX
  else
    L.x = L.lastX + L.lastW + (spacing or L.spacingX)
  end
  L.lineY = L.prevLineY
  L.y = L.lineY
  L.lineH = L.prevLineH
end

function ui.newLine()
  itemSize(0, font():getHeight())
end

function ui.getCursor()
  return { x = L.x, y = L.y }
end

function ui.setCursor(pos)
  L.x, L.y, L.lineY = pos.x, pos.y, pos.y
end

function ui.availableSpaceX()
  return math.max(0, contentRight() - L.x)
end

function ui.availableSpace()
  return { x = math.max(0, contentRight() - L.x), y = math.max(0, contentBottom() - L.y) }
end

function ui.drawRectFilled(from, to, color, rounding)
  setColor(color)
  local w, h = to.x - from.x, to.y - from.y
  if rounding and rounding > 0 then
    gfx.rectangle('fill', from.x, from.y, w, h, rounding, rounding)
  else
    gfx.rectangle('fill', from.x, from.y, w, h)
  end
end

function ui.drawRect(from, to, color, rounding, thickness)
  setColor(color)
  gfx.setLineWidth(thickness or 1)
  gfx.rectangle('line', from.x, from.y, to.x - from.x, to.y - from.y, rounding or 0, rounding or 0)
  gfx.setLineWidth(1)
end

function ui.drawLine(from, to, color, thickness)
  setColor(color)
  gfx.setLineWidth(thickness or 1)
  gfx.line(from.x, from.y, to.x, to.y)
  gfx.setLineWidth(1)
end

--- A scrollable child region. Draws the body inline: the harness has no
--- clipping to speak of, and swallowing the body would leave every check below
--- passing against a window that drew nothing.
function ui.childWindow(_, _, _, body)
  if type(body) == 'function' then body() end
  return true
end

function ui.separator()
  local w = ui.availableSpaceX()
  gfx.setColor(1, 1, 1, 0.10)
  gfx.rectangle('fill', L.x, L.y + 3, w, 1)
  itemSize(w, 7)
end

function ui.setNextItemWidth(w)
  L.nextItemWidth = w
end

function ui.beginGroup()
  L.stack[#L.stack + 1] = {
    x = L.x, lineY = L.lineY, lineStartX = L.lineStartX,
    lineH = L.lineH, prevLineY = L.prevLineY, prevLineH = L.prevLineH,
    maxX = L.maxX, maxY = L.maxY,
  }
  L.lineStartX = L.x
  L.maxX, L.maxY = L.x, L.y
  L.lineH = 0
end

function ui.endGroup()
  local g = table.remove(L.stack)
  if g == nil then return end

  local w = math.max(0, L.maxX - g.x)
  local h = math.max(0, L.maxY - g.lineY)

  L.lineStartX = g.lineStartX
  L.x, L.lineY, L.y = g.x, g.lineY, g.lineY
  L.lineH, L.prevLineY, L.prevLineH = g.lineH, g.prevLineY, g.prevLineH
  L.maxX, L.maxY = math.max(g.maxX, L.maxX), math.max(g.maxY, L.maxY)

  itemSize(w, h)
end

-- ---------------------------------------------------------------------------
-- Interactive widgets
--
-- These exist in CSP too, with these signatures — the overlay's settings
-- window uses checkbox/button/separator, and the harness's own panels use the
-- rest.
-- ---------------------------------------------------------------------------

local COL = {
  frame     = { r = 0.16, g = 0.17, b = 0.21, mult = 1 },
  frameHot  = { r = 0.22, g = 0.24, b = 0.29, mult = 1 },
  frameDown = { r = 0.13, g = 0.14, b = 0.17, mult = 1 },
  accent    = { r = 0.20, g = 0.72, b = 1.00, mult = 1 },
  accentDim = { r = 0.16, g = 0.44, b = 0.62, mult = 1 },
  text      = { r = 0.88, g = 0.90, b = 0.94, mult = 1 },
  textDim   = { r = 0.62, g = 0.66, b = 0.72, mult = 1 },
}

csp.colors = COL

function ui.button(label, size, _flags)
  local f = font()
  local padX, padY = 10, 5
  local shown = caption(label)
  local w = (size and size.x) or itemWidth(f:getWidth(shown) + padX * 2)
  local h = (size and size.y) or (f:getHeight() + padY * 2)
  local x, y = L.x, L.y

  local hot = hovered(x, y, w, h)
  local id = widgetId(label, x, y)
  if hot and csp.input.pressed then csp.input.activeId = id end
  local down = hot and csp.input.down

  setColor(down and COL.frameDown or (hot and COL.frameHot or COL.frame))
  gfx.rectangle('fill', x, y, w, h, 3, 3)
  setColor(hot and COL.text or COL.textDim)
  gfx.setFont(f)
  gfx.print(shown, x + (w - f:getWidth(shown)) * 0.5, y + padY)

  itemSize(w, h)
  return hot and csp.input.released and csp.input.activeId == id
end

function ui.checkbox(label, checked)
  local f = font()
  local box = f:getHeight() + 2
  local w = box + 6 + f:getWidth(caption(label))
  local h = box
  local x, y = L.x, L.y

  local hot = hovered(x, y, w, h)
  local id = widgetId(label, x, y)
  if hot and csp.input.pressed then csp.input.activeId = id end

  setColor(hot and COL.frameHot or COL.frame)
  gfx.rectangle('fill', x, y, box, box, 3, 3)
  if checked then
    setColor(COL.accent)
    gfx.rectangle('fill', x + 3, y + 3, box - 6, box - 6, 2, 2)
  end
  setColor(hot and COL.text or COL.textDim)
  gfx.setFont(f)
  gfx.print(caption(label), x + box + 6, y)

  itemSize(w, h)
  return hot and csp.input.released and csp.input.activeId == id
end

function ui.radioButton(label, active)
  local f = font()
  local r = (f:getHeight()) * 0.5
  local w = r * 2 + 6 + f:getWidth(caption(label))
  local h = r * 2
  local x, y = L.x, L.y

  local hot = hovered(x, y, w, h)
  local id = widgetId(label, x, y)
  if hot and csp.input.pressed then csp.input.activeId = id end

  setColor(hot and COL.frameHot or COL.frame)
  gfx.circle('fill', x + r, y + r, r)
  if active then
    setColor(COL.accent)
    gfx.circle('fill', x + r, y + r, r - 3)
  end
  setColor(hot and COL.text or COL.textDim)
  gfx.setFont(f)
  gfx.print(caption(label), x + r * 2 + 6, y)

  itemSize(w, h)
  return hot and csp.input.released and csp.input.activeId == id
end

--- CSP returns `value, changed`; so does this.
function ui.slider(label, value, min, max, format, power)
  min = min or 0
  max = max or 1
  local integer = power == true
  local f = font()
  local h = f:getHeight() + 6
  local w = itemWidth(math.max(80, ui.availableSpaceX()))
  local x, y = L.x, L.y

  local id = widgetId(label, x, y)
  local hot = hovered(x, y, w, h)
  local input = csp.input

  if hot and input.pressed then input.activeId = id end
  local changed = false
  if input.activeId == id and input.down then
    local t = (input.x - x) / math.max(1, w)
    t = math.max(0, math.min(1, t))
    local v = min + t * (max - min)
    if integer then v = math.floor(v + 0.5) end
    if v ~= value then
      value = v
      changed = true
    end
  end

  local t = (max > min) and ((value - min) / (max - min)) or 0
  t = math.max(0, math.min(1, t))

  setColor(COL.frame)
  gfx.rectangle('fill', x, y, w, h, 3, 3)
  setColor(hot and COL.accent or COL.accentDim)
  gfx.rectangle('fill', x, y, w * t, h, 3, 3)

  local shown = caption(label)
  local text = string.format(
    (shown ~= '' and (shown .. '  ') or '') .. (format or (integer and '%.0f' or '%.3f')),
    value)
  setColor(COL.text)
  gfx.setFont(f)
  gfx.print(text, x + 6, y + 3)

  itemSize(w, h)
  return value, changed
end

--- Text at an arbitrary size, which is how the panel survives a 4K screen:
--- CSP has five font tiers and no way to scale them, but it can draw text at
--- any size through DirectWrite.
function ui.dwriteText(text, fontSize, color)
  text = tostring(text)
  fontSize = fontSize or 14
  local key = math.max(6, math.floor(fontSize + 0.5))
  L.dwriteFonts = L.dwriteFonts or {}
  local f = L.dwriteFonts[key]
  if f == nil then
    f = gfx.newFont(key)
    L.dwriteFonts[key] = f
  end

  setColor(color or topOf(styleColors, ui.StyleColor.Text))
  gfx.setFont(f)
  gfx.print(text, L.x, L.y)
  itemSize(f:getWidth(text), f:getHeight())
end

function ui.measureDWriteText(text, fontSize)
  local key = math.max(6, math.floor((fontSize or 14) + 0.5))
  L.dwriteFonts = L.dwriteFonts or {}
  local f = L.dwriteFonts[key] or gfx.newFont(key)
  L.dwriteFonts[key] = f
  return { x = f:getWidth(tostring(text)), y = f:getHeight() }
end

--- CSP's own text field: returns the value, whether it changed, and whether
--- enter was pressed.
function ui.inputText(label, str, _flags, _size)
  csp.inputState = csp.inputState or { text = '' }
  csp.inputState.text = str or csp.inputState.text
  local focused = ui.inputTextBox(label, csp.inputState, 'type a command')
  local entered = focused and csp.enterPressed or false
  return csp.inputState.text, false, entered
end

--- A single-line text field. `state` is a table with a `text` field, which is
--- where the typing lands; the caller owns it, the way immediate-mode UI wants.
function ui.inputTextBox(label, state, hint)
  local f = font()
  local h = f:getHeight() + 8
  local w = itemWidth(ui.availableSpaceX())
  local x, y = L.x, L.y
  local id = widgetId(label, x, y)

  local hot = hovered(x, y, w, h)
  if hot and csp.input.pressed then csp.focusedInput = id end
  if csp.input.pressed and not hot and csp.focusedInput == id then csp.focusedInput = nil end
  local focused = csp.focusedInput == id

  setColor(focused and COL.frameHot or COL.frame)
  gfx.rectangle('fill', x, y, w, h, 3, 3)
  if focused then
    setColor(COL.accent)
    gfx.rectangle('line', x, y, w, h, 3, 3)
  end

  local text = state.text or ''
  setColor(#text > 0 and COL.text or COL.textDim)
  gfx.setFont(f)
  local shown = #text > 0 and text or (hint or '')
  if focused and math.floor(love.timer.getTime() * 2) % 2 == 0 then
    shown = shown .. '_'
  end
  gfx.print(shown, x + 6, y + 4)

  itemSize(w, h)
  return focused
end

--- Feed a typed character to whatever field has focus.
function csp.textInput(character)
  if csp.focusedInput == nil or csp.inputState == nil then return end
  csp.inputState.text = (csp.inputState.text or '') .. character
end

function csp.inputBackspace()
  if csp.focusedInput == nil or csp.inputState == nil then return end
  local text = csp.inputState.text or ''
  csp.inputState.text = text:sub(1, -2)
end

-- Tab bars keep the labels they saw last frame so the header row can be drawn
-- before the content callbacks run. The one-frame lag only shows on the very
-- first frame, when nothing has been clicked yet anyway.
local tabBars = {}
local currentTabBar = nil

function ui.tabBar(id, a, b)
  local content = b or a
  local state = tabBars[id]
  if state == nil then
    state = { labels = {}, selected = nil }
    tabBars[id] = state
  end

  local f = L.fonts.Small
  local h = f:getHeight() + 8
  local x, y = L.x, L.y
  local cursor = x

  for _, label in ipairs(state.labels) do
    local w = f:getWidth(label) + 18
    local hot = hovered(cursor, y, w, h)
    local selected = state.selected == label
    if hot and csp.input.pressed then csp.input.activeId = widgetId(label, cursor, y) end
    if hot and csp.input.released and csp.input.activeId == widgetId(label, cursor, y) then
      state.selected = label
    end
    setColor(selected and COL.frameHot or (hot and COL.frame or nil))
    if selected or hot then
      gfx.rectangle('fill', cursor, y, w, h, 3, 3)
    end
    setColor(selected and COL.text or COL.textDim)
    gfx.setFont(f)
    gfx.print(label, cursor + 9, y + 4)
    if selected then
      setColor(COL.accent)
      gfx.rectangle('fill', cursor + 4, y + h - 2, w - 8, 2)
    end
    cursor = cursor + w + 2
  end

  itemSize(math.max(0, cursor - x), h)
  ui.offsetCursorY(4)

  -- Restored, not cleared. A tab bar nested inside another one — which is how
  -- the panel's settings window is built — used to set this back to `nil` on
  -- the way out, so every `ui.tabItem` in the *outer* bar after the nested one
  -- saw no bar at all. Those items fell to the "no bar" path, which draws no
  -- label and runs the body unconditionally: the outer bar lost four of its
  -- five tabs and drew their contents stacked underneath whichever one was
  -- selected.
  local parent = currentTabBar
  state.pending = {}
  currentTabBar = state
  local ok, err = pcall(content)
  currentTabBar = parent
  state.labels = state.pending
  if not ok then error(err, 0) end
end

function ui.tabItem(label, a, b)
  local content = b or a
  local state = currentTabBar
  if state == nil then
    if type(content) == 'function' then content() end
    return
  end
  state.pending[#state.pending + 1] = label
  if state.selected == nil then state.selected = label end
  if state.selected == label and type(content) == 'function' then content() end
end

--- Which tab is showing, for the harness to persist across runs.
function csp.selectedTab(id) return tabBars[id] and tabBars[id].selected end
function csp.selectTab(id, label)
  local state = tabBars[id]
  if state == nil then
    state = { labels = {}, selected = label }
    tabBars[id] = state
  else
    state.selected = label
  end
end

-- ---------------------------------------------------------------------------
-- Windows and styles
--
-- CSP gives an app a window and the app may push its own styling inside it.
-- Rounding, padding and background colour are most of what the panel looks
-- like, so they are emulated rather than ignored — otherwise what is on screen
-- here and what is on screen in game are two different panels.
-- ---------------------------------------------------------------------------

ui.WindowFlags = {
  None = 0,
  NoTitleBar = 1,
  NoResize = 2,
  NoMove = 4,
  NoScrollbar = 8,
  NoCollapse = 32,
  AlwaysAutoResize = 64,
  NoBackground = 128,
  NoDecoration = 1 + 2 + 8 + 32,
}

-- CSP's numbering, so a var the panel pushes is a var the harness knows about.
ui.StyleVar = {
  Alpha = 0, WindowRounding = 1, WindowBorderSize = 2, ChildRounding = 3,
  ChildBorderSize = 4, PopupRounding = 5, PopupBorderSize = 6, FrameBorderSize = 7,
  IndentSpacing = 8, ScrollbarSize = 9, FrameRounding = 10, ScrollbarRounding = 11,
  GrabMinSize = 12, GrabRounding = 13, TabRounding = 14, WindowPadding = 15,
  WindowMinSize = 16, WindowTitleAlign = 17, FramePadding = 18, ItemSpacing = 19,
  ItemInnerSpacing = 20, ButtonTextAlign = 21, SelectableTextAlign = 22,
  SelectablePadding = 23, SliderTextAlign = 24,
}

ui.StyleColor = {
  Text = 1, TextDisabled = 2, WindowBg = 3, ChildBg = 4, PopupBg = 5,
  Border = 6, FrameBg = 7, TitleBg = 8, Button = 9, Header = 10,
}

local function pushStyle(store, order, id, value)
  -- A style the harness does not model still has to push and pop in pairs, or
  -- the counts the panel passes to popStyleVar stop matching.
  if id == nil then id = '__unknown' end
  local stack = store[id]
  if stack == nil then
    stack = {}
    store[id] = stack
  end
  stack[#stack + 1] = value
  order[#order + 1] = id
end

local function popStyle(store, order, count)
  for _ = 1, (count or 1) do
    local id = table.remove(order)
    if id ~= nil then
      local stack = store[id]
      if stack ~= nil then stack[#stack] = nil end
    end
  end
end

function ui.pushStyleVar(var, value)
  pushStyle(styleVars, varOrder, var, value)
  -- ItemSpacing is layout, not decoration: the panel pins it so the gaps it
  -- was designed with are the gaps it gets, and the harness has to obey it for
  -- the comparison to mean anything.
  if var == ui.StyleVar.ItemSpacing and type(value) == 'table' then
    L.spacingX, L.spacingY = value.x or L.spacingX, value.y or L.spacingY
  end
end

function ui.popStyleVar(count)
  popStyle(styleVars, varOrder, count)
  local spacing = topOf(styleVars, ui.StyleVar.ItemSpacing)
  if type(spacing) == 'table' then
    L.spacingX, L.spacingY = spacing.x or 6, spacing.y or 3
  else
    L.spacingX, L.spacingY = 6, 3
  end
end
function ui.pushStyleColor(color, value) pushStyle(styleColors, colorOrder, color, value) end
function ui.popStyleColor(count) popStyle(styleColors, colorOrder, count) end

local function flagSet(flags, flag)
  return flags ~= nil and bit.band(flags, flag) ~= 0
end

local function saveLayout()
  return {
    originX = L.originX, originY = L.originY, width = L.width, height = L.height,
    x = L.x, y = L.y, lineStartX = L.lineStartX, lineY = L.lineY, lineH = L.lineH,
    prevLineY = L.prevLineY, prevLineH = L.prevLineH,
    lastX = L.lastX, lastY = L.lastY, lastW = L.lastW, lastH = L.lastH,
    maxX = L.maxX, maxY = L.maxY, stack = L.stack,
  }
end

local function restoreLayout(saved)
  for key, value in pairs(saved) do L[key] = value end
end

local windowStack = {}

--- The app's own window, drawn inside the region CSP already gave it: a
--- background in `StyleColor.WindowBg`, corners from `StyleVar.WindowRounding`
--- and a content area inset by `StyleVar.WindowPadding`.
function ui.begin(title, flags)
  local padding = topOf(styleVars, ui.StyleVar.WindowPadding) or { x = 8, y = 8 }
  local rounding = topOf(styleVars, ui.StyleVar.WindowRounding) or 0
  local background = topOf(styleColors, ui.StyleColor.WindowBg)

  local x, y, w, h = L.originX, L.originY, L.width, L.height

  if background ~= nil and not flagSet(flags, ui.WindowFlags.NoBackground) then
    setColor(background)
    gfx.rectangle('fill', x, y, w, h, rounding, rounding)
  end

  local titleHeight = 0
  if not flagSet(flags, ui.WindowFlags.NoTitleBar) then
    titleHeight = L.fonts.Small:getHeight() + 8
    setColor(COL.frame)
    gfx.rectangle('fill', x, y, w, titleHeight, rounding, rounding)
    setColor(COL.text)
    gfx.setFont(L.fonts.Small)
    gfx.print(tostring(title), x + 8, y + 4)
  end

  windowStack[#windowStack + 1] = saveLayout()

  local contentX = x + (padding.x or 8)
  local contentY = y + titleHeight + (padding.y or 8)
  L.originX, L.originY = contentX, contentY
  L.width = math.max(0, w - (padding.x or 8) * 2)
  L.height = math.max(0, h - titleHeight - (padding.y or 8) * 2)
  L.lineStartX = contentX
  L.x, L.y, L.lineY = contentX, contentY, contentY
  L.lineH, L.prevLineH, L.prevLineY = 0, 0, contentY
  L.lastX, L.lastY, L.lastW, L.lastH = contentX, contentY, 0, 0
  L.maxX, L.maxY = contentX, contentY
  L.stack = {}
  return true
end

ui['end'] = function()
  local saved = table.remove(windowStack)
  if saved ~= nil then restoreLayout(saved) end
end

--- Text that wraps at the content edge, honouring a pushed `StyleColor.Text`.
function ui.textWrapped(text)
  text = tostring(text)
  local f = font()
  local width = math.max(20, ui.availableSpaceX())
  local _, lines = f:getWrap(text, width)
  if #lines == 0 then lines = { text } end

  setColor(topOf(styleColors, ui.StyleColor.Text))
  gfx.setFont(f)
  for index, line in ipairs(lines) do
    gfx.print(line, L.x, L.y + (index - 1) * f:getHeight())
  end
  itemSize(width, #lines * f:getHeight())
end

-- ---------------------------------------------------------------------------
-- CSP's window chrome
--
-- What the app sits inside in game: a rounded translucent panel, the app icon
-- and name along the top, and the gear that opens the settings window. Drawn
-- here so the harness shows the same furniture, not just the same contents.
-- ---------------------------------------------------------------------------

local CHROME = {
  background = { r = 0.06, g = 0.065, b = 0.08, mult = 0.94 },
  titleBar   = { r = 0.11, g = 0.12, b = 0.15, mult = 1 },
  border     = { r = 1, g = 1, b = 1, mult = 0.06 },
  title      = { r = 0.80, g = 0.84, b = 0.90, mult = 1 },
  icon       = { r = 0.62, g = 0.66, b = 0.72, mult = 1 },
  iconHot    = { r = 0.20, g = 0.72, b = 1.00, mult = 1 },
}

local TITLE_HEIGHT = 26

--- Draw a CSP-style app window. Returns the content rectangle and which piece
--- of chrome was clicked this frame.
function csp.appFrame(x, y, w, h, options)
  options = options or {}
  local input = csp.input

  gfx.setColor(CHROME.background.r, CHROME.background.g, CHROME.background.b, CHROME.background.mult)
  gfx.rectangle('fill', x, y, w, h, 6, 6)
  local titleShade = options.dragging and 1.35 or 1
  gfx.setColor(CHROME.titleBar.r * titleShade, CHROME.titleBar.g * titleShade,
    CHROME.titleBar.b * titleShade, 1)
  gfx.rectangle('fill', x, y, w, TITLE_HEIGHT, 6, 6)
  gfx.rectangle('fill', x, y + TITLE_HEIGHT - 6, w, 6)
  if options.dragging then
    gfx.setColor(CHROME.iconHot.r, CHROME.iconHot.g, CHROME.iconHot.b, 0.5)
    gfx.rectangle('line', x, y, w, h, 6, 6)
  end
  gfx.setColor(CHROME.border.r, CHROME.border.g, CHROME.border.b, CHROME.border.mult)
  gfx.rectangle('line', x, y, w, h, 6, 6)

  local textX = x + 8
  if options.icon ~= nil then
    gfx.setColor(1, 1, 1, 0.9)
    local size = 16
    gfx.draw(options.icon, textX, y + (TITLE_HEIGHT - size) * 0.5, 0,
      size / options.icon:getWidth(), size / options.icon:getHeight())
    textX = textX + size + 6
  end

  gfx.setColor(CHROME.title.r, CHROME.title.g, CHROME.title.b, 1)
  gfx.setFont(L.fonts.Small)
  gfx.print(options.title or 'App', textX, y + (TITLE_HEIGHT - L.fonts.Small:getHeight()) * 0.5)

  -- Chrome buttons, right to left, in CSP's order: settings, then close.
  local result = { settings = false, close = false }
  local buttonSize = TITLE_HEIGHT
  local cursorX = x + w - buttonSize

  local function chromeButton(kind, draw)
    local hot = csp.input.x >= cursorX and csp.input.x <= cursorX + buttonSize
      and csp.input.y >= y and csp.input.y <= y + TITLE_HEIGHT
    if hot then
      gfx.setColor(1, 1, 1, 0.08)
      gfx.rectangle('fill', cursorX, y, buttonSize, TITLE_HEIGHT, 4, 4)
    end
    local colour = hot and CHROME.iconHot or CHROME.icon
    gfx.setColor(colour.r, colour.g, colour.b, 1)
    draw(cursorX + buttonSize * 0.5, y + TITLE_HEIGHT * 0.5)
    if hot and input.released then result[kind] = true end
    cursorX = cursorX - buttonSize
  end

  if options.closable then
    chromeButton('close', function(cx, cy)
      gfx.setLineWidth(1.4)
      gfx.line(cx - 4, cy - 4, cx + 4, cy + 4)
      gfx.line(cx + 4, cy - 4, cx - 4, cy + 4)
      gfx.setLineWidth(1)
    end)
  end

  if options.settings then
    chromeButton('settings', function(cx, cy)
      -- A gear: a ring with six teeth, which is what CSP's icon reads as at
      -- this size.
      gfx.setLineWidth(1.6)
      gfx.circle('line', cx, cy, 4.2)
      for tooth = 0, 5 do
        local angle = tooth * math.pi / 3
        gfx.line(cx + math.cos(angle) * 5, cy + math.sin(angle) * 5,
          cx + math.cos(angle) * 7.5, cy + math.sin(angle) * 7.5)
      end
      gfx.setLineWidth(1)
    end)
  end

  result.x = x
  result.y = y + TITLE_HEIGHT
  result.width = w
  result.height = h - TITLE_HEIGHT
  return result
end

csp.TITLE_HEIGHT = TITLE_HEIGHT

-- Unknown calls become no-ops that report themselves once. A CSP function the
-- overlay starts using but the harness has not grown yet then shows up as a
-- line in the Log tab rather than as a crash halfway through a frame.
local uiProxy = setmetatable({}, {
  __index = function(_, key)
    if csp.unimplemented[key] == nil then
      csp.unimplemented[key] = 0
    end
    return function(...)
      csp.unimplemented[key] = csp.unimplemented[key] + 1
      return nil
    end
  end,
})

for k, v in pairs(ui) do uiProxy[k] = v end

-- ---------------------------------------------------------------------------
-- `ac`, and the globals CSP puts in place
-- ---------------------------------------------------------------------------

--- Persistent app settings, the way `ac.storage` works in game: hand it a table
--- of defaults, get back a table whose writes are saved.
---
--- `saveName` of `false` is storage that starts at the defaults and remembers
--- nothing, which is what a screenshot run wants: the pictures have to be the
--- same every time, and a run that turned developer mode on to photograph the
--- Dev tab used to leave it on in every picture taken after it.
local function makeStorage(saveName)
  local saved = {}
  if saveName ~= false then
    local ok, chunk = pcall(love.filesystem.load, saveName)
    if ok and chunk then
      local good, value = pcall(chunk)
      if good and type(value) == 'table' then saved = value end
    end
  end

  return function(defaults, _prefix)
    local values = {}
    for k, v in pairs(defaults) do
      local stored = saved[k]
      if stored ~= nil and type(stored) == type(v) then
        values[k] = stored
      else
        values[k] = v
      end
    end

    local function persist()
      if saveName == false then return end
      local out = { 'return {' }
      for k, v in pairs(values) do
        if type(v) == 'string' then
          out[#out + 1] = string.format('  [%q] = %q,', k, v)
        else
          out[#out + 1] = string.format('  [%q] = %s,', k, tostring(v))
        end
      end
      out[#out + 1] = '}'
      love.filesystem.write(saveName, table.concat(out, '\n'))
    end

    return setmetatable({}, {
      __index = values,
      __newindex = function(_, k, v)
        values[k] = v
        persist()
      end,
      __pairs = function() return pairs(values) end,
    })
  end
end

--- Put CSP's globals in place. `frameSource` is a function returning the table
--- the overlay reads its telemetry from.
--- Where the app's own settings file goes. The panel asks CSP for a folder
--- and keeps its settings there; without this it falls back to a path relative
--- to the working directory, which under the harness is the middle of the
--- checkout.
function csp.install(frameSource, storageFile, settingsDir)
  buildFonts()

  _G.vec2 = function(x, y) return { x = x or 0, y = y or 0 } end
  _G.vec3 = function(x, y, z) return { x = x or 0, y = y or 0, z = z or 0 } end
  _G.rgbm = function(r, g, b, m) return { r = r or 0, g = g or 0, b = b or 0, mult = m or 1 } end
  _G.rgb = function(r, g, b) return { r = r or 0, g = g or 0, b = b or 0, mult = 1 } end
  _G.bit = require('bit')
  _G.ui = uiProxy

  _G.ac = {
    -- The layout table is CSP's business; the harness only needs the call to
    -- succeed, since the frame it hands back is already a Lua table.
    StructItem = setmetatable({}, {
      __index = function() return function() return 0 end end,
    }),
    readMemoryMappedFile = function(_name, _layout) return frameSource() end,
    storage = makeStorage(storageFile == nil and 'app-settings.lua' or storageFile),
    -- CSP's folder API, enough of it for the panel to find somewhere to keep
    -- its settings file.
    FolderID = { ExtCfgUser = 1, Cfg = 2 },
    getFolder = function()
      return settingsDir or love.filesystem.getSaveDirectory()
    end,
    log = function(...) csp.log(...) end,
    warn = function(...) csp.log(...) end,
    debug = function() end,
    getSim = function() return {} end,
  }

  _G.script = {}
  return _G.script
end

csp.messages = {}

function csp.log(...)
  local parts = {}
  for i = 1, select('#', ...) do parts[#parts + 1] = tostring((select(i, ...))) end
  csp.messages[#csp.messages + 1] = table.concat(parts, ' ')
  if #csp.messages > 200 then table.remove(csp.messages, 1) end
end

return csp
