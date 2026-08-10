# /// script
# requires-python = ">=3.11"
# dependencies = ["mlx-whisper", "mlx-lm"]
# ///
"""MLX side of the Step 0 A/B comparison (see plan.md).

Mirrors crates/spike: same audio, same sentences, same metrics.
Usage: uv run scripts/mlx_bench.py [asr|translate|all]
"""

import resource
import sys
import time

WHISPER_REPO = "mlx-community/whisper-large-v3-turbo"
LLM_REPO = "mlx-community/Qwen3-4B-Instruct-2507-4bit"
AUDIO = [("assets/test-ja.wav", "ja"), ("assets/test-en.wav", "en")]
WINDOW_SECS = 15
STEP_SECS = 1
SR = 16_000

EN_SENTENCES = [
    "Thank you all for joining today.",
    "The implementation of the new search feature was completed on schedule.",
    "Next week we plan to start working on performance improvements, but we may "
    "need to revisit the priorities depending on the customer feedback we receive.",
]
JA_SENTENCES = [
    "先週の進捗を共有します。",
    "新しい検索機能の実装は予定どおり完了しましたが、パフォーマンスにはまだ改善の余地があります。",
]


def peak_rss_mb() -> int:
    return resource.getrusage(resource.RUSAGE_SELF).ru_maxrss // (1024 * 1024)


def load_audio(path: str):
    """Read a 16 kHz mono 16-bit wav without shelling out to ffmpeg."""
    import wave

    import numpy as np

    with wave.open(path, "rb") as w:
        assert w.getframerate() == SR and w.getnchannels() == 1
        data = w.readframes(w.getnframes())
    return np.frombuffer(data, dtype=np.int16).astype(np.float32) / 32768.0


def run_asr() -> None:
    import mlx_whisper

    print(f"=== ASR ({WHISPER_REPO}) ===")

    def decode(audio, lang):
        t = time.monotonic()
        result = mlx_whisper.transcribe(
            audio, path_or_hf_repo=WHISPER_REPO, language=lang, verbose=None
        )
        return result["text"].strip(), int((time.monotonic() - t) * 1000)

    for path, lang in AUDIO:
        audio = load_audio(path)
        dur_s = len(audio) / SR
        print(f"\n--- {path} lang={lang} ({dur_s:.1f}s) ---")

        # First call includes model load + compile; time it separately.
        t = time.monotonic()
        text, wall = decode(audio, lang)
        print(f"cold full decode: {wall} ms")
        text, wall = decode(audio, lang)
        print(f"warm full decode: {wall} ms (RTF {wall / 1000 / dur_s:.2f})")
        print(f"text: {text}")

        print(f"windowed re-decode (window <={WINDOW_SECS}s, step {STEP_SECS}s):")
        worst = 0
        first = None
        for end_s in range(STEP_SECS, int(dur_s) + 1, STEP_SECS):
            end = min(end_s * SR, len(audio))
            start = max(0, end - WINDOW_SECS * SR)
            text, wall = decode(audio[start:end], lang)
            worst = max(worst, wall)
            if first is None and text:
                first = (end_s, wall)
            print(f"  t={end_s:>2}s window={(end - start) / SR:>5.1f}s decode={wall:>5} ms")
        if first:
            print(f"first non-empty partial: audio_t={first[0]}s + decode {first[1]} ms")
        print(f"worst window decode: {worst} ms (budget: first partial <=2000 ms)")


def run_translate() -> None:
    from mlx_lm import load, stream_generate

    print(f"\n=== Translation ({LLM_REPO}) ===")
    t = time.monotonic()
    model, tokenizer = load(LLM_REPO)
    print(f"llm model loaded in {int((time.monotonic() - t) * 1000)} ms")

    def translate(sentence, source, target):
        messages = [
            {
                "role": "system",
                "content": (
                    "You are a professional simultaneous interpreter. "
                    f"Translate the user's sentence from {source} to {target}. "
                    "Output only the translation, nothing else."
                ),
            },
            {"role": "user", "content": sentence},
        ]
        prompt = tokenizer.apply_chat_template(messages, add_generation_prompt=True)
        t = time.monotonic()
        out, ntok = "", 0
        for resp in stream_generate(model, tokenizer, prompt, max_tokens=256):
            out += resp.text
            ntok += 1
        return out.strip(), int((time.monotonic() - t) * 1000), ntok

    # Warm-up (first call pays compile cost).
    translate("Hello.", "English", "Japanese")
    for s in EN_SENTENCES:
        out, wall, ntok = translate(s, "English", "Japanese")
        print(f"en->ja {wall:>5} ms {ntok:>3} tok ({ntok / (wall / 1000):.1f} tok/s): {out}")
    for s in JA_SENTENCES:
        out, wall, ntok = translate(s, "Japanese", "English")
        print(f"ja->en {wall:>5} ms {ntok:>3} tok ({ntok / (wall / 1000):.1f} tok/s): {out}")


if __name__ == "__main__":
    mode = sys.argv[1] if len(sys.argv) > 1 else "all"
    if mode in ("asr", "all"):
        run_asr()
    if mode in ("translate", "all"):
        run_translate()
    print(f"\npeak RSS: {peak_rss_mb()} MB")
