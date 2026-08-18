#!/usr/bin/env python3
"""Check a recording session: are the three lanes on one timeline, and does
the transcript point at the audio it claims?

    scripts/check-recording.py [session-dir] [--sync]

With no argument it takes the newest session under ~/Music/live-poly-trans.

The alignment test is deliberately blunt: an utterance's span should be
louder than the lane's quiet background. A transcript stamped against the
wrong clock lands on silence, and that shows up here as a level at or below
the lane's noise floor.

`--sync` measures the lanes against each other, which only works for a
recording made with the audio coming out of real speakers and into the mic:
cross-correlating the two envelopes then shows how far apart the same sound
landed in the two files. Expect a few ms to ~30ms (air plus input latency).
A recording made on headphones has nothing in common between the lanes and
reports a meaningless peak — the correlation value says which case it is.
"""

import json
import math
import struct
import sys
import wave
from pathlib import Path

LANES = ("mic", "speaker", "mix")


def read_wav(path):
    with wave.open(str(path)) as w:
        if w.getsampwidth() != 2 or w.getnchannels() != 1:
            sys.exit(f"{path.name}: expected 16-bit mono, got "
                     f"{w.getsampwidth()*8}-bit {w.getnchannels()}ch")
        frames = w.getnframes()
        data = struct.unpack(f"<{frames}h", w.readframes(frames))
        return w.getframerate(), data


def rms(samples):
    if not samples:
        return 0.0
    return math.sqrt(sum(s * s for s in samples) / len(samples)) / 32767


def percentile(values, fraction):
    ordered = sorted(values)
    return ordered[min(len(ordered) - 1, int(len(ordered) * fraction))]


def noise_floor(samples, rate):
    """Level of the quietest tenth of the recording, in 100ms blocks."""
    block = rate // 10
    blocks = [rms(samples[i:i + block]) for i in range(0, len(samples), block)]
    return percentile(blocks, 0.1)


def onsets(samples, rate, hop_ms=5):
    """Where sound *starts*, per block: the rising part of the loudness curve.

    Steady loudness cancels out, so this keys on attacks — a click played
    through the speakers is a spike in both lanes, while the two lanes' own
    unrelated content (your voice here, a video there) is not.
    """
    hop = rate * hop_ms // 1000
    env = [rms(samples[i:i + hop]) for i in range(0, len(samples) - hop, hop)]
    rising = [max(0.0, env[i] - env[i - 1]) for i in range(1, len(env))]
    mean = sum(rising) / len(rising)
    centred = [r - mean for r in rising]
    norm = math.sqrt(sum(r * r for r in centred)) or 1.0
    return [r / norm for r in centred]


def report_sync(audio, hop_ms=5, span_ms=1000):
    """Where the same attack sits in the two lanes, by cross-correlation."""
    rate, mic = audio["mic"]
    _, speaker = audio["speaker"]
    mic, speaker = onsets(mic, rate, hop_ms), onsets(speaker, rate, hop_ms)
    scores = []
    for lag in range(-span_ms // hop_ms, span_ms // hop_ms + 1):
        a, b = (mic[lag:], speaker) if lag >= 0 else (mic, speaker[-lag:])
        n = min(len(a), len(b))
        scores.append((sum(a[i] * b[i] for i in range(n)), lag))
    peak, lag = max(scores)
    # How much the winner stands out from lags it could not be confused with.
    far = [s for s, l in scores if abs(l - lag) * hop_ms > 100]
    background = max(far) if far else 0.0
    print(f"\nlane sync: peak at {lag * hop_ms:+d} ms (mic behind speaker), "
          f"score {peak:.3f} against {background:.3f} elsewhere")
    if peak < 2 * background or peak < 0.05:
        print("  ! no shared attack to measure — the lanes need one sound "
              "that reaches both: play a click through the speakers (not "
              "headphones) with the room otherwise quiet")
    elif -50 <= lag * hop_ms <= 100:
        print("  ✓ the same sound lands in both lanes within acoustic delay")
    else:
        print(f"  ✗ {lag * hop_ms} ms apart — more than air and input latency "
              f"explain")


def main():
    base = Path.home() / "Music" / "live-poly-trans"
    argv = [a for a in sys.argv[1:] if not a.startswith("--")]
    want_sync = "--sync" in sys.argv
    if argv:
        session = Path(argv[0])
    else:
        sessions = sorted(d for d in base.glob("*") if d.is_dir())
        if not sessions:
            sys.exit(f"no sessions under {base}")
        session = sessions[-1]
    print(f"session: {session}\n")

    audio = {}
    for lane in LANES:
        path = session / f"{lane}.wav"
        if not path.exists():
            sys.exit(f"missing {path}")
        rate, samples = read_wav(path)
        audio[lane] = (rate, samples)
        print(f"{lane + '.wav':>12}  {len(samples)/rate:7.2f}s  "
              f"peak {max(abs(s) for s in samples)/32767:.3f}  "
              f"rms {rms(samples):.4f}  floor {noise_floor(samples, rate):.4f}")

    lengths = {lane: len(audio[lane][1]) for lane in LANES}
    if len(set(lengths.values())) == 1:
        print("\n  ✓ all three lanes are the same length")
    else:
        print(f"\n  ✗ lanes disagree on length: {lengths}")

    if want_sync:
        report_sync(audio)

    log = session / "transcript.jsonl"
    if not log.exists():
        print(f"\nno transcript.jsonl (older build?)")
        return
    utterances = [json.loads(line) for line in log.read_text().splitlines() if line.strip()]
    if not utterances:
        print("\ntranscript.jsonl is empty — nothing was said, or nothing was heard")
        return

    print(f"\n{len(utterances)} utterances\n")
    print(f"{'lane':<8}{'start':>9}{'end':>9}{'level':>9}{'floor×':>8}  text")
    misaligned = 0
    for u in utterances:
        rate, samples = audio[u["lane"]]
        floor = noise_floor(samples, rate)
        span = samples[int(u["startMs"] * rate / 1000):int(u["endMs"] * rate / 1000)]
        level = rms(span)
        ratio = level / floor if floor else float("inf")
        flag = " " if ratio >= 2 else "✗"
        if flag == "✗":
            misaligned += 1
        print(f"{u['lane']:<8}{u['startMs']/1000:8.2f}s{u['endMs']/1000:8.2f}s"
              f"{level:9.4f}{ratio:7.1f}x {flag} {u['text'][:40]}")

    print()
    if misaligned:
        print(f"  ✗ {misaligned} utterance(s) point at audio no louder than the "
              f"lane's noise floor — check the timestamps")
    else:
        print("  ✓ every utterance lands on audible audio")


if __name__ == "__main__":
    main()
