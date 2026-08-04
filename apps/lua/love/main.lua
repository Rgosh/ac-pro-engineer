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

  -- The app does `require('frame_layout')`, and that file has to be read from
  -- the app's own directory rather than from the harness's.
  package.loaded['frame_layout'] = nil
  package.preload['frame_layout'] = function()
    local chunk, err = loadfile(dir .. 'frame_layout.lua')
    if chunk == nil then error(err, 0) end
    return chunk()
  end

  _G.script = {}
  local chunk, err = loadfile(app.path)
  if chunk == nil then
    app.loaded, app.error = false, err
    return
  end

  local ok, runError = pcall(chunk)
  if not ok then
    app.loaded, app.error = false, tostring(runError)
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

  csp.install(function() return sim.frame end, 'app-settings.lua')
  ui = _G.ui
  harness.titleFont = love.graphics.newFont(12)
  harness.statusFont = love.graphics.newFont(11)
  csp.setScale(S.uiScale)
  csp.selectTab('harness', S.tab)
  loadApp()

  love.window.setTitle('AC Pro Engineer — overlay harness')
  love.keyboard.setKeyRepeat(true)
end

function love.keypressed(key)
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
    isOpen = function() return true end,
  },
  {
    id = 'engineer',
    title = 'AC Pro Engineer — advice',
    fn = 'windowEngineer',
    closable = true,
    size = function() return 280, 150 end,
    isOpen = function() return harness.engineerOpen end,
    onClose = function()
      harness.engineerOpen = false
      S.engineerOpen = false
      config.save()
    end,
  },
  {
    id = 'settings',
    title = 'AC Pro Engineer — settings',
    fn = 'windowSettings',
    closable = true,
    size = function() return 264, 360 end,
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
local order = { 'main', 'engineer', 'settings' }
local drag = { id = nil, offsetX = 0, offsetY = 0 }

local function windowById(id)
  for _, window in ipairs(windows) do
    if window.id == id then return window end
  end
end

local function positionOf(id)
  return S[id .. 'X'] or 24, S[id .. 'Y'] or 46
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
    frame.width - PADDING * 2, frame.height - PADDING * 2)
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
  for i = 0, 3 do
    local text = sim.frame.messages[i]
    if text ~= nil and text ~= '' then
      ui.textColored(string.format('%d  %s', i + 1, text), csp.colors.text)
    end
  end
  local count, changed = slider('Shown', sim.frame.message_count, 0, 4, '%.0f', true)
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

  local w, wChanged = slider('Width', S.panelWidth, 180, 700, '%.0f px', true)
  if wChanged then S.panelWidth = w; config.save() end
  local h, hChanged = slider('Height', S.panelHeight, 120, 900, '%.0f px', true)
  if hChanged then S.panelHeight = h; config.save() end

  ui.separator()
  ui.textColored('WINDOWS', csp.colors.textDim)
  if ui.checkbox('Advice window', harness.engineerOpen) then
    harness.engineerOpen = not harness.engineerOpen
    S.engineerOpen = harness.engineerOpen
    config.save()
  end
  if ui.checkbox('Settings window', harness.settingsOpen) then
    harness.settingsOpen = not harness.settingsOpen
    S.settingsOpen = harness.settingsOpen
    config.save()
  end
  ui.textColored('drag a title bar to move a window', csp.colors.textDim)
  if ui.button('Reset positions') then
    S.mainX, S.mainY = config.defaults.mainX, config.defaults.mainY
    S.engineerX, S.engineerY = config.defaults.engineerX, config.defaults.engineerY
    S.settingsX, S.settingsY = config.defaults.settingsX, config.defaults.settingsY
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

  csp.beginWindow(x + PADDING, y + PADDING, w - PADDING * 2, h - PADDING * 2)
  ui.tabBar('harness', function()
    ui.tabItem('Telemetry', telemetryTab)
    ui.tabItem('App settings', appSettingsTab)
    ui.tabItem('Harness', harnessSettingsTab)
    ui.tabItem('Log', logTab)
  end)
  csp.endWindow()

  local selected = csp.selectedTab('harness')
  if selected and selected ~= S.tab then
    S.tab = selected
    config.save()
  end
end

function love.draw()
  love.graphics.clear(0.05, 0.05, 0.06)

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

  if S.showFps then
    love.graphics.setColor(0.45, 0.48, 0.52, 1)
    love.graphics.setFont(harness.statusFont)
    love.graphics.print(string.format('%d fps   %s   seq %d   %s',
      harness.fps, S.source, sim.frame.sequence, S.paused and 'paused' or 'running'), 24, 8)
  end

  -- A screenshot of the harness itself, for a README or a bug report. Taken
  -- three seconds in, so the tab bar has its labels, the simulation has moved
  -- off its starting values, and a frozen feed has had time to time out.
  if config.shot ~= nil then
    harness.shotFrames = (harness.shotFrames or 0) + 1
    if harness.shotFrames == 180 then
      love.graphics.captureScreenshot(config.shot)
    elseif harness.shotFrames > 190 then
      print('screenshot: ' .. love.filesystem.getSaveDirectory() .. '/' .. config.shot)
      love.event.quit(0)
    end
  end

  -- Input edges last exactly one frame, and the frame ends here.
  csp.input.pressed = false
  csp.input.released = false
  if not csp.input.down then csp.input.activeId = nil end
end
