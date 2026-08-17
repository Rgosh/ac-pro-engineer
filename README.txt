========================================
       PRO ENGINEER v0.4.0
========================================

Telemetry and race engineering for Assetto Corsa and Assetto Corsa
Competizione, with an in-game overlay for Assetto Corsa — the panel is a
Custom Shaders Patch app, and CSP is an AC mod.

Pick the game on the launcher: GAME: < ... >. What each one can and
cannot measure is listed there, and in README.md next to this file.

This archive holds both builds:

  ac_pro_engineer.exe    Windows
  ac_pro_engineer        Linux / Steam Deck
  shm-bridge.exe         Linux only — see below
  overlay/               the in-game panel, for a manual install


[ WINDOWS ]

Run ac_pro_engineer.exe. There is nothing to install.

Start Assetto Corsa as well — the application reads the game's shared
memory, so with no session running it sits on its launcher screen.

The in-game panel installs itself into Assetto Corsa the first time the
application starts. Then, in game: CSP's app sidebar -> enable
"Pro Engineer". If the automatic install cannot work (a game folder
it may not write to, an install in an unusual place, a second copy of
AC), the panel is also in the overlay\ folder here — drop the
ac_pro_engineer folder into:

    <Assetto Corsa>\apps\lua\

The panel needs Custom Shaders Patch. It does not need shm-bridge.exe:
on Windows the application creates the shared mapping itself. The bridge
is a Linux-only piece.


[ LINUX & STEAM DECK ]

Run ./ac_pro_engineer — the native Linux binary, not the .exe.

Assetto Corsa itself runs under Proton, so its telemetry lives inside the
game's prefix. shm-bridge.exe passes it out and must be running for the
application to see anything. Keep it next to the binary; the launcher
card reports its version and whether it fits, and [B] there fetches a
current one. protontricks is required to start it inside the prefix.

CSP needs Windows libraries that Proton ships only as stubs. If Content
Manager opens on a black screen or the game crashes when a Lua script
runs, that is what proton-setup.sh in this folder installs. The full
sequence is under "Linux / Steam Deck / Proton" in README.md.


[ IF YOUR ANTIVIRUS COMPLAINS ]

It is a false positive. Two reasons together trip the heuristics: the
binary is new and carries no publisher certificate, and reading another
process's memory is exactly what telemetry is. Add it to your
exclusions. The whole thing is open source and can be built from the
repository below.


[ LICENCE ]

Free software under the GNU AGPL v3 — use it, fork it, build on it. Change
it for yourself and nothing is asked of you at all. Pass your version on
and the one condition is that it stays open, with the project credited.

Keeping your own source closed, or selling a product with this code inside
it, needs written permission first: rgoshbbb@gmail.com.

LICENSE, NOTICE and LICENSING.md in the bundle have the detail.
shm-bridge.exe is a separate piece and stays under its own MIT licence.


[ LINKS & SUPPORT ]

Source and issues:
  https://github.com/Rgosh/ac-pro-engineer

Updates and reviews:
  https://www.overtake.gg/downloads/ac-pro-engineer-zero-lag-telemetry-setup-cloud-rust-powered.81695/

Enjoy your racing.
