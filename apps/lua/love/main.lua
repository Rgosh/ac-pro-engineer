-- The overlay, running outside Assetto Corsa.
--
-- Left half is the panel exactly as CSP would draw it, at the size the
-- manifest asks for. Right half is the harness: what telemetry to feed it,
-- the app's own settings window, the harness's settings, and a log of anything
-- the emulation could not answer.
--
-- Nothing here is shipped to the game. The only file this shares with the real
-- app is `ac_pro_engineer.lua` itself, loaded from its own directory so what is
-- on screen is what would be installed.

local csp = require('csp')
local sim = require('sim')
local config = require('config')

local app = {
  loaded = false,
  error = nil,
  path = nil,
}

local harness = {
  test = false,
  appStopped = false,
  settingsOpen = false,
  engineerOpen = true,
  telemetryOpen = false,
  statusOpen = false,
  testFrames = 0,
  fps = 0,
  lastError = nil,
  errorCount = 0,
}

local S = config.values

-- Bound in love.load, once csp.install has put CSP's globals in place. It has
-- to be declared up here: the panels below close over this local, and a
-- declaration after them would leave them looking at a different variable.
local ui

-- Same reason: the keyboard handler feeds the console, and the console is
-- defined further down with the tabs it belongs to.
local consoleState = { text = '' }
local consoleHistory = {}
local runConsole

-- And again: `love.load` sets up a portrait, but the helpers that do it need
-- the window table, which is defined with the drawing code below. Declared
-- here so `love.load` closes over these locals rather than over three globals
-- that are nil at the point it runs.
local applyAppTab
local applyAppDeveloperMode
local enterPortrait

--- One window, alone, on a canvas exactly its size. `nil` for the harness
--- proper. Read by `love.draw`, which skips the control panel and the other
--- windows when it is set.
local portrait = nil

-- ---------------------------------------------------------------------------
-- Loading the overlay
-- ---------------------------------------------------------------------------

local function appDirectory()
  local source = love.filesystem.getSource()
  if source:sub(1, 1) ~= '/' then
    source = love.filesystem.getWorkingDirectory() .. '/' .. source
  end
  return source .. '/../ac_pro_engineer/'
end

--- Load (or reload) the overlay script. Any error is kept and shown rather
--- than thrown: a typo mid-edit should leave the harness standing so the next
--- save can fix it.
local function loadApp()
  local dir = appDirectory()
  app.path = dir .. 'ac_pro_engineer.lua'

  -- The panel requires `frame_layout` and a dozen `acpe.*` modules, and every
  -- one of them has to be read from the app's own directory rather than from
  -- the harness's. LÖVE mounts its own source as the filesystem root, so
  -- `package.path` cannot reach the app; a preload loader that resolves the
  -- module name to a path under `dir` can.
  --
  -- Cleared first, or F5 reloads the entry point and keeps every module the
  -- previous load left behind — which is not a reload, and would make editing
  -- a module look like editing had no effect.
  for name in pairs(package.loaded) do
    if name == 'frame_layout' or name:match('^acpe') then
      package.loaded[name] = nil
    end
  end
  setmetatable(package.preload, {
    __index = function(_, name)
      if name ~= 'frame_layout' and not name:match('^acpe') then return nil end
      return function()
        local file = dir .. name:gsub('%.', '/') .. '.lua'
        local chunk, err = loadfile(file)
        if chunk == nil then error(err, 0) end
        return chunk()
      end
    end,
  })

  _G.script = {}
  -- A panel that does not load is the worst outcome there is — every window
  -- draws the error instead of the panel — and it used to be the one the
  -- harness stayed silent about. `--test` counted errors thrown by `update`
  -- and by the draw calls, and a script that never got as far as defining
  -- them threw none of those, so it printed "0 errors" and exited 0 while the
  -- window on screen showed the failure. Load failures are errors.
  local chunk, err = loadfile(app.path)
  if chunk == nil then
    app.loaded, app.error = false, err
    harness.lastError = 'load: ' .. tostring(err)
    harness.errorCount = harness.errorCount + 1
    return
  end

  local ok, runError = pcall(chunk)
  if not ok then
    app.loaded, app.error = false, tostring(runError)
    harness.lastError = 'load: ' .. tostring(runError)
    harness.errorCount = harness.errorCount + 1
    return
  end

  app.loaded, app.error = true, nil

  -- The icon CSP puts in the title bar and the sidebar. It lives outside the
  -- harness's own directory, so it is read as bytes rather than mounted.
  if app.icon == nil then
    local handle = io.open(dir .. 'icon.png', 'rb')
    if handle ~= nil then
      local bytes = handle:read('*a')
      handle:close()
      local iconOk, image = pcall(function()
        return love.graphics.newImage(love.filesystem.newFileData(bytes, 'icon.png'))
      end)
      if iconOk then app.icon = image end
    end
  end

  csp.log('loaded ' .. app.path)
end

-- ---------------------------------------------------------------------------
-- love callbacks
-- ---------------------------------------------------------------------------

function love.load(args)
  config.load()
  local proceed, message, testMode = config.applyArguments(args or {})
  if not proceed then
    print(message)
    love.event.quit(0)
    return
  end
  S = config.values
  harness.test = testMode or false
  harness.settingsOpen = S.settingsOpen
  harness.engineerOpen = S.engineerOpen
  harness.telemetryOpen = S.telemetryOpen
  harness.statusOpen = S.statusOpen

  -- A portrait gets storage that remembers nothing, so every picture is taken
  -- from the panel's defaults regardless of what the last run left behind.
  --
  -- An if, not `cond and false or name`: that idiom yields the *fallback*
  -- whenever the middle value is false, so it handed every portrait the real
  -- settings file and the run that photographed the Dev tab left developer
  -- mode on in every picture after it.
  local storageFile = 'app-settings.lua'
  -- The panel keeps a settings file of its own as well as using `ac.storage`.
  -- A portrait gets a fresh directory for both, emptied on the way in, so the
  -- pictures are taken from the panel's defaults however the last run left it.
  local settingsDir = nil
  if config.portrait ~= nil then
    storageFile = false
    settingsDir = '/tmp/acpe-portrait-settings'
    os.execute('rm -rf ' .. settingsDir .. ' && mkdir -p ' .. settingsDir)
  end
  csp.install(function() return sim.frame end, storageFile, settingsDir)
  ui = _G.ui
  harness.titleFont = love.graphics.newFont(12)
  harness.statusFont = love.graphics.newFont(11)
  csp.setScale(S.uiScale)
  csp.selectTab('harness', S.tab)
  loadApp()

  applyAppTab(config.appTab)
  applyAppDeveloperMode(config.appDev)
  enterPortrait(config.portrait)

  love.window.setTitle('AC Pro Engineer — overlay harness')
  love.keyboard.setKeyRepeat(true)
end

function love.textinput(character)
  csp.textInput(character)
end

function love.keypressed(key)
  if csp.focusedInput ~= nil then
    if key == 'backspace' then
      csp.inputBackspace()
      return
    elseif key == 'return' or key == 'kpenter' then
      runConsole(consoleState.text)
      consoleState.text = ''
      return
    elseif key == 'escape' then
      csp.focusedInput = nil
      return
    end
    -- Everything else while typing is text, not a shortcut.
    if #key == 1 then return end
  end

  if key == 'f5' then
    loadApp()
  elseif key == 'space' then
    S.paused = not S.paused
    config.save()
  elseif key == 'f2' then
    harness.settingsOpen = not harness.settingsOpen
  elseif key == 'escape' then
    love.event.quit(0)
  end
end

function love.mousepressed() csp.input.pressed = true; csp.input.down = true end
function love.wheelmoved(_, y) csp.wheel = y end
function love.mousereleased() csp.input.released = true; csp.input.down = false end

function love.update(dt)
  csp.input.x, csp.input.y = love.mouse.getPosition()
  harness.fps = love.timer.getFPS()

  if not S.paused and not harness.appStopped then
    sim.update(dt, S.source, { speed = S.simSpeed, path = S.shmPath })
  end

  if app.loaded and _G.script.update then
    local ok, err = pcall(_G.script.update, dt)
    if not ok then
      harness.lastError = 'update: ' .. tostring(err)
      harness.errorCount = harness.errorCount + 1
    end
  end

  if harness.test then
    harness.testFrames = harness.testFrames + 1
    if harness.testFrames > 120 then
      print(string.format('harness: %d frames, %d errors%s',
        harness.testFrames, harness.errorCount,
        harness.errorCount > 0 and (' — ' .. tostring(harness.lastError)) or ''))
      love.event.quit(harness.errorCount > 0 and 1 or 0)
    end
  end
end

-- ---------------------------------------------------------------------------
-- Drawing: the panel
-- ---------------------------------------------------------------------------

local function drawBackdrop(x, y, w, h)
  if S.background == 'checker' then
    -- A checkerboard reads through the panel's translucent parts the way a
    -- track would, which flat grey does not.
    local step = 16
    for row = 0, math.ceil(h / step) - 1 do
      for col = 0, math.ceil(w / step) - 1 do
        local shade = ((row + col) % 2 == 0) and 0.22 or 0.16
        love.graphics.setColor(shade, shade, shade, 1)
        love.graphics.rectangle('fill', x + col * step, y + row * step,
          math.min(step, w - col * step), math.min(step, h - row * step))
      end
    end
  elseif S.background == 'green' then
    love.graphics.setColor(0.10, 0.35, 0.14, 1)
    love.graphics.rectangle('fill', x, y, w, h)
  else
    love.graphics.setColor(0.07, 0.075, 0.09, 1)
    love.graphics.rectangle('fill', x, y, w, h)
  end
end

local PADDING = 10

-- Every window the app declares in its manifest, plus the settings window CSP
-- opens from the gear. Each gets the same chrome, remembers where it was put,
-- and is dragged by its title bar — which is what happens in game, where the
-- driver arranges the windows once and CSP keeps the layout.
local windows = {
  {
    id = 'main',
    title = 'AC Pro Engineer',
    fn = 'windowMain',
    gear = true,
    size = function() return S.panelWidth, S.panelHeight end,
    resize = function(w, h) S.panelWidth, S.panelHeight = w, h end,
    minimum = { 200, 120 },   -- MIN_SIZE from the manifest
    isOpen = function() return true end,
  },
  {
    id = 'engineer',
    title = 'AC Pro Engineer — advice',
    fn = 'windowEngineer',
    closable = true,
    size = function() return S.engineerWidth, S.engineerHeight end,
    resize = function(w, h) S.engineerWidth, S.engineerHeight = w, h end,
    minimum = { 160, 70 },
    isOpen = function() return harness.engineerOpen end,
    onClose = function()
      harness.engineerOpen = false
      S.engineerOpen = false
      config.save()
    end,
  },
  {
    id = 'telemetry',
    title = 'AC Pro Engineer — telemetry',
    fn = 'windowTelemetry',
    closable = true,
    size = function() return S.telemetryWidth, S.telemetryHeight end,
    resize = function(w, h) S.telemetryWidth, S.telemetryHeight = w, h end,
    minimum = { 220, 160 },
    isOpen = function() return harness.telemetryOpen end,
    onClose = function()
      harness.telemetryOpen = false
      S.telemetryOpen = false
      config.save()
    end,
  },
  {
    id = 'status',
    title = 'AC Pro Engineer — status',
    fn = 'windowStatus',
    closable = true,
    size = function() return S.statusWidth, S.statusHeight end,
    resize = function(w, h) S.statusWidth, S.statusHeight = w, h end,
    minimum = { 220, 140 },
    isOpen = function() return harness.statusOpen end,
    onClose = function()
      harness.statusOpen = false
      S.statusOpen = false
      config.save()
    end,
  },
  {
    id = 'settings',
    title = 'AC Pro Engineer — settings',
    fn = 'windowSettings',
    closable = true,
    size = function() return S.settingsWidth, S.settingsHeight end,
    resize = function(w, h) S.settingsWidth, S.settingsHeight = w, h end,
    minimum = { 200, 140 },
    isOpen = function() return harness.settingsOpen end,
    onClose = function()
      harness.settingsOpen = false
      S.settingsOpen = false
      config.save()
    end,
  },
}

-- Back to front. Pressing anywhere in a window raises it, so the one being
-- worked on is the one on top.
local order = { 'main', 'engineer', 'telemetry', 'status', 'settings' }
local drag = { id = nil, offsetX = 0, offsetY = 0 }
local resize = { id = nil, offsetX = 0, offsetY = 0 }

-- The grip in the bottom-right corner, the same one CSP puts there.
local GRIP = 14

local function windowById(id)
  for _, window in ipairs(windows) do
    if window.id == id then return window end
  end
end

local function positionOf(id)
  return S[id .. 'X'] or 24, S[id .. 'Y'] or 46
end

-- ---------------------------------------------------------------------------
-- Portraits: one window, alone, sized to itself
--
-- The README needs a picture of each of the app's windows, and of each tab in
-- its settings — a screenshot of the whole harness is a picture of the
-- harness. These three set that up: pick the tab, unlock the developer tabs if
-- the picture is of one, and shrink the LÖVE window down to exactly the window
-- being photographed so `--shot` needs no cropping afterwards.
-- ---------------------------------------------------------------------------

--- Open the app's settings on a tab, given as `Tab` or `Tab/Subtab`.
---
--- The nested bars are named after their parent — `acpePanel`, `acpeLook`,
--- `acpeDev` — so the path maps onto them without a table of exceptions.
--- Selecting a bar that has not been drawn yet is fine: `csp.selectTab`
--- creates the state, and the first draw finds its label already chosen.
function applyAppTab(path)
  if path == nil then return end
  local parts = {}
  for part in tostring(path):gmatch('[^/]+') do parts[#parts + 1] = part end
  if parts[1] ~= nil then csp.selectTab('acpeSettings', parts[1]) end
  if parts[2] ~= nil then csp.selectTab('acpe' .. parts[1], parts[2]) end
end

--- Turn on the panel's *own* developer mode, which is what puts the Dev tab in
--- its settings window at all.
---
--- Reached through `package.loaded` rather than `require`: the app's modules
--- are resolved by the preload loader in `loadApp`, so the harness has no path
--- to them by name, but the instances the app just loaded are right there.
function applyAppDeveloperMode(on)
  if not on then return end
  local store = package.loaded['acpe.settings']
  if store == nil or store.values == nil then
    csp.log('--app-dev: the panel has no settings module loaded')
    return
  end
  store.values.devMode = true
end

--- Draw `id` alone, at the size it asks for, in a window that fits it exactly.
function enterPortrait(id)
  if id == nil then return end

  local window = windowById(id)
  if window == nil then
    print('--portrait: no window called ' .. tostring(id))
    love.event.quit(1)
    return
  end

  -- Nothing about a portrait is a preference, and writing any of it back would
  -- leave the next ordinary run drawing one window on a small black square.
  config.save = function() end

  if config.sizeOverride ~= nil and window.resize ~= nil then
    window.resize(config.sizeOverride[1], config.sizeOverride[2])
  end

  harness.engineerOpen = id == 'engineer'
  harness.telemetryOpen = id == 'telemetry'
  harness.statusOpen = id == 'status'
  harness.settingsOpen = id == 'settings'
  S.showFps = false

  local w, h = window.size()
  local margin = 18
  local width = w + margin * 2
  local height = h + csp.TITLE_HEIGHT + margin * 2
  portrait = { id = id, margin = margin, width = width, height = height }
  S[id .. 'X'], S[id .. 'Y'] = margin, margin
  love.window.setMode(width, height, {
    resizable = false,
    highdpi = true,
    vsync = 1,
  })
end

local function raise(id)
  for index, value in ipairs(order) do
    if value == id then
      table.remove(order, index)
      order[#order + 1] = id
      return
    end
  end
end

--- Is the pointer over any open window? Used to keep a click that lands on a
--- floating window from also reaching the control panel underneath it.
local function pointerOverWindow()
  local input = csp.input
  for _, id in ipairs(order) do
    local window = windowById(id)
    if window ~= nil and window.isOpen() then
      local x, y = positionOf(id)
      local w, h = window.size()
      h = h + csp.TITLE_HEIGHT
      if input.x >= x and input.x <= x + w and input.y >= y and input.y <= y + h then
        return true
      end
    end
  end
  return false
end

--- Start a drag when the press lands on a title bar, clear of the chrome
--- buttons on its right.
local function beginDrag(window, x, y, w)
  local input = csp.input
  if drag.id ~= nil or not input.pressed then return end

  local buttons = csp.TITLE_HEIGHT * 2
  local onTitle = input.x >= x and input.x <= x + w - buttons
    and input.y >= y and input.y <= y + csp.TITLE_HEIGHT
  if onTitle then
    drag.id = window.id
    drag.offsetX = input.x - x
    drag.offsetY = input.y - y
  end
end

--- Start a resize when the press lands on the grip in the bottom-right corner.
local function beginResize(window, x, y, w, h)
  local input = csp.input
  if resize.id ~= nil or drag.id ~= nil or not input.pressed then return end
  if window.resize == nil then return end

  local onGrip = input.x >= x + w - GRIP and input.x <= x + w
    and input.y >= y + h - GRIP and input.y <= y + h
  if onGrip then
    resize.id = window.id
    resize.offsetX = x + w - input.x
    resize.offsetY = y + h - input.y
  end
end

--- Follow the pointer, and save the size once the button comes up.
local function updateResize()
  local input = csp.input
  if resize.id == nil then return end

  local window = windowById(resize.id)
  if window == nil then
    resize.id = nil
    return
  end

  if input.down then
    local x, y = positionOf(resize.id)
    local minimum = window.minimum or { 120, 80 }
    local w = math.max(minimum[1], math.floor(input.x + resize.offsetX - x))
    local h = math.max(minimum[2], math.floor(input.y + resize.offsetY - y - csp.TITLE_HEIGHT))
    window.resize(w, h)
  else
    config.save()
    resize.id = nil
  end
end

--- Follow the pointer, and save the position once the button comes up: the
--- layout is a setting, and losing it on every restart is the thing that makes
--- arranging windows feel like work.
local function updateDrag()
  local input = csp.input
  if drag.id == nil then return end

  if input.down then
    S[drag.id .. 'X'] = math.floor(input.x - drag.offsetX)
    S[drag.id .. 'Y'] = math.max(0, math.floor(input.y - drag.offsetY))
  else
    config.save()
    drag.id = nil
  end
end

local function drawWindow(window)
  if not window.isOpen() then return end

  local x, y = positionOf(window.id)
  local w, h = window.size()
  h = h + csp.TITLE_HEIGHT

  if window.id == 'main' then
    drawBackdrop(x, y, w, h)
  end

  local input = csp.input
  local inside = input.x >= x and input.x <= x + w and input.y >= y and input.y <= y + h
  if inside and input.pressed then raise(window.id) end

  local frame = csp.appFrame(x, y, w, h, {
    title = window.title,
    icon = app.icon,
    settings = window.gear,
    closable = window.closable,
    dragging = drag.id == window.id,
  })

  beginDrag(window, x, y, w)
  beginResize(window, x, y, w, h)

  -- The grip, drawn last so it sits over the contents.
  local hotGrip = csp.input.x >= x + w - GRIP and csp.input.x <= x + w
    and csp.input.y >= y + h - GRIP and csp.input.y <= y + h
  local gripAlpha = (hotGrip or resize.id == window.id) and 0.75 or 0.28
  love.graphics.setColor(0.62, 0.66, 0.72, gripAlpha)
  for line = 1, 3 do
    local offset = line * 4
    love.graphics.line(x + w - offset, y + h - 2, x + w - 2, y + h - offset)
  end

  if frame.settings then
    harness.settingsOpen = not harness.settingsOpen
    S.settingsOpen = harness.settingsOpen
    raise('settings')
    config.save()
  end

  if frame.close and window.onClose then window.onClose() end

  if not app.loaded then
    love.graphics.setColor(1, 0.34, 0.34, 1)
    love.graphics.printf(app.error or 'app not loaded', frame.x + 8, frame.y + 8, w - 16)
    return
  end

  local draw = _G.script[window.fn]
  if draw == nil then
    csp.beginWindow(frame.x + PADDING, frame.y + PADDING,
      frame.width - PADDING * 2, frame.height - PADDING * 2)
    ui.textColored('this app has no ' .. window.fn, csp.colors.textDim)
    csp.endWindow()
    return
  end

  csp.beginWindow(frame.x + PADDING, frame.y + PADDING,
    frame.width - PADDING * 2, frame.height - PADDING * 2, window.id)
  local ok, err = pcall(draw, love.timer.getDelta())
  csp.endWindow()

  if not ok then
    harness.lastError = window.fn .. ': ' .. tostring(err)
    harness.errorCount = harness.errorCount + 1
    love.graphics.setColor(1, 0.34, 0.34, 1)
    love.graphics.printf(tostring(err), x + 8, y + h - 40, w - 16)
  end

  if S.showBounds then
    love.graphics.setColor(0.20, 0.72, 1.00, 0.6)
    love.graphics.rectangle('line', frame.x + PADDING, frame.y + PADDING,
      frame.width - PADDING * 2, frame.height - PADDING * 2)
  end
end

local function drawWindows()
  for _, id in ipairs(order) do
    local window = windowById(id)
    if window ~= nil then drawWindow(window) end
  end
end

-- ---------------------------------------------------------------------------
-- Drawing: the control panel
-- ---------------------------------------------------------------------------

local function slider(label, value, min, max, format, integer)
  ui.setNextItemWidth(ui.availableSpaceX())
  local v, changed = ui.slider(label, value, min, max, format, integer)
  return v, changed
end

local function telemetryTab()
  if not S.devMode then
    ui.textColored('READING', csp.colors.textDim)
    ui.textColored(string.format('source   %s', S.source), csp.colors.text)
    ui.textColored(string.format('sequence %d', sim.frame.sequence), csp.colors.text)
    ui.textColored(string.format('speed    %.0f km/h', sim.frame.speed_kmh), csp.colors.text)
    ui.textColored(string.format('advice   %d line(s)', sim.frame.message_count),
      csp.colors.text)
    ui.separator()
    ui.pushStyleColor(ui.StyleColor.Text, csp.colors.textDim)
    ui.textWrapped('Feeding the panel by hand is developer work: it can make the '
      .. 'overlay show things no real session would. The Dev tab, or --dev-mode, '
      .. 'unlocks it.')
    ui.popStyleColor()
    return
  end

  ui.textColored('SOURCE', csp.colors.textDim)
  for _, option in ipairs({
    { 'sim', 'Simulated lap' },
    { 'shm', 'Shared memory (real app)' },
    { 'manual', 'Manual — sliders only' },
  }) do
    if ui.radioButton(option[2], S.source == option[1]) then
      S.source = option[1]
      config.save()
    end
  end

  if S.source == 'shm' then
    ui.textColored(S.shmPath, csp.colors.textDim)
    if sim.shmError then
      ui.textColored(sim.shmError, { r = 1, g = 0.34, b = 0.34, mult = 1 })
    else
      ui.textColored('reading frames', { r = 0.35, g = 0.85, b = 0.45, mult = 1 })
    end
  elseif S.source == 'sim' then
    local speed, changed = slider('Rate', S.simSpeed, 0.25, 4, '%.2fx')
    if changed then S.simSpeed = speed; config.save() end
  end

  -- Freezing the sequence is exactly what a closed desktop app looks like from
  -- the panel's side, and it is the state most worth checking.
  if ui.button(harness.appStopped and 'Start desktop app' or 'Stop desktop app') then
    harness.appStopped = not harness.appStopped
  end
  ui.sameLine()
  if ui.button(S.paused and 'Resume  (space)' or 'Pause  (space)') then
    S.paused = not S.paused
    config.save()
  end
  ui.sameLine()
  if ui.button('Reload app  (F5)') then loadApp() end

  ui.separator()
  ui.textColored('FLAGS', csp.colors.textDim)
  for _, flag in ipairs({
    { 'Connected', sim.FLAG.CONNECTED },
    { 'Telemetry section', sim.FLAG.SHOW_TELEMETRY },
    { 'Engineer section', sim.FLAG.SHOW_ENGINEER },
    { 'Session section', sim.FLAG.SHOW_SESSION },
    { 'Lap timing section', sim.FLAG.SHOW_TIMING },
    { 'Fuel section', sim.FLAG.SHOW_FUEL },
    { 'Pit limiter', sim.FLAG.PIT_LIMITER },
    { 'Fuel warning', sim.FLAG.FUEL_WARNING },
  }) do
    if ui.checkbox(flag[1], sim.hasFlag(flag[2])) then
      sim.setFlag(flag[2], not sim.hasFlag(flag[2]))
    end
  end

  ui.separator()
  ui.textColored('VALUES', csp.colors.textDim)
  for _, control in ipairs(sim.controls) do
    local label, key, min, max, format, integer = control[1], control[2], control[3], control[4], control[5], control[6]
    local v, changed = slider(label, sim.frame[key], min, max, format, integer)
    if changed then
      sim.frame[key] = v
      S.source = 'manual'
    end
  end

  ui.separator()
  ui.textColored('CORNERS  (FL FR RL RR)', csp.colors.textDim)
  for _, control in ipairs(sim.cornerControls) do
    local label, key, min, max, format = control[1], control[2], control[3], control[4], control[5]
    local width = (ui.availableSpaceX() - 12) * 0.25
    ui.textColored(label, csp.colors.textDim)
    for corner = 0, 3 do
      if corner > 0 then ui.sameLine() end
      ui.setNextItemWidth(width)
      local v, changed = ui.slider('##' .. key .. corner, sim.frame[key][corner], min, max, format)
      if changed then
        sim.frame[key][corner] = v
        S.source = 'manual'
      end
    end
    ui.newLine()
  end

  ui.separator()
  ui.textColored('ENGINEER MESSAGES', csp.colors.textDim)
  for i = 0, 7 do
    local text = sim.frame.messages[i]
    if text ~= nil and text ~= '' then
      ui.textColored(string.format('%d  %s', i + 1, text), csp.colors.text)
    end
  end
  local count, changed = slider('Shown', sim.frame.message_count, 0, 8, '%.0f', true)
  if changed then sim.frame.message_count = count end
end

local function appSettingsTab()
  if not app.loaded then
    ui.textColored('app not loaded', { r = 1, g = 0.34, b = 0.34, mult = 1 })
    return
  end
  if _G.script.windowSettings == nil then
    ui.textColored('this app has no settings window', csp.colors.textDim)
    return
  end
  -- The overlay's own settings window, drawn by the overlay's own code. What
  -- is clicked here is what would be clicked in game.
  local ok, err = pcall(_G.script.windowSettings, love.timer.getDelta())
  if not ok then
    ui.textColored(tostring(err), { r = 1, g = 0.34, b = 0.34, mult = 1 })
  end
end

local function harnessSettingsTab()
  ui.textColored('PANEL', csp.colors.textDim)
  local scaleValue, scaleChanged = slider('Font scale', S.uiScale, 0.6, 2.0, '%.2f')
  if scaleChanged then
    S.uiScale = scaleValue
    csp.setScale(scaleValue)
    config.save()
  end

  local w, wChanged = slider('Panel width', S.panelWidth, 180, 700, '%.0f px', true)
  if wChanged then S.panelWidth = w; config.save() end
  local h, hChanged = slider('Panel height', S.panelHeight, 120, 900, '%.0f px', true)
  if hChanged then S.panelHeight = h; config.save() end

  local ew, ewChanged = slider('Advice width', S.engineerWidth, 160, 700, '%.0f px', true)
  if ewChanged then S.engineerWidth = ew; config.save() end
  local eh, ehChanged = slider('Advice height', S.engineerHeight, 70, 600, '%.0f px', true)
  if ehChanged then S.engineerHeight = eh; config.save() end

  ui.separator()
  ui.textColored('WINDOWS', csp.colors.textDim)
  if ui.checkbox('Advice window', harness.engineerOpen) then
    harness.engineerOpen = not harness.engineerOpen
    S.engineerOpen = harness.engineerOpen
    config.save()
  end
  if ui.checkbox('Telemetry window', harness.telemetryOpen) then
    harness.telemetryOpen = not harness.telemetryOpen
    S.telemetryOpen = harness.telemetryOpen
    config.save()
  end
  if ui.checkbox('Status window', harness.statusOpen) then
    harness.statusOpen = not harness.statusOpen
    S.statusOpen = harness.statusOpen
    config.save()
  end
  if ui.checkbox('Settings window', harness.settingsOpen) then
    harness.settingsOpen = not harness.settingsOpen
    S.settingsOpen = harness.settingsOpen
    config.save()
  end
  ui.textColored('drag a title bar to move, a corner to resize', csp.colors.textDim)
  if ui.button('Reset layout') then
    for _, key in ipairs({
      'mainX', 'mainY', 'engineerX', 'engineerY', 'settingsX', 'settingsY',
      'telemetryX', 'telemetryY', 'statusX', 'statusY',
      'panelWidth', 'panelHeight', 'engineerWidth', 'engineerHeight',
      'settingsWidth', 'settingsHeight', 'telemetryWidth', 'telemetryHeight',
      'statusWidth', 'statusHeight',
    }) do
      S[key] = config.defaults[key]
    end
    config.save()
  end

  ui.separator()
  ui.textColored('BACKDROP', csp.colors.textDim)
  for _, option in ipairs({ 'dark', 'checker', 'green' }) do
    if ui.radioButton(option, S.background == option) then
      S.background = option
      config.save()
    end
  end

  ui.separator()
  if ui.checkbox('Outline content rectangle', S.showBounds) then
    S.showBounds = not S.showBounds
    config.save()
  end
  if ui.checkbox('Show frame rate', S.showFps) then
    S.showFps = not S.showFps
    config.save()
  end

  ui.separator()
  ui.textColored('Settings are saved to:', csp.colors.textDim)
  ui.textColored(love.filesystem.getSaveDirectory(), csp.colors.textDim)
  if ui.button('Reset harness settings') then
    config.reset()
    S = config.values
    csp.setScale(S.uiScale)
  end
end

--- Apply a typed command line. Everything `run.sh` accepts works here, which
--- is the point: one vocabulary, whether it is given at launch or mid-session.
function runConsole(line)
  local args = {}
  for word in tostring(line):gmatch('%S+') do args[#args + 1] = word end
  if #args == 0 then return end

  local before = S.uiScale
  local proceed, message = config.applyArguments(args)
  S = config.values
  harness.settingsOpen = S.settingsOpen
  harness.engineerOpen = S.engineerOpen
  harness.telemetryOpen = S.telemetryOpen
  harness.statusOpen = S.statusOpen
  if S.uiScale ~= before then csp.setScale(S.uiScale) end
  config.save()

  consoleHistory[#consoleHistory + 1] = '> ' .. line
  if not proceed and message ~= nil then
    for text in message:gmatch('[^\n]+') do
      consoleHistory[#consoleHistory + 1] = text
    end
  end
  while #consoleHistory > 40 do table.remove(consoleHistory, 1) end
end

local function advancedTab()
  ui.textColored('CONSOLE', csp.colors.textDim)
  csp.inputState = consoleState
  ui.inputTextBox('##console', consoleState, 'type --help and press enter')
  if ui.button('Run') then
    runConsole(consoleState.text)
    consoleState.text = ''
  end
  ui.sameLine()
  if ui.button('Clear') then consoleHistory = {} end

  ui.separator()
  if ui.checkbox('Developer mode', S.devMode) then
    S.devMode = not S.devMode
    config.save()
  end
  ui.pushStyleColor(ui.StyleColor.Text, csp.colors.textDim)
  ui.textWrapped('unlocks the simulation controls on the Telemetry tab')
  ui.popStyleColor()

  ui.separator()
  ui.textColored('OUTPUT', csp.colors.textDim)
  local first = math.max(1, #consoleHistory - 24)
  for i = first, #consoleHistory do
    ui.textColored(consoleHistory[i], csp.colors.textDim)
  end
end

local function logTab()
  ui.textColored('APP', csp.colors.textDim)
  ui.textColored(app.path or '-', csp.colors.text)
  ui.textColored(app.loaded and 'loaded' or ('failed: ' .. tostring(app.error)),
    app.loaded and { r = 0.35, g = 0.85, b = 0.45, mult = 1 } or { r = 1, g = 0.34, b = 0.34, mult = 1 })

  if harness.errorCount > 0 then
    ui.separator()
    ui.textColored(string.format('%d runtime errors', harness.errorCount),
      { r = 1, g = 0.34, b = 0.34, mult = 1 })
    ui.textColored(tostring(harness.lastError), csp.colors.textDim)
  end

  ui.separator()
  ui.textColored('CSP CALLS NOT EMULATED', csp.colors.textDim)
  local any = false
  for name, count in pairs(csp.unimplemented) do
    if count > 0 then
      any = true
      ui.textColored(string.format('ui.%s  x%d', name, count), csp.colors.text)
    end
  end
  if not any then
    ui.textColored('none — everything the app calls is implemented', csp.colors.textDim)
  end

  ui.separator()
  ui.textColored('MESSAGES', csp.colors.textDim)
  local first = math.max(1, #csp.messages - 12)
  for i = first, #csp.messages do
    ui.textColored(csp.messages[i], csp.colors.textDim)
  end
end

local function drawControlPanel(x, y, w, h)
  love.graphics.setColor(0.09, 0.095, 0.11, 1)
  love.graphics.rectangle('fill', x, y, w, h, 4, 4)

  csp.beginWindow(x + PADDING, y + PADDING, w - PADDING * 2, h - PADDING * 2, 'controls')
  ui.tabBar('harness', function()
    ui.tabItem('Telemetry', telemetryTab)
    ui.tabItem('App settings', appSettingsTab)
    ui.tabItem('Harness', harnessSettingsTab)
    ui.tabItem('Dev', advancedTab)
    ui.tabItem('Log', logTab)
  end)
  csp.endWindow()

  local selected = csp.selectedTab('harness')
  if selected and selected ~= S.tab then
    S.tab = selected
    config.save()
  end
end

--- Take the `--shot` screenshot, if one was asked for, and quit afterwards.
---
--- Three seconds in, so the tab bar has its labels, the simulation has moved
--- off its starting values, and a frozen feed has had time to time out. A
--- couple of frames after that before quitting, because `captureScreenshot`
--- hands the image over at the end of the frame rather than during it.
local function shoot()
  if config.shot == nil then return end

  -- A portrait resizes the window on the way in and the window manager takes a
  -- few frames to agree, while drawing carries on regardless. Counting from
  -- the first frame caught the window mid-resize about one batch run in ten
  -- and wrote a picture two thirds the size, cropped on two edges — and
  -- nothing failed, because a cropped PNG is still a PNG.
  if portrait ~= nil then
    harness.waited = (harness.waited or 0) + 1
    if love.graphics.getWidth() ~= portrait.width
      or love.graphics.getHeight() ~= portrait.height then
      if harness.waited > 900 then
        print(string.format('--portrait: asked for %dx%d, the window is %dx%d',
          portrait.width, portrait.height,
          love.graphics.getWidth(), love.graphics.getHeight()))
        love.event.quit(1)
      end
      return
    end
  end

  harness.shotFrames = (harness.shotFrames or 0) + 1
  if harness.shotFrames == 180 then
    love.graphics.captureScreenshot(config.shot)
  elseif harness.shotFrames > 190 then
    print('screenshot: ' .. love.filesystem.getSaveDirectory() .. '/' .. config.shot)
    love.event.quit(0)
  end
end

function love.draw()
  love.graphics.clear(0.05, 0.05, 0.06)

  if portrait ~= nil then
    -- One window, nothing else: no control panel, no other windows, and the
    -- pointer parked off-screen so a stray hover does not light a widget up in
    -- the picture.
    csp.input.x, csp.input.y = -1000, -1000
    local window = windowById(portrait.id)
    if window ~= nil then
      -- `drawWindow` lays the backdrop for the main panel itself; every other
      -- window is drawn straight onto whatever is behind it, which in a
      -- portrait is nothing.
      if portrait.id ~= 'main' then
        local w, h = window.size()
        drawBackdrop(portrait.margin, portrait.margin, w, h + csp.TITLE_HEIGHT)
      end
      drawWindow(window)
    end
    shoot()
    return
  end

  -- The control panel is the backdrop the windows float over, so it is drawn
  -- first — and while it is drawn the pointer is moved out of the way if it is
  -- over a window, so a click on a window does not also press a slider behind
  -- it.
  local controlW = 340
  local controlX = love.graphics.getWidth() - controlW - 24

  local overWindow = pointerOverWindow()
  local savedX, savedY = csp.input.x, csp.input.y
  if overWindow then csp.input.x, csp.input.y = -1000, -1000 end
  drawControlPanel(controlX, 24, controlW, love.graphics.getHeight() - 48)
  csp.input.x, csp.input.y = savedX, savedY

  drawWindows()
  updateDrag()
  updateResize()

  if S.showFps then
    love.graphics.setColor(0.45, 0.48, 0.52, 1)
    love.graphics.setFont(harness.statusFont)
    love.graphics.print(string.format('%d fps   %s   seq %d   %s',
      harness.fps, S.source, sim.frame.sequence, S.paused and 'paused' or 'running'), 24, 8)
  end

  -- A screenshot of the harness itself, for a README or a bug report.
  shoot()

  -- Input edges last exactly one frame, and the frame ends here.
  csp.input.pressed = false
  csp.input.released = false
  csp.wheel = 0
  if not csp.input.down then csp.input.activeId = nil end
end
