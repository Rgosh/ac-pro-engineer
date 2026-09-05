# Releasing v0.4.5

Everything is prepared and committed; nothing is pushed, tagged, uploaded or
posted. This is the list, in the order it has to happen, with the reason for
the order where it matters.

Three repositories are involved: this one (public, the core and the terminal),
`RGProEngineer` (private, the window) and `proengineer.app` (the site, already
deployed — see step 6).

## Before you start

**A toolchain this list needs.** Step 2 cross-builds the window for Windows,
which wants the mingw compiler and the Rust target beside it. Without them
`package.sh` stops on `ring`, which is C:

```bash
sudo pacman -S --needed mingw-w64-gcc
rustup target add x86_64-pc-windows-gnu
```

If that machine cannot have them, the tag in step 2a builds both systems on
GitHub's runners instead and step 3 takes the two binaries from the draft
release — but the Overtake archive in step 4 is made by `package.sh` and has no
substitute.

**One thing only a person can do.** Nothing here has been run against a real
game: every check is a test, a harness or an offscreen render. Half an hour
with the wheel closes what none of that can:

1. Start Assetto Corsa with the panel installed. The panel should load and say
   `0.4.5`, and the dashboard's **front-left tyre should be on the left** —
   that is the bug this release was reported for.
2. Two machines, if you have them: `LAN`, `S` on one and `W` on the other, and
   check they find each other and that the map draws the other person's line.
   One machine works too — two copies find each other on loopback.
3. A car that is **not** a GT3 — a road car and a single-seater if you have
   both. The pressure target on ENGINEER → PRESSURES should be the class's own
   (21 psi for a Formula car, 27.5 for a GT3), the advice should agree with it
   rather than with 27.5, and a road car should never be told about a wing.

If any of them is wrong, stop: everything below is reversible only by
publishing again.

## 1. The public repository, and the tag

```bash
cd ~/projects/RaceEngineer && git push && git tag v0.4.5 && git push --tags
```

The tag is what starts the release workflow. `dist` builds both targets and
publishes `ac_tui-x86_64-*.tar.gz` / `.zip`, the `shm-bridge` zip and a
checksum for each. It refuses a tag whose number does not match the package
version, which is 0.4.5 in `Cargo.toml`, the panel, the manifest and the
`README.txt` banner.

**Wait for it to finish** before step 4: the terminal's in-app updater reads
that release, and a listing pointing at a release that is still building is a
download button that 404s.

## 2. The window's archives, and its update manifest

```bash
cd ~/projects/RGProEngineer && NOTES="Sharing a session: watch somebody else drive, on your own screens." ./package.sh && git push
```

Builds both systems, writes the two archives and the bundle for Overtake, and
— new in this release — writes `dist/download/` with the two bare binaries and
`rg-pro-engineer.json`, which is what an installed copy checks itself against.

**Run it after the core is pushed and tagged, not before.** The window is built
against `../RaceEngineer/core` as a path dependency, so whatever is checked out
there is what ships — and the class-aware pressure advice is in the core.

**The binaries in `dist/` from a previous session are stale.** Anything built
before the advice fixes is a 0.4.5 that does not contain them, and its
checksums are in a manifest that would then be wrong about what it names.

### 2a. If this machine cannot cross-build for Windows

```bash
cd ~/projects/RGProEngineer && git push && git tag v0.4.5 && git push --tags
```

`release.yml` builds Linux on 22.04 and Windows on a Windows runner, checks the
core out at the same tag, and attaches both binaries to a **draft** private
release. Download the two files, `sha256sum` them, and write
`dist/download/rg-pro-engineer.json` by hand in the shape `package.sh` writes —
`update.rs`'s `the_manifest_the_packager_writes_is_one_this_reads` is the test
that says what that shape is.

## 3. The window's update path onto the server

```bash
scp ~/projects/RGProEngineer/dist/download/rg-pro-engineer* mvps1:/var/www/proengineer.app/download/
```

**This is what makes the update real for the window.** Until it runs, the site
says 0.4.5 and serves the previous binaries, and no installed copy is offered
anything. `scp` sends them in alphabetical order, so the manifest — `.json` —
goes last, after the two binaries it names. That is the order it needs.

Check it landed:

```bash
curl -s https://proengineer.app/download/rg-pro-engineer.json | head -3
```

## 4. Overtake — two listings, two threads

Both products have a page there, and each has its own copy of the words.

| | The terminal | The window |
|---|---|---|
| Listing | `docs/overtake-listing.bbcode` | `RGProEngineer/docs/overtake-listing.bbcode` |
| Release post | `docs/overtake-0.4.5-post.txt` | `RGProEngineer/docs/overtake-0.4.5-post.txt` |
| The form | `docs/overtake-fields.md` | `RGProEngineer/docs/overtake-fields.md` |
| Version field | `0.4.5` | `0.4.5` |
| Attachment | none — it links to the GitHub release | `dist/rg-pro-engineer-0.4.5-windows-and-linux.zip` |

**The listing is BBCode; the update post is not.** Paste the listing into the
editor's **source view** or its tags arrive as literal words. The post is plain
text on purpose — an update post carries no pictures and no markup, which is
how every previous one on both threads was written.

And one reply that is owed, in the window's own thread:
`RGProEngineer/docs/overtake-reply-to-the-report.txt` — it thanks the report
that found the mirrored car and asks for the one screenshot that would settle
the question this release could not.

## 5. The site's repository

```bash
cd ~/projects/proengineer.app && git push
```

Only to make the repository match what is already served.

## 6. The site itself — already done

`./deploy.sh` ran on 28 August. Live and checked: the landing says
`running v0.4.5`, `/rg/` and `/technical/` carry the new sharing sections, the
screenshots are from this build, and `download/rg-pro-engineer*` survived the
upload as it is meant to.

Run it again only if you change a page.

## Why the site currently links to a page that does not exist

Reported on GitHub alongside the mirrored car: "clicking the version links
brings you into a github page that doesnt exist". The site was deployed on 28
August announcing v0.4.5, and the tag in step 1 was never pushed — so
`/releases/tag/v0.4.5` is a 404 and `download/rg-pro-engineer.json` still says
0.4.1, which is why no installed copy has been offered anything. Steps 1 and 3
are the whole of that fix; nothing on the site needs changing.

## What is still open after this release

* **The inner-minus-outer question from the report.** The mirrored car explains
  everything that could be seen on screen; it does not explain a negative `I-O`
  in the advice text, if there was one. The reply asks for the screenshot that
  decides it. Not a blocker — both fixes ship either way.
* **Competizione in the window.** One constant away in the code, and one driven
  stint away in fact: its brake and pad thresholds have never been read against
  real laps. `docs/plan-acc.md` §10.
* **Nothing has been run against a real game by anyone but you.** See the top
  of this file.
