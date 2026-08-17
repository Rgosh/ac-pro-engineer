-- The settings, in a file the panel owns.
--
-- `ac.storage` is CSP's answer to persistence and it is tried first, but it is
-- a black box from here: the panel assigns a value, and whether it survived to
-- the next session is not a question this side can ask. When it does not, every
-- checkbox in the settings window silently reverts, and there is nothing to
-- look at and nothing to fix.
--
-- So there is also a file. It is written on every change and read at startup,
-- it wins over storage when both have something to say — it is only ever
-- written by an actual change, so if the two disagree the file is the one that
-- reflects what the driver did — and it can be opened in a text editor, which
-- makes "did it save" a question with an answer.
--
-- Everything here is guarded. A panel that cannot write its settings is a
-- panel with settings that last for one session; a panel that throws while
-- trying is a panel that draws an error where the telemetry should be.

local M = {}

local FILE = 'ac_pro_engineer_overlay.lua'

--- The file library, if this Lua has one.
---
--- Read once, into a local, and every use goes through the two helpers below.
--- `pcall(io.open, path, mode)` looks guarded and is not: `io.open` is
--- evaluated before `pcall` ever runs, so on a build where `io` is nil that
--- line throws where nothing can catch it and every window draws the error
--- instead of the panel. It is the same shape as the `ui.Icons` crash that
--- stopped v0.3.3's panel loading at all.
local ioLib = type(io) == 'table' and io or nil

local function openFile(path, mode)
  if ioLib == nil or type(ioLib.open) ~= 'function' then return nil end
  local ok, handle = pcall(ioLib.open, path, mode)
  if not ok or handle == nil then return nil end
  return handle
end

--- Whether this Lua can keep a file at all. `false` is a working panel whose
--- settings last for the session, which is what it did before the file existed.
function M.available()
  return ioLib ~= nil and type(ioLib.open) == 'function'
end

--- Where the file might live, best first.
---
--- CSP's folder API when it is there — the enum names have moved between
--- builds, so each is tried and the first that yields a string is used. Then
--- `cfg/`, which exists in every Assetto Corsa install, is the user's own and
--- is not managed by this app's installer: a file written into the app folder
--- would sit alongside the ones the desktop side rewrites on every launch.
local function candidateDirectories()
  local dirs = {}

  if type(ac) == 'table' and type(ac.getFolder) == 'function' and type(ac.FolderID) == 'table' then
    for _, name in ipairs({ 'ExtCfgUser', 'Cfg', 'ACDocuments', 'Documents' }) do
      local id = ac.FolderID[name]
      if id ~= nil then
        local ok, dir = pcall(ac.getFolder, id)
        if ok and type(dir) == 'string' and dir ~= '' then
          dirs[#dirs + 1] = dir
        end
      end
    end
  end

  -- If CSP said where its configuration lives, that is the answer and the
  -- guesses below are not tried. Falling through to a relative path after a
  -- perfectly good absolute one is how a file ends up somewhere nobody looks —
  -- and, under the harnesses, in the middle of a checkout.
  if #dirs > 0 then return dirs end

  -- No folder API: relative to the process's working directory, which for CSP
  -- is the Assetto Corsa root. `cfg` exists in every install and belongs to
  -- the user; the app's own folder does not, because the desktop side rewrites
  -- it on every launch.
  dirs[#dirs + 1] = 'cfg'
  dirs[#dirs + 1] = '.'
  return dirs
end

local function join(dir, name)
  if dir:sub(-1) == '/' or dir:sub(-1) == '\\' then return dir .. name end
  return dir .. '/' .. name
end

--- Where the last successful read or write happened, for the settings window
--- to show. Nil until one of them has.
local chosen = nil

--- Turn one stored value into the text that reads back as itself.
---
--- Strings, numbers and booleans are all the settings hold, and anything else
--- is refused rather than written badly — a half-quoted table would make the
--- whole file unreadable and take every setting with it.
local function encode(value)
  local kind = type(value)
  if kind == 'string' then
    return string.format('%q', value)
  elseif kind == 'number' or kind == 'boolean' then
    return tostring(value)
  end
  return nil
end

--- Read one value back. Only the three types `encode` writes.
local function decode(text)
  if text == 'true' then return true end
  if text == 'false' then return false end
  local number = tonumber(text)
  if number ~= nil then return number end
  local quoted = text:match('^"(.*)"$')
  if quoted ~= nil then
    return (quoted:gsub('\\(.)', '%1'))
  end
  return nil
end

--- Read the file, if there is one to read.
---
--- Parsed line by line rather than executed. The panel wrote it, so running it
--- would probably be safe — but "probably" is doing a lot of work in a file
--- that sits in a folder anything can write to, and a pattern match cannot run
--- anything at all.
---
--- Returns a table of key to value, or nil.
function M.load()
  if not M.available() then return nil end
  for _, dir in ipairs(candidateDirectories()) do
    local path = join(dir, FILE)
    local handle = openFile(path, 'r')
    if handle ~= nil then
      local values = {}
      local read = pcall(function()
        for line in handle:lines() do
          local key, text = line:match('^%s*%[\"([%w_]+)\"%]%s*=%s*(.-),?%s*$')
          if key ~= nil then
            local value = decode(text)
            if value ~= nil then values[key] = value end
          end
        end
      end)
      pcall(function() handle:close() end)
      if read then
        chosen = path
        return values
      end
    end
  end
  return nil
end

--- Write every key in `keys` from `values`.
---
--- Whole file each time. Seventy short lines is nothing next to opening the
--- file, and rewriting it entirely means a key that was removed from the panel
--- does not linger, and a half-written file from a previous run cannot survive
--- as a mix of two states.
---
--- Returns the path written, or nil and why not.
function M.save(values, keys)
  local body = { '-- Pro Engineer overlay settings.', '-- Written by the panel; safe to edit or delete.', 'return {' }
  for _, key in ipairs(keys) do
    local text = encode(values[key])
    if text ~= nil then
      body[#body + 1] = string.format('  [%q] = %s,', key, text)
    end
  end
  body[#body + 1] = '}'
  local contents = table.concat(body, '\n') .. '\n'

  -- The directory that worked last time first, so a panel that has found a
  -- home does not go looking again on every change.
  local dirs = candidateDirectories()
  if chosen ~= nil then table.insert(dirs, 1, chosen:sub(1, #chosen - #FILE - 1)) end

  if not M.available() then return nil, 'this build of Lua has no file access' end

  local lastError = 'no writable folder'
  for _, dir in ipairs(dirs) do
    local path = join(dir, FILE)
    local handle = openFile(path, 'w')
    if handle ~= nil then
      local written = pcall(function() handle:write(contents) end)
      pcall(function() handle:close() end)
      if written then
        chosen = path
        return path
      end
      lastError = 'could not write ' .. path
    else
      lastError = 'could not open ' .. path
    end
  end
  return nil, lastError
end

--- The file the panel is using, or the one it would use.
---
--- Before anything has been read or written there is no `chosen` yet, and
--- returning nil there made the settings window say nothing at all on a fresh
--- install — which reads as "there is no file", the opposite of the truth.
function M.path()
  if chosen ~= nil then return chosen end
  if not M.available() then return nil end
  local dirs = candidateDirectories()
  if dirs[1] == nil then return nil end
  return join(dirs[1], FILE)
end

return M
