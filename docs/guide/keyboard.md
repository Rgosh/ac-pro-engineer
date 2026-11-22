[← Guide](README.md) · [Main README](../../README.md)

# Keyboard

| Key | Where | What it does |
|:---:|:---:|:---|
| **1** – **9**, **0** | everywhere | Switch tabs — the digits, not the function keys; `0` is LAN |
| **Tab** / **Shift+Tab** | everywhere | Next / previous tab |
| **F1** / **?** | everywhere | Help for the current tab |
| **Esc** / **Q** | everywhere | Back to the launcher, then quit |
| **Ctrl+C** | everywhere | Back to the launcher, or quit from it |
| **Ctrl+L** | everywhere | Switch language (English / Russian) |
| **Ctrl+S** | everywhere | Save a screenshot of the current screen |
| **↑ / ↓** | lists | Move through laps, chapters, setups, settings |
| **← / →** | tabs with sub-tabs | Switch sub-tab; change a setting's value |
| **S** | Analysis | Save the selected lap |
| **L** | Analysis | Load a lap from disk |
| **C** | Analysis | Toggle the ghost comparison |
| **E** | Analysis | Export the selected lap as MoTeC CSV |
| **F** | Analysis | Corners: show only the losses over a tenth |
| **B** | Setup | Open / close the Setup Cloud browser |
| **D** | Setup | Download the selected setup, or open the browser |
| **PgUp / PgDn** | Setup | Scroll the details pane |
| **S** | LAN | Share this session, or stop |
| **W** | LAN | Watch somebody, or stop |
| **Enter** | LAN | Send my session to the machine the cursor is on |
| **O** | LAN | Everything off, keeping the addresses |
| **A** | LAN | Be findable on the network, or stop being |
| **A S D F G H** | Settings | Jump to a settings category |
| **I** / **U** | Settings → OVERLAY | Install / remove the in-game panel |
| **C** | Settings → OVERLAY | Overlay diagnostics — why the panel is blank |
| **O** / **H** | Launcher | Open the review page / hide that banner |

**All of these are defaults.** **Settings → KEYS `[H]`** rebinds any of them:
`ENTER` arms the capture, the next key you press becomes the binding, `DEL`
restores the default and `ESC` cancels. A key another action already holds is
refused with the name of the action holding it, rather than silently shadowing
it — and two keys on *different* tabs are not a clash, so `C` compares laps on
Analysis and opens the overlay diagnostics on Settings without either refusing
the other. `?` and `Q` stay fixed, because the help modal names them in places a binding
cannot reach.

Bindings are stored as text in `config.json` (`"f1"`, `"ctrl+s"`,
`"shift+tab"`, `"1"`), so you can also edit them in a text editor. Cyrillic
layouts work without switching: bind `s` and `ы` works too.

The hint at the bottom right of every tab is printed from your bindings, so it
always names the key that actually does the thing.

---
