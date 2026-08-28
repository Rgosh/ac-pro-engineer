# The upload form, filled in — the terminal

Everything the Overtake "Upload mod" page asks for, for **this** repository's
program. The window has its own copy of this in `RGProEngineer/docs/`, and the
two listings are separate pages that point at each other.

The description itself is `overtake-listing.bbcode` beside this; the post for a
new release is `overtake-0.4.5-post.txt`, which is plain text rather than
BBCode — see below.

| Field | What to put |
|---|---|
| **Title** | `Pro Engineer — telemetry and a race engineer for AC and ACC` |
| **Tag line** | `A race engineer that says what to change, and where. Free, open source, no account.` |
| **Type** | Link to an external site — `https://github.com/Rgosh/ac-pro-engineer/releases/latest` |
| **Version number** | `0.4.5` |
| **Description** | paste `overtake-listing.bbcode` |
| **Tags** | `telemetry, race engineer, acc, analysis, setup, tyres, overlay, csp, linux, windows, free, open source` |
| **Terms of Service** | *I am the sole creator* |
| **Additional information URL** | `https://proengineer.app/` |
| **Icon** | `~/Downloads/rg-pro-engineer-icon-512.png` — the same mark both products use |

**Link rather than an upload.** The release archives are built by CI on the
tag and published on GitHub with a checksum beside each; a copy attached here
would be a second thing to remember to replace, and the one people download
after an update would be the stale one.

## Updating it for a release

1. **The listing** — `overtake-listing.bbcode`, pasted over the old
   description. It carries a `NEW in vX` section; move the previous release's
   out of it rather than letting two accumulate.
2. **The version field** — the number alone, and it is what people see in the
   "updated" column.
3. **The post** — `overtake-0.4.5-post.txt` in the mod's own update thread.
   That is what reaches everybody who is watching it.

**The post is plain text and carries no pictures.** The listing is the page
somebody lands on and it is worth the screenshots; an update post is read by
people who already have the program, and every one on this thread so far has
been words. Paste it straight in.

## The description editor

The box is a rich-text editor, not a BBCode field. XenForo keeps a toggle for
the source view — the icon at the top right of the toolbar, the one that looks
like a page. Switch to it before pasting, or the tags arrive as literal text.

If there is no toggle, paste it anyway and check the preview: XenForo usually
converts BBCode on paste. What must not happen is `[IMG]` showing up as words.

## The pictures

Hotlinked from proengineer.app, which is under your control and stays up — so
a screenshot refreshed there is refreshed in the listing, and there is one copy
of each rather than one here and one on the forum's CDN.

Every one of them was checked live before this file was written:

| | |
|---|---|
| `Dashboard.png` | the terminal, mid-session |
| `Engineer.png` | the advice, grouped and ranked |
| `Launcher_Game.png` | the game picker, and what each one measures |
| `LAN.png` | sharing and watching — new in 0.4.5 |
| `Overlay_Main.png` | the in-game panel |
| `Analysis_Corners.png` | corner by corner against a reference lap |

If one 404s after a site rebuild, the deploy check would have caught it — it
refuses to upload a site that names a picture it does not have.
