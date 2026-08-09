-- The settings window.
--
-- CSP opens this from the app's own entry in the sidebar, and it is a separate
-- window from the panel: none of this runs while the overlay is being drawn, so
-- it is the one place in the app where a little work per frame costs nothing.
--
-- Tabs rather than one long column: the window is as tall as the driver left
-- it, and a list that runs past the bottom edge hides the half of the settings
-- nobody scrolled to.

local store = require('acpe.settings')
local settings = store.values
local DEFAULTS = store.DEFAULTS
local theme = require('acpe.theme')
local i18n = require('acpe.i18n')
local layout = require('acpe.layout')
local format = require('acpe.format')
local frame = require('acpe.frame')
local controls = require('acpe.controls')
local binds = require('acpe.binds')
local console = require('acpe.console')
local telemetry = require('acpe.windows.telemetry')
local status = require('acpe.windows.status')
local dev = require('acpe.windows.dev')
local changed = require('acpe.windows.changed')

local COLOR = theme.COLOR
local tr = i18n.tr
local say = layout.say
local pushRole = layout.pushRole
local row = layout.row
local contentWidth = layout.contentWidth
local shown = frame.shown

local storeActive, storeError = store.storage()
local paletteSelection = 1

return function(dt)
  -- The settings stay usable with the application closed — they are this
  -- machine's preferences, not the feed's — but the panel will not come back
  -- until it publishes, and saying so here saves a hunt through the checkboxes.
  if not frame.live() then
    pushRole('caption')
    ui.textColored('Panel hidden: AC Pro Engineer is not running', COLOR.warn)
    ui.popFont()
    ui.separator()
  end

  -- Four tabs rather than one long column: the window is as tall as the driver
  -- left it, and a list that runs past the bottom edge hides the half of the
  -- settings nobody scrolled to.
  local styles, colors = layout.push()
  ui.tabBar('acpeSettings', function()
    ui.tabItem(tr('Panel'), function()
      ui.tabBar('acpePanel', function()
        ui.tabItem(tr('Blocks'), function()
          controls.toggle('Speed and gear', 'showHeader')
          controls.toggle('RPM bar', 'showRpmBar')
          controls.toggle('Tyres and brakes', 'showTyres')
          controls.toggle('Lap timing', 'showTiming')
          controls.toggle('Fuel', 'showFuel')
          controls.toggle('Session', 'showSession')
          controls.toggle('Engineer advice', 'showEngineer')
          controls.toggle('Section captions', 'sectionLabels')
          controls.toggle('LIMITER badge', 'showLimiter')
          controls.toggle('Tell me when a newer panel is installed', 'showUpdateNotice')

          ui.separator()
          controls.toggle('One-line mode', 'hudMode')
          say('caption', 'speed, gear, delta, fuel — nothing else', COLOR.dim)

          ui.separator()
          controls.toggle('Shift light', 'shiftLight')
          local shift, shiftChanged = ui.slider('##shiftAt', settings.shiftAt * 100, 80, 100,
            'shift at  %.0f%% of the range')
          if shiftChanged then settings.shiftAt = shift / 100 end
        end)

        ui.tabItem(tr('Corners'), function()
          controls.toggle('Tyre temperature', 'showTyreTemp')
          controls.toggle('Inner / middle / outer', 'showTyreEdges')
          controls.toggle('Brake temperature', 'showBrakeTemp')
          controls.toggle('Wear', 'showWear')
          controls.toggle('Distance from target', 'showPressureTarget')

          ui.separator()
          say('caption', tr('PRESSURE'), COLOR.label)
          for decimals = 0, 2 do
            if ui.radioButton(string.format('%d decimal%s', decimals,
                decimals == 1 and '' or 's'), settings.pressureDecimals == decimals) then
              settings.pressureDecimals = decimals
              format.rebuild(shown)
            end
          end
        end)

        ui.tabItem(tr('Limits'), function()
          say('caption', tr('TYRE TEMPERATURE'), COLOR.label)
          local function limit(label, key, low, high, format)
            local value, changed = ui.slider('##' .. key, settings[key], low, high, format, true)
            if changed then settings[key] = value end
          end
          limit('cold', 'tyreCold', 40, 100, 'cold below  %.0f')
          limit('hot', 'tyreHot', 60, 130, 'working to  %.0f')
          limit('over', 'tyreOver', 80, 160, 'overheating past  %.0f')

          ui.separator()
          say('caption', tr('BRAKE TEMPERATURE'), COLOR.label)
          limit('bcold', 'brakeCold', 50, 400, 'cold below  %.0f')
          limit('bhot', 'brakeHot', 200, 900, 'working to  %.0f')
          limit('bover', 'brakeOver', 400, 1200, 'overheating past  %.0f')

          ui.separator()
          say('caption', tr('WEAR'), COLOR.label)
          limit('wwarn', 'wearWarn', 80, 100, 'good above  %.0f%%')
          limit('wbad', 'wearBad', 50, 99, 'worn below  %.0f%%')

          ui.separator()
          say('caption', 'the colours are yours; a slick at 95 is in its window', COLOR.dim)
          say('caption', 'and a hard at 95 is stone cold', COLOR.dim)
        end)

        ui.tabItem(tr('Fields'), function()
          say('caption', tr('TIMING'), COLOR.label)
          controls.toggle('Delta', 'showDelta')
          controls.toggle('Best lap', 'showBest')
          controls.toggle('Last lap', 'showLast')

          ui.separator()
          say('caption', tr('FUEL'), COLOR.label)
          controls.toggle('In the tank', 'showFuelLitres')
          controls.toggle('Laps left', 'showLapsLeft')
          controls.toggle('Per lap', 'showPerLap')

          ui.separator()
          say('caption', tr('SESSION'), COLOR.label)
          controls.toggle('Position', 'showPosition')
          controls.toggle('Lap number', 'showLapNumber')
          controls.toggle('Current lap', 'showCurrentLap')
          controls.toggle('Track conditions', 'showConditions')

          ui.separator()
          say('caption', tr('COLUMNS'), COLOR.label)
          for _, option in ipairs({ { 'automatic', 0 }, { 'two', 2 }, { 'three', 3 } }) do
            if ui.radioButton(option[1], settings.columnsPerRow == option[2]) then
              settings.columnsPerRow = option[2]
            end
          end
        end)

        -- Sections the application itself is suppressing. Without this the
        -- settings read as broken: a box is ticked and nothing appears.
        ui.tabItem(tr('State'), function()
          -- What the application is sending, flag by flag. "Nothing wrong" is
          -- an answer too, and an empty tab does not give it.
          say('caption', tr('THE APPLICATION IS SENDING'), COLOR.label)
          for _, entry in ipairs(frame.FLAG_NAMES) do
            local on = bit.band(shown.flags, entry[2]) ~= 0
            row(entry[1], on and 'on' or 'off', on and COLOR.good or COLOR.dim)
          end

          ui.separator()
          if not frame.live() then
            say('caption', 'the desktop application is not running', COLOR.warn)
          elseif settings.devIgnoreFlags then
            say('caption', 'developer mode is drawing every block anyway', COLOR.warn)
          else
            say('caption', 'a block needs its flag here and its switch in Blocks',
              COLOR.dim)
          end
        end)
      end)
    end)

    ui.tabItem(tr('Debrief'), function()
      say('caption', tr('LINES'), COLOR.label)
      -- Zero is a real setting here, unlike the live advice: a driver who
      -- wants the panel and not the post-lap summary sets this to nothing and
      -- the application stops publishing it at all.
      controls.slider('debriefLines', 'debriefLines', 0, frame.DEBRIEF_LINES,
        'draw up to  %.0f', true)
      say('caption', string.format(tr('the application is sending %d of %d'),
        shown.debrief_lap_count, frame.DEBRIEF_LAPS), COLOR.dim)

      ui.separator()
      controls.toggle('Show the lap time', 'debriefShowTime')
      controls.toggle('Compare with the lap before', 'debriefShowDelta')
      controls.toggle('Show sector times', 'debriefShowSectors')
      controls.toggle('Show what is left', 'debriefShowRemaining')
      controls.toggle('Show stint length', 'debriefShowStint')
      controls.toggle('Compare with your best lap', 'debriefCompareToBest')
      controls.toggle('Highlight a new lap', 'debriefHighlightNew')
      controls.toggle('Scroll when it does not fit', 'debriefScroll')
      controls.toggle('Colour the text by severity', 'debriefHighlight')
      -- Off, and a driver who paged back to compare two laps stays where they
      -- put themselves; on, and a lap ending brings them back to it.
      controls.toggle('Jump to the newest lap', 'debriefFollowNewest')

      ui.separator()
      say('caption', tr('WHEEL BUTTONS'), COLOR.label)
      -- Assigned here, stored by Assetto Corsa. These are sections in the
      -- game's own `controls.ini`, so a wheel button, a gamepad or a key all
      -- work and the panel never has to know which it was.
      if binds.available() then
        say('caption', tr('click to assign, then press the button'), COLOR.dim)
        binds.drawControls()
      else
        -- The actual reason, not a guess at it. This said "needs Custom Shaders
        -- Patch in a session" to people who were in a session with CSP, which
        -- names the one thing that is definitely not the problem and leaves
        -- them nowhere to go.
        say('caption', tr('no wheel bindings available'), COLOR.dim)
        say('caption', binds.reason(), COLOR.dim)
      end
    end)

    ui.tabItem(tr('Advice'), function()
      say('caption', tr('LINES'), COLOR.label)
      -- A slider, not a radio per line. Four radios were fine while the frame
      -- carried four slots; eight of them is a column of buttons where a
      -- number belongs.
      controls.slider('adviceLines', 'engineerLines', 1, frame.MESSAGE_SLOTS,
        'draw up to  %.0f', true)
      -- What the application actually sent, underneath. "I asked for eight and
      -- see three" is the application having three things to say, not the
      -- setting failing, and there is no way to tell those apart from here.
      say('caption', string.format(tr('the application is sending %d of %d'),
        shown.message_count, frame.MESSAGE_SLOTS), COLOR.dim)

      ui.separator()
      -- `say`, not pushRole + textColored: CSP draws widget text at its own
      -- font size, which cannot be scaled, so on a 4K screen this heading came
      -- out a third the size of everything around it.
      say('caption', tr('MARKER'), COLOR.label)
      for _, bullet in ipairs(layout.BULLET_NAMES) do
        if controls.radio(bullet, 'bullet' .. bullet, settings.engineerBullet == bullet) then
          settings.engineerBullet = bullet
        end
      end

      ui.separator()
      controls.slider('adviceScale', 'engineerScale', 0.6, 2.5, 'advice scale  %.2fx')

      controls.slider('adviceChars', 'engineerMaxChars', 20, 64, 'line limit  %.0f chars', true)

      ui.separator()
      say('caption', tr('SHOW'), COLOR.label)
      for index, name in ipairs({ 'everything', 'warnings and worse', 'critical only' }) do
        if controls.radio(name, 'sev' .. index, settings.engineerMinSeverity == index - 1) then
          settings.engineerMinSeverity = index - 1
        end
      end

      ui.separator()
      controls.toggle('Wrap long lines', 'engineerWrap')
      controls.toggle('Highlight advice', 'engineerHighlight')
      controls.toggle('Pick out new advice', 'engineerEmphasiseNew')
      controls.toggle('Space between lines', 'engineerSpacing')
      controls.slider('adviceGap', 'engineerLineGap', 0, 20, 'gap  %.0f px', true)
      controls.toggle('Rule between lines', 'engineerSeparator')
      controls.toggle('Show how many are hidden', 'engineerShowCount')

      controls.slider('advicePlate', 'engineerBackground', 0, 1, 'plate behind the advice  %.2f')

      ui.separator()
      say('caption', tr('FORMAT'), COLOR.label)
      controls.toggle('Upper case', 'engineerUppercase')
      controls.toggle('Number the lines', 'engineerNumbered')
      controls.toggle('Spell the severity', 'engineerSeverityWord')
    end)

    ui.tabItem(tr('Look'), function()
      ui.tabBar('acpeLook', function()
        ui.tabItem(tr('Screen'), function()
          -- Presets first: a panel that opens unreadable on a 4K screen is a
          -- panel nobody gets as far as configuring.
          for _, preset in ipairs(theme.PRESETS) do
            if ui.button(preset[1]) then
              for key, value in pairs(preset[2]) do settings[key] = value end
            end
            if preset[3] then ui.sameLine() end
          end

          ui.separator()
          controls.toggle('Grow with the window', 'autoScale')
          say('caption', settings.autoScale
            and 'size follows the window; width is the window'
            or 'fixed size and width', COLOR.dim)
        end)

        ui.tabItem(tr('Size'), function()
      -- Sliders, because these are the two numbers worth nudging while looking
      -- at the panel rather than picking from a list.
      controls.slider('scale', 'fontScale', 0.6, 3.0, 'text scale  %.2fx')

      controls.slider('width', 'contentWidth', 240, 900, 'content width  %.0f px', true)

      controls.slider('bar', 'barHeight', 3, 24, 'rev bar  %.0f px', true)

          ui.separator()
          for _, size in ipairs(layout.TEXT_SIZES) do
            if controls.radio(size, 'text' .. size, settings.textSize == size) then
              settings.textSize = size
            end
          end

          ui.separator()
          controls.toggle('VR mode', 'vrMode')
          say('caption', 'largest text, thicker bar, more spacing', COLOR.dim)
        end)

        ui.tabItem(tr('Colour'), function()
          say('caption', tr('ACCENT'), COLOR.label)
          for _, name in ipairs(theme.ACCENT_NAMES) do
            if controls.radio(name, 'accent' .. name, settings.accent == name) then
              settings.accent = name
            end
          end

          ui.separator()
          say('caption', tr('PALETTE'), COLOR.label)
          -- A colour button is a swatch, not a picker: clicking one does not
          -- open anything, which is why nothing happened. So the swatches
          -- choose, and one picker edits what was chosen.
          for index, entry in ipairs(theme.PALETTE) do
            ui.colorButton(entry[1] .. '##swatch', COLOR[entry[1]])
            ui.sameLine()
            if ui.button(entry[1] .. (paletteSelection == index and '  •' or '')) then
              paletteSelection = index
            end
            if index % 2 == 0 then ui.newLine() else ui.sameLine() end
          end
          ui.newLine()

          ui.separator()
          local chosen = theme.PALETTE[paletteSelection]
          if chosen ~= nil then
            local current = COLOR[chosen[1]]
            say('caption', 'editing ' .. chosen[1], COLOR.label)
            if ui.colorPicker('##palettePicker', current) then
              settings[chosen[2]] =
                string.format('%.3f,%.3f,%.3f', current.r, current.g, current.b)
            end
          end

          ui.separator()
          if ui.button('Default palette') then
            for _, entry in ipairs(theme.PALETTE) do settings[entry[2]] = DEFAULTS[entry[2]] end
          end

          ui.separator()
          local backing2, backingChanged2 = ui.slider('##backing2', settings.background, 0, 1,
            'backing  %.2f')
          if backingChanged2 then settings.background = backing2 end
          say('caption', 'the panel\'s own plate, for a bright sky', COLOR.dim)
        end)
      end)
    end)

    ui.tabItem(tr('Units'), function()
      if ui.checkbox('Celsius', settings.celsius) then
        settings.celsius = not settings.celsius
        format.rebuild(shown)
      end
      if ui.checkbox('PSI', settings.psi) then
        settings.psi = not settings.psi
        format.rebuild(shown)
      end
      if ui.checkbox('Miles per hour', settings.mph) then
        settings.mph = not settings.mph
        format.rebuild(shown)
      end
      if ui.checkbox('Gallons', settings.gallons) then
        settings.gallons = not settings.gallons
        format.rebuild(shown)
      end

      ui.separator()
      say('caption', tr('FORMAT'), COLOR.label)
      if ui.checkbox('Short lap times', settings.shortLapTimes) then
        settings.shortLapTimes = not settings.shortLapTimes
        format.rebuild(shown)
      end
      if ui.checkbox('Unit suffixes', settings.unitSuffix) then
        settings.unitSuffix = not settings.unitSuffix
        format.rebuild(shown)
      end

      pushRole('caption')
      ui.textColored(settings.celsius and 'temperatures in °C' or 'temperatures in °F', COLOR.dim)
      ui.textColored(settings.psi and 'pressures in psi' or 'pressures in bar', COLOR.dim)
      ui.popFont()

      ui.separator()
      -- Where they went, not just that they went. "Did it save" was a question
      -- the panel could not answer, and a driver whose settings kept reverting
      -- had nothing to look at.
      local file, fileError = store.file()
      if file ~= nil then
        say('caption', tr('settings are saved as you change them'), COLOR.dim)
        say('caption', file, COLOR.dim)
      elseif fileError ~= nil then
        say('caption', tr('could not write a settings file'), COLOR.warn)
        say('caption', fileError, COLOR.dim)
      end
      if storeActive then
        say('caption', tr("CSP's own storage is working too"), COLOR.dim)
      elseif file == nil then
        say('caption', tr('storage unavailable: settings last for this session'), COLOR.warn)
        if storeError ~= nil then
          say('caption', storeError, COLOR.dim)
        end
      end
      if ui.button(tr('Save now')) then store.saveAll() end
      ui.sameLine()
      if ui.button(tr('Reset to defaults')) then
        for key, value in pairs(DEFAULTS) do settings[key] = value end
        store.saveAll()
        format.rebuild(shown)
      end
      -- What the press did. A button that saves silently is a button people
      -- press twice and still do not believe.
      if store.report.ever then
        if store.report.failed > 0 then
          say('caption', string.format(tr('%d saved, %d would not stick'),
            store.report.written, store.report.failed), COLOR.bad)
        else
          say('caption', string.format(tr('%d saved'), store.report.written), COLOR.good)
        end
      end
    end)

    -- Everything that is not as it ships, in one list with a way back. Named
    -- with the count so it answers "have I changed anything" without being
    -- opened, which is most of what it is for.
    local changedCount = changed.count()
    ui.tabItem(changedCount > 0 and (tr('Changed') .. ' ' .. changedCount)
      or tr('Changed'), changed.body)

    -- The same two panels that have windows of their own, here as tabs: one
    -- window to open when the question is "what is going on", rather than
    -- three entries to hunt for in the sidebar.
    ui.tabItem(tr('Console'), console.draw)

    -- Red, and only here when asked for. The raw frame and the link state live
    -- under it: they answer questions a driver does not have, and four tabs
    -- nobody needs are four tabs in the way of the two they do.
    if settings.devMode then
      ui.pushStyleColor(ui.StyleColor.Tab, rgbm(0.45, 0.12, 0.14, 1))
      ui.pushStyleColor(ui.StyleColor.TabHovered, rgbm(0.75, 0.20, 0.22, 1))
      ui.pushStyleColor(ui.StyleColor.TabActive, rgbm(0.62, 0.16, 0.18, 1))
      ui.tabItem(tr('Dev'), function()
        ui.tabBar('acpeDev', function()
          ui.tabItem(tr('Switches'), dev.body)
          ui.tabItem(tr('Data'), telemetry.body)
          ui.tabItem(tr('Link'), status.body)
        end)
      end)
      ui.popStyleColor(3)
    end
  end)

  -- Anything the tabs changed is written here, once, at the end of the frame
  -- that changed it. `saveSettings` decides what changed itself now — the
  -- change detector that used to sit here consumed the difference into
  -- `lastSaved` before the save could look at it.
  store.save()

  layout.pop(styles, colors)
end
