# In-game overlay via a CSP Lua app

Design for replacing the overlay with a Lua app that draws inside Assetto
Corsa, while all computation stays in Rust.

> **Done, as of v0.3.5.** The panel ships, and step 8 below resolved the way it
> reads: `native_window.rs`, `openxr.rs`, `provider.rs`, `state.rs` and the
> whole `OverlayManager` are gone, along with the F10 and F11 bindings and the
> control centre in the terminal. Kept as the record of why the panel is shaped
> the way it is; read it in the past tense.

## Why the current overlay does not work

- `NativeWindowProvider` is `#[cfg(target_os = "windows")]` only, 422 lines of
  Win32. On Linux `OverlayManager::new` selects `None` and `render` is a no-op —
  there is no overlay at all on the platform this project otherwise supports.
- `OpenXrProvider` is a 24-line stub: `init` returns `Ok`, `render` does
  nothing. VR mode has never drawn anything.
- Even on Windows a separate always-on-top window is the wrong shape. It does
  not survive exclusive fullscreen, does not appear in VR, and is invisible to
  AC's own screenshot and replay systems.

Drawing inside the game is the only approach that solves all three, and AC has
exactly one supported way to do that: a Custom Shaders Patch Lua app.

## The idea, and why it is the right one

Rust computes, Lua only draws. The split matters because the two sides have
very different costs:

- Lua runs **on AC's render thread**. Every millisecond spent there is a
  millisecond off the frame budget. At 165 Hz the whole frame is 6 ms.
- LuaJIT allocates and garbage-collects. Anything that parses text, builds
  tables or formats strings per frame produces garbage that gets collected
  *during* a frame, which shows up as a stutter rather than as lower average
  FPS.

So the goal is: **the Lua app performs no computation, no parsing and no
allocation per frame** — it reads fields out of a struct at a fixed address and
passes them to ImGui.

## Architecture

```
  Rust: ac_pro_engineer            shared memory              CSP Lua app
  ┌──────────────────────┐      ┌────────────────┐      ┌──────────────────┐
  │ analyzer / engineer  │      │  OverlayFrame  │      │ script.update()  │
  │        ↓             │─────▶│  (packed C     │─────▶│   read fields    │
  │ pack once per tick   │write │   struct)      │ read │   draw ImGui     │
  └──────────────────────┘      └────────────────┘      └──────────────────┘
       ~60 Hz, 1 memcpy            fixed ~512 B            zero allocation
```

One writer, one reader, one struct. No serialisation on either side: the bytes
Rust writes are the bytes Lua reads, interpreted through the same layout.

### Why not the alternatives

| Approach | Per-frame cost in Lua | Verdict |
|---|---|---|
| **Shared struct** | pointer deref | chosen |
| JSON over file | parse ~2 KB, allocate tables | GC stutter every frame |
| TCP/UDP socket | syscall + parse | CSP Lua networking is restricted; same parsing cost |
| Lua computes from `ac.getCar()` | full analyzer in Lua | duplicates thousands of lines, on the render thread |

## The data contract

### Layout rules (verified against the CSP SDK)

CSP's `ac.StructItem` **reorders structure fields by default** for optimal
packing. The SDK says so explicitly:

> By default, CSP will reorder fields in your structures for optimal data
> packing. […] if you want for your script to exchange data with other
> programs, explicit order would work much better.

So the Lua side **must** call `ac.StructItem.explicitOrder(4, 4)` — without it
the field order is decided by CSP's packing algorithm and will not match a
`#[repr(C)]` Rust struct. This is the single most likely cause of a "reads
garbage" bug, and it is silent.

Rules for the shared struct:

1. Rust side is `#[repr(C)]`, Lua side declares `explicitOrder`.
2. Fixed-size scalars only — `i32`, `f32`, fixed `[u8; N]` for text. No
   pointers, no Rust `String`, no `Vec`.
3. Fields ordered largest-alignment first, so no padding differences can arise.
4. A `version: u32` first field. Lua refuses to draw if it does not recognise
   it, rather than misinterpreting a struct from a different release.

### Sketch

```rust
#[repr(C)]
pub struct OverlayFrame {
    version: u32,          // ACPE_OVERLAY_VERSION
    sequence: u32,         // torn-read detection, see below
    speed_kmh: f32,
    rpm: i32,
    gear: i32,
    fuel_laps_remaining: f32,
    delta_ms: i32,
    tyre_pressure: [f32; 4],
    tyre_temp: [f32; 4],
    brake_temp: [f32; 4],
    tyre_wear: [f32; 4],
    // ...
    message_count: u32,
    messages: [[u8; 64]; 4],   // engineer advice, UTF-8, NUL-padded
}
```

Roughly 512 bytes. At 60 Hz that is **30 KB/s** of writes and a single
`memcpy` per tick — against the ~9 MB/s the app already reads out of AC's own
shared memory. Immaterial.

### Torn reads

Same problem the app just fixed on the reading side, now as the writer: Lua
can read while Rust writes. Use the sequence-lock the codebase already
understands:

- Rust: `sequence += 1` (now odd) → write fields → `sequence += 1` (now even).
- Lua: read `sequence`, read fields, read `sequence` again; if it changed or is
  odd, keep the previous frame's values.

Skipping one frame at 60 Hz is invisible. A spliced frame is not — it produces
a flickering wrong number, which is exactly the class of bug the `packet_id`
work in v0.3.1 removed from the input side.

## Platform specifics

### Windows

The Rust app creates the mapping directly with `CreateFileMappingW`. This is
the same API `shm-bridge` already wraps in `FileMapping::new`, so the code
exists.

### Linux — the part that needs the bridge

The Rust app is a **native Linux process**. AC and CSP run under **Proton**. A
Linux process cannot create a Win32 named object that Wine will resolve.

The project already solved this in the other direction, and the same mechanism
inverts cleanly. `shm-bridge.exe` runs under Wine, opens a file under
`/dev/shm/`, and calls `CreateFileMappingW(name, file, size)` — a **file-backed**
named mapping. Both sides then see the same bytes:

- Wine side (AC, CSP Lua): a `Local\<name>` named mapping
- Linux side (our app): an ordinary file at `/dev/shm/<name>`, mmap-able

For the overlay, add one more name to `ACC_FILES` in the bridge. Rust opens
`/dev/shm/<name>` read-write and mmaps it; Lua opens the Windows name. Nothing
new has to be invented and nothing new has to be installed — the bridge is
already shipped and already running whenever the app reads telemetry on Linux.

### The name

`ac.readMemoryMappedFile` refuses names for scripts without IO permission
**unless the name starts with `AcTools.CSP.Limited.`**:

```lua
if not __allowIO__ and not string.startsWith(filename, 'AcTools.CSP.Limited.') then
  error('Script of this type can't access shared memory files', 2)
end
```

Using that prefix works whether or not the app is granted IO, so it removes an
entire category of "works on my machine" failure. Proposed:

```
AcTools.CSP.Limited.ACPE.v1
```

## The Lua app

Layout under the AC install:

```
assettocorsa/assets/frontends/csp-panel/
├── manifest.ini
└── ac_pro_engineer.lua
```

Skeleton:

```lua
local FRAME = ac.StructItem.combine({
  ac.StructItem.explicitOrder(4, 4),   -- REQUIRED, see layout rules
  version  = ac.StructItem.uint32(),
  sequence = ac.StructItem.uint32(),
  speed    = ac.StructItem.float(),
  -- ...
})

local frame = ac.readMemoryMappedFile('AcTools.CSP.Limited.ACPE.v1', FRAME)
local shown = {}   -- last consistent snapshot

function script.update(dt)
  local seq = frame.sequence
  if seq % 2 == 0 then                 -- writer not mid-update
    -- copy the few fields we draw
    shown.speed = frame.speed
    -- ...
    if frame.sequence ~= seq then return end   -- torn, keep previous
  end
end

function script.windowMain(dt)
  ui.text(string.format('%.0f km/h', shown.speed))
end
```

`script.update` runs every frame even when no window is visible, so keep the
copy small; `script.windowMain` only runs when the window is shown.

## Work breakdown

1. **`core/src/overlay/frame.rs`** — `OverlayFrame`, the version constant, and
   a `pack(&AppState) -> OverlayFrame`. Pure function, fully unit-testable
   without AC.
2. **`core/src/overlay/shared_writer.rs`** — create/open the mapping and write
   with the sequence lock. Windows via `CreateFileMappingW`, Linux via
   `/dev/shm/<name>`.
3. **`shm-bridge`** — one more entry in `ACC_FILES` with the overlay size.
4. **Wire into the tick** — pack and write once per `tick()`, next to the
   existing `overlay_manager.update`.
5. **The Lua app** — `manifest.ini`, the struct declaration, drawing.
6. **A layout conformance test** — see below. This is the one that stops the
   whole thing silently breaking.
7. **Packaging** — install the app into `apps/lua/`, using the `ac_paths`
   discovery added in v0.3.1.
8. **Retire `native_window.rs`** once the Lua app covers it, or keep it behind
   `OverlayMode::NativeDesktop` for people without CSP.

## Verification

The hard part is that the two sides are written in different languages and
compiled by different toolchains, so nothing catches a mismatch at build time.

- **Layout conformance test (Rust).** Assert `size_of::<OverlayFrame>()` and
  `offset_of!` for every field against constants written down in the test. Any
  reordering or padding change fails immediately. This is the same technique
  `ac_structs.rs` already uses to pin AC's own layout, and it caught a real bug
  there.
- **Generate the Lua declaration from the Rust struct** rather than maintaining
  it by hand — a small build step or a test that emits the `ac.StructItem`
  block and fails if the checked-in `.lua` differs. Two hand-maintained copies
  of one layout will drift; the only question is when.
- **Round-trip test.** Write a frame from Rust, read it back through the same
  layout, compare. Runs on both platforms in CI, no AC required.
- **Simulator.** Extend `tui/src/bin/simulator.rs` to also publish an overlay
  frame, so the Lua app can be developed and demonstrated with no game running.
- **In-game.** The only step that needs AC + CSP: install the app, confirm
  values match the TUI, and check the frame cost with CSP's own Lua profiler.

## Risks and open questions

- **CSP is required.** A plain AC install has no Lua apps at all. The native
  window path should stay for those users rather than being deleted.
- **`__allowIO__` for apps is unconfirmed.** The `AcTools.CSP.Limited.` prefix
  makes it moot, which is why the design uses it — but it should be confirmed
  in-game before building on top of it.
- **CSP is not installed on this machine**, and neither is AC (the Steam
  directories exist but are empty). Everything above is from the published CSP
  SDK sources, not from a running game. The layout rules and the two API
  functions are quoted from source and are solid; the app-permission question
  and the frame cost are not, and both need a real install to settle.
- **Wine file-backed mapping visibility.** The bridge proves this works for
  AC → app. The reverse direction (app writes, Wine process reads) uses the
  same mechanism, but has not been exercised.

## Sources

- [acc-lua-sdk `ac_extras_connectmmf.lua`](https://github.com/ac-custom-shaders-patch/acc-lua-sdk/blob/main/common/ac_extras_connectmmf.lua)
  — `ac.readMemoryMappedFile`, `ac.writeMemoryMappedFile`, the
  `AcTools.CSP.Limited.` permission check
- [acc-lua-sdk `ac_struct_item.lua`](https://github.com/ac-custom-shaders-patch/acc-lua-sdk/blob/main/common/ac_struct_item.lua)
  — field reordering and `explicitOrder`
- [acc-lua-sdk wiki: Lua apps](https://github.com/ac-custom-shaders-patch/acc-lua-sdk/wiki/Lua-apps)
  — app folder layout, `script.update` / `script.windowMain`
- `shm-bridge/src/file_mapping.rs` in this repository — the file-backed named
  mapping the Linux path depends on
