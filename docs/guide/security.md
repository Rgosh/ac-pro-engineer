[← Guide](README.md) · [Main README](../../README.md)

# Security — why your antivirus might complain

This application reads Assetto Corsa's shared memory. That is what telemetry
tools do, and it is also a pattern some antivirus heuristics dislike.

- **It only reads.** It does not modify game files, inject code, or touch
  anything but its own overlay folder under `apps/lua/`.
- **It is entirely open source.** Audit it, or build it yourself from this
  repository.
- **The releases are built by GitHub Actions** from the tagged commit.
- If Windows Defender flags it, add the folder to your exclusions.

---
