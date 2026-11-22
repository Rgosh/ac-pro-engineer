[← Guide](README.md) · [Main README](../../README.md)

# The in-game overlay

The overlay is a **Custom Shaders Patch Lua app**. You need CSP installed; the
rest is automatic.

**It installs itself.** Every time the application starts it writes the panel
into `assettocorsa/assets/frontends/csp-panel/`, and rewrites it whenever it
differs from what the running build ships. So updating the application updates
the panel, with no step to forget. Enable **Pro Engineer** in CSP's app
sidebar once and it stays.

**It is reachable before the race.** The application publishes a frame from its
launcher screen and while Assetto Corsa has nothing in shared memory yet, so the
panel opens in the garage saying *waiting for the car* rather than claiming the
application is not running. Settings, versions and the link state are all there
while you wait.

**Five windows**, each a separate entry in CSP's sidebar, moved and sized
independently: the panel, the advice, the raw frame, the link state and the
settings. All five, and every tab of the settings window, are pictured with the
rest of the application's screens under [Every screen](screens.md#every-screen).

### The Linux bridge

`wineshm.exe` is a small Windows binary that runs inside the Proton prefix and
wraps the files in `/dev/shm` in the Win32 named mappings the game and CSP can
open. It is the only Linux-specific piece.

**Ask it whether the overlay can be seen from inside the prefix:**

```bash
protontricks-launch --appid 244210 wineshm.exe --verify
```

It makes exactly the call a CSP script makes, and prints the frame version, the
sequence counter and the application's version. If it opens, the panel can open
it too.

**Ask the desktop side which bridge is in play:**

```bash
cargo run -p ac_core --example bridge_probe
```

It reports the bridge on disk, the bridge running, and the version, protocol and
mapped size of each against what this build needs. The same report is a screen
in the application — `[C]` on **Settings → OVERLAY** — so none of this needs a
terminal or a checkout.

**Which bridge `[B]` fetches.** The one published with *this* release, and only
if there is none, the newest there is. The bridge is not republished every time,
so "newest published" is often older than the application asking for it — and a
bridge one release behind can be the one that cannot serve the frame. If no
bridge for this release has been published, the card says exactly that and gives
you the command to build one, rather than "nothing to fetch". The launcher's overlay card
shows the same verdict in one line, and **[B]** on that card downloads a
published bridge — verifying it before it replaces anything and keeping the old
one as `wineshm.exe.previous`.

> **⚠️ A bridge older than the frame maps too few bytes, and CSP silently refuses
> the mapping.** No error appears anywhere; the panel just waits forever beside a
> file that is right there. v0.3.5 grew the frame from 440 to 712 bytes, so a
> bridge from an earlier release will not serve it. Press **[B]**, or build one:
>
> ```bash
> cargo build --release --target x86_64-pc-windows-gnu   # in the wineshm checkout
> ```

---
