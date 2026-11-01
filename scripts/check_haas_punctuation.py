"""Read three public FLEURS clips from the start of its archive, then compare
speech.cpp's raw Haas output with Hex's Haas and Whisper adapters. No microphone,
paste, user transcript, or model-backed post-processing is used.
"""
import csv
import hashlib
import io
import json
import math
import os
from pathlib import Path
import subprocess
import struct
import sys
import tarfile
import urllib.request
import wave

sys.stdout.reconfigure(encoding="utf-8")
ROOT = Path(__file__).resolve().parents[1]
OUT = ROOT / "tools" / "haas" / "fixtures" / "fleurs-punctuation"
OUT.mkdir(parents=True, exist_ok=True)
TSV = "https://huggingface.co/datasets/google/fleurs/raw/main/data/ja_jp/test.tsv"
ARCHIVE = "https://huggingface.co/datasets/google/fleurs/resolve/main/data/ja_jp/audio/test.tar.gz"
with urllib.request.urlopen(TSV, timeout=60) as response:
    labels = {row[1]: {"id": row[0], "reference": row[2]} for row in csv.reader(io.StringIO(response.read().decode("utf-8")), delimiter="\t") if len(row) >= 3}

manifest = OUT / "manifest.json"
if manifest.exists():
    clips = json.loads(manifest.read_text(encoding="utf-8"))
else:
    class LimitedReader:
        def __init__(self, source):
            self.source, self.count = source, 0
        def read(self, size=-1):
            if size < 0:
                raise ValueError("unbounded archive read")
            self.count += size
            if self.count > 25 * 1024 * 1024:
                raise ValueError("archive streaming limit exceeded")
            return self.source.read(size)
    clips = []
    with urllib.request.urlopen(ARCHIVE, timeout=60) as response:
        with tarfile.open(fileobj=LimitedReader(response), mode="r|gz") as archive:
            for member in archive:
                name = Path(member.name).name
                if not member.isfile() or name not in labels:
                    continue
                if member.size > 8 * 1024 * 1024:
                    raise ValueError("unexpectedly large public clip")
                data = archive.extractfile(member).read()
                (OUT / name).write_bytes(data)
                clips.append({"file": name, **labels[name], "sha256": hashlib.sha256(data).hexdigest()})
                if len(clips) == 3:
                    break
    if len(clips) != 3:
        raise RuntimeError("did not retrieve three public clips")
    manifest.write_text(json.dumps(clips, ensure_ascii=False, indent=2), encoding="utf-8")

env = os.environ.copy()
for name in ("HEX_APPLICATION_SUPPORT_DIR", "HEX_USER_CONFIG_DIR", "HEX_SPEECH_CPP_EXE"):
    if env.get(name):
        raise RuntimeError(f"Unset {name} before testing the ordinary Haas package.")
speech = ROOT / "dist" / "haas" / "speech.exe"
hex_exe = Path(os.environ.get("HEX_PUNCTUATION_TEST_EXE", ROOT / "dist" / "haas" / "hex.exe"))
model = Path(os.environ["APPDATA"]) / "hex-haas" / "models" / "Haas-616M-ja-v1.0-Q8_0.gguf"

def canonical_pcm16(source):
    """Use exactly the same PCM16 file for every engine, avoiding a float-WAV
    versus worker-PCM16 comparison as a confounding input difference."""
    data = source.read_bytes()
    if data[:4] != b"RIFF" or data[8:12] != b"WAVE":
        raise ValueError("not a WAV")
    fmt = payload = None
    at = 12
    while at + 8 <= len(data):
        name, size = data[at:at+4], struct.unpack_from("<I", data, at+4)[0]
        if name == b"fmt ":
            fmt = struct.unpack_from("<HHIIHH", data, at+8)
        if name == b"data":
            payload = data[at+8:at+8+size]
        at += 8 + size + (size & 1)
    encoding, channels, rate, _, _, bits = fmt
    if channels != 1 or rate != 16000 or payload is None:
        raise ValueError("expected 16 kHz mono WAV")
    if encoding == 3 and bits == 32:
        def quantize(value):
            value = max(-1.0, min(1.0, value)) * 32767.0 if math.isfinite(value) else 0.0
            return math.floor(value + 0.5) if value >= 0 else math.ceil(value - 0.5)
        payload = b"".join(struct.pack("<h", quantize(value)) for value, in struct.iter_unpack("<f", payload))
    elif encoding != 1 or bits != 16:
        raise ValueError("unsupported fixture encoding")
    output = source.with_name(source.stem + "-pcm16.wav")
    with wave.open(str(output), "wb") as stream:
        stream.setparams((1, 2, 16000, 0, "NONE", "not compressed"))
        stream.writeframes(payload)
    return output

results = []
for clip in clips:
    wav = canonical_pcm16(OUT / clip["file"])
    row = dict(clip)
    row["canonical_pcm16"] = wav.name
    commands = {
        "speech_cpp_haas": [speech, "asr", model, "--threads", "1", wav],
        "hex_haas": [hex_exe, "model", "transcribe", wav, "--model", "haas_ja_v1", "--language", "ja"],
        "hex_whisper": [hex_exe, "model", "transcribe", wav, "--model", "whisper_large_v3_turbo", "--language", "ja"],
    }
    for key, command in commands.items():
        run = subprocess.run([str(part) for part in command], env=env, capture_output=True, encoding="utf-8", errors="strict", timeout=90)
        if run.returncode:
            raise RuntimeError(f"{key}: {run.stderr}")
        row[key] = run.stdout.strip()
    results.append(row)
    print(json.dumps(row, ensure_ascii=False), flush=True)
(OUT / "results.json").write_text(json.dumps(results, ensure_ascii=False, indent=2), encoding="utf-8")
