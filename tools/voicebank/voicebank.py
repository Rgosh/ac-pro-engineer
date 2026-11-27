"""Make the race engineer's voices, with nobody recording a word.

    tools/voicebank/build.sh                  # every voice, every language
    tools/voicebank/build.sh --lang en        # one language
    tools/voicebank/build.sh --demo           # and a listening demo per voice

What it does, in order — and why each step is here rather than in the program:

1. **Fetches the models** named in models.json, each pinned to a revision and
   held to its SHA-256. A model whose licence is not in the allowed list is
   refused before a byte is downloaded: the list decides, not whoever runs it.
2. **Makes the voices.** A voice in the synthesiser is a table of style
   vectors; a weighted blend of several is a new voice that belongs to no real
   person. The weights are in voices.json, so the voice is the same on every
   build and nobody's own voice is ever imitated.
3. **Speaks every line** in lines/<lang>.tsv in its style (pace per style in
   voices.json), after lexicon/<lang>.tsv has respelled the words the
   synthesiser would get wrong.
4. **Checks every clip by listening to it.** Recognition reads it back and the
   words are compared with the line; a clip that does not match is made again
   at another pace, and a line that fails five times is reported, never
   shipped quietly.
5. **Packs** each clip trimmed, at one loudness, as mono Ogg Vorbis at 16 kHz,
   with a manifest naming every clip's SHA-256 and every model's licence.

The synthesiser and the recogniser never reach a driver's machine — only the
clips do. That is what lets the program stay small and use next to no CPU,
and why the synthesiser's own dependencies do not travel with it.
"""

from __future__ import annotations

import argparse
import difflib
import hashlib
import json
import re
import shutil
import subprocess
import sys
import time
import urllib.error
import urllib.request
from pathlib import Path

HERE = Path(__file__).resolve().parent
CACHE = Path.home() / ".cache" / "pe-voicebank"
RATE = 24_000  # what the synthesiser produces
PACK_RATE = 16_000  # what is shipped: speech over a radio needs no more
TRIES = 5


def say(*what: object) -> None:
    print(*what, flush=True)


# ── 1. Models ────────────────────────────────────────────────────────────────


def sha256(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as f:
        for block in iter(lambda: f.read(1 << 20), b""):
            digest.update(block)
    return digest.hexdigest()


def licence_of(repo: str, revision: str) -> str:
    """The licence Hugging Face states for a model at a revision.

    **Asked once per revision and kept.** A pinned revision's licence cannot
    change, and asking on every run is what got the build answered "429 Too
    Many Requests" — so the answer is cached, and a refused request is tried
    again after a pause rather than ending the build."""
    known_path = CACHE / "licences.json"
    known = json.loads(known_path.read_text()) if known_path.exists() else {}
    key = f"{repo}@{revision}"
    if key in known:
        return known[key]
    url = f"https://huggingface.co/api/models/{repo}/revision/{revision}"
    for attempt in range(5):
        try:
            with urllib.request.urlopen(url, timeout=60) as answer:
                card = json.load(answer).get("cardData") or {}
            break
        except urllib.error.HTTPError as error:
            if error.code != 429 or attempt == 4:
                raise
            time.sleep(10 * (attempt + 1))
    licence = str(card.get("license", "")).lower()
    known[key] = licence
    known_path.parent.mkdir(parents=True, exist_ok=True)
    known_path.write_text(json.dumps(known, indent=2) + "\n")
    return licence


def fetch(models: dict) -> dict[str, Path]:
    allowed = set(models["allowed_licences"])
    where: dict[str, Path] = {}
    for name, model in models["models"].items():
        repo, revision = model["repo"], model["revision"]
        stated = licence_of(repo, revision)
        if stated not in allowed or stated != model["licence"]:
            sys.exit(
                f"{repo}@{revision[:8]} is licensed '{stated}', and models.json says "
                f"'{model['licence']}' — refusing it (allowed: {sorted(allowed)})"
            )
        folder = CACHE / "models" / f"{repo.replace('/', '--')}@{revision[:12]}"
        for file, want in model["files"].items():
            path = folder / file
            if not path.exists() or (want and sha256(path) != want):
                path.parent.mkdir(parents=True, exist_ok=True)
                url = f"https://huggingface.co/{repo}/resolve/{revision}/{file}"
                say(f"  fetching {repo}/{file}")
                part = path.with_suffix(path.suffix + ".part")
                with urllib.request.urlopen(url, timeout=600) as answer, part.open("wb") as out:
                    shutil.copyfileobj(answer, out, 1 << 20)
                part.replace(path)
            if want and sha256(path) != want:
                path.unlink()
                sys.exit(f"{repo}/{file} does not hash to the SHA-256 in models.json")
        say(f"  {name}: {repo}@{revision[:8]}, {stated}")
        where[name] = folder
    return where


# ── 2. Voices ────────────────────────────────────────────────────────────────


def blend(np, kokoro_dir: Path, weights: dict[str, float]):
    total = sum(weights.values())
    mixed = None
    for name, weight in weights.items():
        vector = np.fromfile(kokoro_dir / "voices" / f"{name}.bin", dtype=np.float32)
        vector = vector.reshape(-1, 1, 256) * (weight / total)
        mixed = vector if mixed is None else mixed + vector
    return mixed


# ── 3. Lines ─────────────────────────────────────────────────────────────────


def table(path: Path) -> list[list[str]]:
    rows = []
    for line in path.read_text(encoding="utf-8").splitlines():
        if not line.strip() or line.startswith("#"):
            continue
        rows.append([cell.strip() for cell in line.split("\t")])
    return rows


def spoken_value(kind: str, value: float) -> str:
    """A value as the engineer says it: 11 → "eleven", 1.4 → "one point four"."""
    if kind == "t":
        whole, tenth = divmod(round(value * 10), 10)
        return f"{spoken_number(whole)} point {spoken_number(tenth)}"
    return spoken_number(int(value))


def expanded(rows: list[list[str]]) -> list[tuple[str, str, str]]:
    """Every line, with a line per value where it carries one.

    **Whole phrases, never single words.** A word on its own has no
    sentence around it: the synthesiser reads "six" or "rear left" flat and
    unsure, and recognition cannot tell "four" from "for" without context —
    the first build proved both. So a line that carries a number is spoken
    once per value it can take, as the whole phrase, and the program picks
    the clip rather than joining words. Fuel for 1 to 40 laps and gaps of
    0.1 to 9.9 s are a few hundred clips of a few kilobytes each.

    The fourth column names the values: `n=1..40` (whole numbers) or
    `t=0.1..9.9` (tenths)."""
    out = []
    for row in rows:
        key, style, text = row[0], row[1], row[2]
        spec = row[3] if len(row) > 3 else ""
        if not spec:
            out.append((key, style, text))
            continue
        kind, span = spec.split("=")
        low, high = (float(x) for x in span.split(".."))
        step = 0.1 if kind == "t" else 1.0
        count = round((high - low) / step) + 1
        for k in range(count):
            value = low + k * step
            label = f"{value:.1f}" if kind == "t" else str(int(value))
            out.append((key.replace("{" + kind + "}", label), style,
                        text.replace("{" + kind + "}", spoken_value(kind, value))))
    return out


def respell(text: str, lexicon: list[list[str]]) -> str:
    for written, said in lexicon:
        text = re.sub(rf"\b{re.escape(written)}\b", said, text, flags=re.IGNORECASE)
    return text


# ── 4. Listening back ────────────────────────────────────────────────────────

ONES = "zero one two three four five six seven eight nine ten eleven twelve thirteen fourteen fifteen sixteen seventeen eighteen nineteen".split()
TENS = "_ _ twenty thirty forty fifty sixty seventy eighty ninety".split()
SOUNDS_ALIKE = {"for": "four", "to": "two", "too": "two", "won": "one", "ate": "eight", "oh": "zero",
               # The same word in American spelling, which is how recognition writes it.
               "tires": "tyres", "tire": "tyre", "curbs": "kerbs", "curb": "kerb"}


def spoken_number(n: int) -> str:
    if n < 20:
        return ONES[n]
    if n < 100:
        return TENS[n // 10] + ("" if n % 10 == 0 else " " + ONES[n % 10])
    return spoken_number(n // 100) + " hundred" + ("" if n % 100 == 0 else " " + spoken_number(n % 100))


def words(text: str) -> list[str]:
    text = text.lower().replace("-", " ")
    text = re.sub(r"(\d+)\.(\d+)", lambda m: f"{m[1]} point {' '.join(m[2])}", text)
    text = re.sub(r"\d+", lambda m: spoken_number(int(m[0])) if len(m[0]) < 4 else m[0], text)
    text = re.sub(r"[^a-z' ]+", " ", text).replace("'", "")
    return [SOUNDS_ALIKE.get(w, w) for w in text.split()]


def likeness(heard: str, meant: str) -> float:
    """How much of the line was heard: by words, or by letters when
    recognition only ran two words together ("Goodlap")."""
    a, b = words(heard), words(meant)
    by_words = difflib.SequenceMatcher(None, a, b).ratio()
    by_letters = difflib.SequenceMatcher(None, "".join(a), "".join(b)).ratio()
    return max(by_words, by_letters if by_letters > 0.97 else 0.0)


#: What a race engineer's radio is about, given to recognition as context, the
#: way a person listening knows the subject: without it "lap" is heard as
#: "lab" and "faster" as "foster". It biases which words are expected; it
#: cannot make a clip that says something else match.
CONTEXT = {
    "en": "Race engineer on the team radio: box this lap, stay out, yellow flag, "
          "fuel, laps, gap, seconds, tenths, front left, rear right, tyres, kerbs, "
          "turn, apex, brake, throttle, rear wing, personal best.",
}


# ── 5. Packing ───────────────────────────────────────────────────────────────


def trimmed(np, audio, floor_db: float = -45.0, pad: float = 0.04):
    level = 10 ** (floor_db / 20)
    loud = np.nonzero(np.abs(audio) > level)[0]
    if loud.size == 0:
        return audio
    margin = int(pad * RATE)
    return audio[max(0, loud[0] - margin) : loud[-1] + margin]


def encode(np, audio, out: Path) -> None:
    out.parent.mkdir(parents=True, exist_ok=True)
    peak = float(np.max(np.abs(audio))) or 1.0
    audio = (audio / peak * 0.5).astype(np.float32)
    subprocess.run(
        ["ffmpeg", "-v", "error", "-y", "-f", "f32le", "-ar", str(RATE), "-ac", "1", "-i", "-",
         "-af", f"loudnorm=I=-18:TP=-2:LRA=7,aresample={PACK_RATE}",
         "-c:a", "libvorbis", "-q:a", "-1", str(out)],
        input=audio.tobytes(), check=True,
    )


RADIO = (
    "highpass=f=300,lowpass=f=3400,acompressor=threshold=-20dB:ratio=4:attack=5:release=60,"
    "volume=1.6,alimiter=limit=0.9"
)


def demo(clips: list[Path], out: Path) -> None:
    """Every clip in a row, through a radio, with the press and release of
    the button — what a driver would hear. For listening, never shipped:
    the program puts the radio on at run time."""
    # Everything as the same raw samples first: the concat step joins files
    # of one format only, and an Ogg next to a WAV is two.
    raw = bytearray()
    pcm = ["-f", "s16le", "-ar", str(PACK_RATE), "-ac", "1", "-"]
    click = subprocess.run(["ffmpeg", "-v", "error", "-f", "lavfi", "-i",
                            f"anoisesrc=d=0.035:c=white:a=0.25:r={PACK_RATE}", *pcm],
                           capture_output=True, check=True).stdout
    gap = bytes(int(0.7 * PACK_RATE) * 2)
    for clip in clips:
        said = subprocess.run(["ffmpeg", "-v", "error", "-i", str(clip), *pcm],
                              capture_output=True, check=True).stdout
        raw += click + said + click + gap
    subprocess.run(["ffmpeg", "-v", "error", "-y", "-f", "s16le", "-ar", str(PACK_RATE), "-ac", "1",
                    "-i", "-", "-af", RADIO, "-ar", "22050", "-c:a", "libmp3lame", "-q:a", "4", str(out)],
                   input=bytes(raw), check=True)


# ── The run ──────────────────────────────────────────────────────────────────


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--lang", action="append", help="only these languages")
    parser.add_argument("--voice", action="append", help="only these voices (male, female)")
    parser.add_argument("--out", type=Path, default=CACHE / "out")
    parser.add_argument("--demo", action="store_true", help="also a listening demo per voice")
    parser.add_argument("--only", help="only lines whose key starts with this")
    args = parser.parse_args()

    import numpy as np
    from faster_whisper import WhisperModel
    from kokoro_onnx import Kokoro

    models = json.loads((HERE / "models.json").read_text())
    voices = json.loads((HERE / "voices.json").read_text())
    say("Models")
    where = fetch(models)

    styles = voices["styles"]
    wanted: dict[str, dict] = {}
    for gender, langs in voices["voices"].items():
        if args.voice and gender not in args.voice:
            continue
        for lang, voice in langs.items():
            if args.lang and lang not in args.lang:
                continue
            wanted[f"{gender}-{lang}"] = {"gender": gender, "lang": lang, **voice}

    # The blended voices, in the file shape the synthesiser loads.
    vectors = {name: blend(np, where["kokoro"], v["blend"]) for name, v in wanted.items()}
    bank = CACHE / "voices.npz"
    np.savez(bank, **vectors)
    tts = Kokoro(str(where["kokoro"] / "onnx" / "model.onnx"), str(bank))
    ears = WhisperModel(str(where["whisper"]), device="cpu", compute_type="int8")

    licences = {k: {"repo": m["repo"], "revision": m["revision"], "licence": m["licence"], "what": m["what"]}
                for k, m in models["models"].items()}
    report: list[str] = []
    for name, voice in wanted.items():
        lang = voice["lang"]
        lines = expanded(table(HERE / "lines" / f"{lang}.tsv"))
        lexicon_path = HERE / "lexicon" / f"{lang}.tsv"
        lexicon = table(lexicon_path) if lexicon_path.exists() else []
        folder = args.out / name
        clips: dict[str, dict] = {}
        # **--only remakes some lines and keeps the rest**; a whole build
        # starts the voice afresh, so a line taken out of the table does not
        # live on in the pack.
        if args.only and (folder / "manifest.json").exists():
            clips = json.loads((folder / "manifest.json").read_text())["clips"]
        elif folder.exists():
            shutil.rmtree(folder)
        say(f"\n{name}: {len(lines)} lines")
        started = time.time()
        for key, style, text in lines:
            if args.only and not key.startswith(args.only):
                continue
            # A pace of its own per line, from the line's own text, so it is
            # the same on every build (see "jitter" in voices.json).
            spread = voices.get("jitter", {}).get("fraction", 0.0)
            turn = int(hashlib.sha256(text.encode()).hexdigest(), 16) % 1000 / 1000 - 0.5
            pace = styles[style]["speed"] * (1.0 + turn * 2 * spread)
            spoken = respell(text, lexicon)
            best = None
            for attempt in range(TRIES):
                speed = pace * (1.0, 0.94, 1.06, 0.9, 1.1)[attempt]
                audio, rate = tts.create(spoken, voice=name, speed=speed, lang=voice["speaks"])
                assert rate == RATE, rate
                audio = trimmed(np, np.asarray(audio, dtype=np.float32))
                segments, _ = ears.transcribe(audio, language=lang, beam_size=5,
                                              condition_on_previous_text=False,
                                              initial_prompt=CONTEXT.get(lang))
                heard = " ".join(s.text for s in segments).strip()
                score = likeness(heard, text)
                if best is None or score > best[0]:
                    best = (score, audio, heard, speed)
                if score >= 0.9:
                    break
            score, audio, heard, speed = best
            ok = score >= 0.9
            if not ok:
                report.append(f"{name} {key}: wanted '{text}', heard '{heard}' ({score:.2f})")
            path = folder / f"{key}.ogg"
            encode(np, audio, path)
            clips[key] = {
                "file": path.name, "sha256": sha256(path), "text": text, "style": style,
                "seconds": round(len(audio) / RATE, 2), "heard": heard, "match": round(score, 3), "ok": ok,
            }
            say(f"  {'ok ' if ok else 'BAD'} {key:<22} {score:.2f}  {heard}")
        if args.only:
            # This run's misses are already in the report; add the lines kept
            # from the last build that missed then and were not made again.
            report += [
                f"{name} {k}: wanted '{c['text']}', heard '{c['heard']}' ({c['match']:.2f})"
                for k, c in clips.items() if not c["ok"] and not k.startswith(args.only)]
        manifest = {
            "voice": name, "gender": voice["gender"], "lang": lang, "speaks": voice["speaks"],
            "blend": voice["blend"], "rate": PACK_RATE, "format": "ogg/vorbis mono",
            "made_from": licences, "clips": clips,
        }
        (folder / "manifest.json").write_text(json.dumps(manifest, indent=2, ensure_ascii=False) + "\n")
        size = sum(p.stat().st_size for p in folder.glob("*.ogg"))
        say(f"  {len(clips)} clips, {size / 1024:.0f} KB, {time.time() - started:.0f} s")
        if args.demo:
            # Each plain line, and three values of each line that has them.
            seen: dict[str, int] = {}
            sentences = []
            for k, c in clips.items():
                family = re.sub(r"\.[0-9]+(\.[0-9])?$", "", k)
                seen[family] = seen.get(family, 0) + 1
                if seen[family] <= 3:
                    sentences.append(folder / c["file"])
            demo(sentences, args.out / f"demo-{name}.mp3")
            say(f"  demo: {args.out / f'demo-{name}.mp3'}")

    say("\nEvery clip matched what it says." if not report else "\nDid not match:\n  " + "\n  ".join(report))
    sys.exit(1 if report else 0)


if __name__ == "__main__":
    main()
