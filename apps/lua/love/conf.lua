-- LÖVE window setup. The identity decides where settings are saved:
-- ~/.local/share/love/acpe-harness on Linux.

function love.conf(t)
  t.identity = 'acpe-harness'
  t.window.title = 'AC Pro Engineer — overlay harness'
  t.window.width = 1000
  t.window.height = 620
  t.window.minwidth = 720
  t.window.minheight = 480
  t.window.resizable = true
  t.window.vsync = 1
  t.window.highdpi = true

  -- Nothing here needs audio or physics, and not opening them keeps the
  -- harness startable on a machine with no sound device.
  t.modules.audio = false
  t.modules.physics = false
  t.modules.joystick = false
end
