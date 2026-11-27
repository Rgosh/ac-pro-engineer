# The engineer's voices

```bash
tools/voicebank/build.sh --demo
```

One command makes every voice the race engineer speaks in, and nobody
records a word. What comes out is a folder of short clips per voice and
language, with a manifest; the programs only ever play those clips.

## What it does

1. **Fetches the models** in `models.json` — pinned revisions, each file held
   to its SHA-256 — and refuses any whose licence is not in the allowed list
   there (Apache-2.0, MIT, CC0, CC-BY), asking Hugging Face what the licence
   is rather than trusting the file.
2. **Makes the voices** in `voices.json`. A voice in the synthesiser
   (Kokoro-82M) is a table of style vectors; a weighted blend of several is a
   voice that belongs to no real person. The weights are written down, so the
   voice is the same on every build.
3. **Speaks every line** of `lines/<lang>.tsv` in its style — calm, report,
   urgent, praise, each its own pace — after `lexicon/<lang>.tsv` has
   respelled what the synthesiser would say wrong.
4. **Listens to every clip.** Whisper reads it back and the words are compared
   with the line. A clip that does not match is made again at another pace;
   a line that fails three times is listed and the build exits non-zero.
5. **Packs** each clip trimmed, at one loudness (EBU R128, −18 LUFS), as mono
   Ogg Vorbis at 16 kHz, with `manifest.json`: every clip's text, style,
   length and SHA-256, what recognition heard, and the licence of every model
   the voice was made from.

`--demo` also writes `demo-<voice>.mp3`: the lines one after another through
a radio — band-passed, compressed, with the button's click — for listening.
The programs put the radio on themselves at run time; the clips are clean.

## Whole phrases, never single words

A line that carries a number is spoken once per value, as the whole phrase:
`fuel.laps.{n}` with `n=1..40` is forty clips, "Fuel for one more laps" to
"Fuel for forty more laps". The program picks the clip; it never joins words.

The first build tried words — "six", "turn", "rear left" — and both halves
failed: the synthesiser reads a lone word flat, and recognition cannot tell
"four" from "for" without a sentence around it. Whole phrases sound like a
person and are checked in context. A few hundred clips of a few kilobytes is
still a voice pack of a few megabytes.

## Where things go

Everything large is under `~/.cache/pe-voicebank`: the Python environment,
the models (about 800 MB), and `out/`. Nothing large lands in the repository,
and the synthesiser and recogniser never reach a driver's machine — only the
clips do, which is why their dependencies do not travel with the programs.

## Adding a language

A voice per gender in `voices.json` (blended from voices the synthesiser has
for that language), `lines/<lang>.tsv`, and a context sentence for
recognition in `CONTEXT` in `voicebank.py`. A language the synthesiser has no
voices for needs a model with an allowed licence added to `models.json`
first; the build refuses anything else.
