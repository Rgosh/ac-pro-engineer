========================================
       AC PRO ENGINEER v0.3.5
========================================

Telemetry, race engineering and an in-game overlay for Assetto Corsa.
Full documentation is in README.md next to this file.


[ WINDOWS ]

Run ac_pro_engineer.exe. There is nothing to install.

Start Assetto Corsa as well — the application reads the game's shared
memory, so with no session running it sits on its launcher screen.

The in-game panel installs itself into Assetto Corsa the first time the
application starts. Then, in game: CSP's app sidebar -> enable
"AC Pro Engineer". If the automatic install cannot work (a game folder
it may not write to, an install in an unusual place, a second copy of
AC), the panel is also in the overlay\ folder here — drop the
ac_pro_engineer folder into:

    <Assetto Corsa>\apps\lua\

The panel needs Custom Shaders Patch. It does not need shm-bridge.exe:
on Windows the application creates the shared mapping itself. The bridge
is a Linux-only piece.


[ LINUX & STEAM DECK ]

This archive is the Windows build. The Linux build ships as its own
archive and additionally needs shm-bridge.exe, which is run inside the
game's Proton prefix through protontricks — see the "Linux / Steam Deck
/ Proton" section of README.md.


[ IF YOUR ANTIVIRUS COMPLAINS ]

It is a false positive. Two reasons together trip the heuristics: the
binary is new and carries no publisher certificate, and reading another
process's memory is exactly what telemetry is. Add it to your
exclusions. The whole thing is open source and can be built from the
repository below.


[ LINKS & SUPPORT ]

Source and issues:
  https://github.com/Rgosh/ac-pro-engineer

Updates and reviews:
  https://www.overtake.gg/downloads/ac-pro-engineer-zero-lag-telemetry-setup-cloud-rust-powered.81695/

Enjoy your racing.
