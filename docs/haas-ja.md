# Haas-ja support for Windows

This local fork is based on official Hex `v3.0.0` merged with `v3.1.1` and `v3.3.0`. It adds a Japanese-only `haas_ja_v1` picker choice and is intended to replace ordinary Hex for daily use. Haas builds enable the backend without any switch: the model is available only when `app_paths::is_haas_build()`. The recommended model and normal Hex behavior remain unchanged; only the data/config directory name is `hex-haas` instead of `hex`.

## Runtime

```text
Hex shortcut -> existing capture and 16 kHz conversion
  -> Haas selected -> speech.cpp worker (anonymous stdin/stdout pipes)
     -> bounded, pause-aware full-context pieces -> joined Japanese text
  -> existing text processing, History and paste
```

The worker remains loaded between requests, has bounded JSON messages and response deadlines, and is killed on cancellation, timeout, malformed output or model teardown. A following request restarts a failed worker. No captured audio is written to disk by this adapter. Text logging and retention still follow Hex's ordinary policies. Incomplete model output is rejected rather than silently pasted. Pieces that encode to entirely zero PCM are not sent to the recognizer. When a long clip must be partitioned, pieces the existing optional Silero detector identifies as speechless are also skipped; an unavailable detector never rejects a piece. Whole clips do not get this additional per-piece gate.

Haas is not a transcribe.cpp-compatible Parakeet checkpoint: its prediction network uses Kimi Delta Attention. This fork routes it through speech.cpp instead of renaming another model's weights. The picker uses Hex's existing verified GGUF download path, but never passes Haas to transcribe.cpp. Other models retain the existing engine.

Haas has no live-text streaming or timestamp support in this adapter. Its custom vocabulary is not wired to Hex's recognition hints yet. Dictations of up to 15 seconds are kept whole. Longer input is partitioned only when necessary, preferring the latest natural pause within the bound; planned ranges cover every sample. Hard cuts in continuous speech can affect boundary words. The picker intentionally shows no accuracy percentage: the published Haas metric is CER, not the WER-derived percentage used by Hex's other cards.

## Native punctuation and the adapter correction

Haas **does produce punctuation by default**. The speech.cpp [FastConformer guide](https://github.com/nyosegawa/speech.cpp/blob/main/docs/models/fastconformer.md) describes it as `ja, with punctuation`. Haas support was introduced in speech.cpp v0.9.3; the packaged v0.10.0 executable and DLL already satisfy that requirement. The absence of a punctuation ON/OFF option does not mean the model cannot produce punctuation. No separate punctuation-restoration model is required to enable its native output.

The initial adapter cut even short dictations at pauses, changing their inference context and sometimes decoding noisy tail fragments into extra words. A single synthetic fixture that happened not to receive punctuation was insufficient evidence about the model's general capability.

Three public FLEURS Japanese test clips (IDs 1828, 1834 and 1813, lengths 10.44, 14.22 and 12.84 seconds) confirmed default Japanese commas and full stops in speech.cpp's raw output. The original adapter lost a comma on one clip and appended unrelated tail text on two. After keeping short clips whole, `haas_public_clips_preserve_default_punctuation` verifies exact equality between the raw warm-worker result and the adapter result for all three, including `、` and `。`. PCM16 copies are used on both paths to avoid confounding float-WAV/worker quantization differences. `scripts/check_haas_punctuation.py` records the public source URLs, labels and hashes; it streams only the first three matching audio files, not the full archive.

This is a regression check, not proof that every utterance always receives punctuation or that Japanese recognition is error-free. Whisper's output varied on the same public clips; no Whisper behavior or user setting was changed by this correction.

## Pinned components

- Upstream model: https://huggingface.co/third-intelligence/Haas-ja-v1.0 (Apache-2.0).
- Q8_0 conversion: `sakasegawa/Haas-ja-v1.0-GGUF`, revision `6b965d5007ba780585de51549d57a8c1c0d67c7e`, file `Haas-616M-ja-v1.0-Q8_0.gguf`, 656,381,472 bytes, SHA-256 `cd5f60b2f1c411404b83fca4a65e3ce30a26d9e951b6a346529e65f22758afeb`.
- speech.cpp v0.10.0 Windows x64 Vulkan: https://github.com/nyosegawa/speech.cpp/releases/tag/v0.10.0 . Archive SHA-256 `2f88eb9e29242f1846a80bd2d418553bb0b17637254092b66b73803380a94ab4`.
- transcribe.cpp: the exact Anomaly fork and revision already pinned by Hex's Cargo manifest, built from source with Vulkan.

The stock transcribe.cpp v0.4.1 native package was investigated but rejected: it lacks Hex's `transcribe_parakeet_vocabulary_ext_init` extension. The final build uses the original pinned fork rather than shimming or suppressing the missing function. Portable CMake, Khronos Vulkan headers and Google's shaderc compiler stay under `tools/haas/`; the import library is generated from the PC's installed Vulkan loader. No system SDK is installed and no Rust dependency checkout is modified.

Build tools: CMake 4.4.5 (SHA-256 `326c826486d46a23cf9d5a1c9ee3a567d427bdd02c4a34071937a2a675cff004`), shaderc's 2026-10-28 Windows release build (SHA-256 `2d5358873b2726a15ed9ab6fe45143a76913f4d62a8434a6f6a1a471bdfe1270`, from Google's published build attestation), and Vulkan-Headers commit `6802bb4733b63ed5efd3adb308a6c885ef180ea1` (archive SHA-256 `3760505ffbd3589de77e1b9f5ba11eaac13d3c86869588e19f3df3e0fac7e69c`). The setup script checks all three before use.

SPIRV-Headers commit `496543121ce6419f23d6fa5d7194ba66c36212d2` is also installed into the private build-tools prefix (archive SHA-256 `b33782a6ccd124e6e02634f6382456b0aba2242c6ec5b68d07be20ee77a5b6bc`). Native compilation explicitly uses `/utf-8`; MSVC's default Japanese code page misreads upstream Unicode comments otherwise.

## Build and launch

`scripts/setup-haas-build-tools.ps1` downloads the pinned speech.cpp v0.10.0 Windows x64 Vulkan build beside CMake, shaderc, and the Vulkan/SPIRV headers, verifies every hash before use, and extracts speech.exe and speech.dll under `tools/haas/speech/`. Visual Studio x64 C++ tools, a Windows SDK, Rust, PowerShell 7, and Git (its bash, for the filtered check) are required; inference needs a Vulkan-capable GPU driver (`vulkan-1.dll`).

```powershell
./scripts/setup-haas-build-tools.ps1
./scripts/build-haas.ps1
./scripts/build-haas.ps1 -Check
./scripts/build-haas.ps1 -Lint
./scripts/build-haas.ps1 -TestFilter haas
```

Launch `dist/haas/hex.exe` directly, or use `Start-Hex-Haas.cmd` as a convenience. Both use `%APPDATA%\hex-haas` for settings and history, `%LOCALAPPDATA%\hex-haas` for logs and models (upstream's post-v3.3.0 move of Windows logs and models to Local AppData; the fork's Local folder keeps the fork name so models never mix with the installed Hex's), and `~/.config/hex-haas` for `hex.config.ts`. No special launch environment is required, no settings are automatically copied or forced, and Hex's ordinary single-instance handling applies. Launch at login uses the unmodified Windows registration behavior. As with any official local build, automatic updates are off; rebuild to update the fork.

The former checkout-local `haas-data/` and `haas-config/` directories are no longer used. To keep that profile, quit Hex and move them to the corresponding `hex-haas` directories, only when the destinations do not already contain data. The ordinary `hex` directories remain untouched. There is no parallel-install or launcher-switching mechanism; uninstall the official application if replacing it, keeping its old data if desired.

## Verification

Available tests cover protocol version validation, bounded parsing, PCM conversion, incomplete/mismatched responses, and lossless/bounded audio partitioning. The opt-in `haas_worker_transcribes_a_fixture_twice` test requires `HEX_HAAS_TEST_MODEL`, `HEX_HAAS_TEST_WAV`, and `HEX_SPEECH_CPP_EXE`.

Observed tests: the Hex binary's 578 default unit tests passed (13 opt-in/platform tests skipped). The explicit Haas fixture test also passed: two warm requests, cancellation with worker teardown, automatic recovery, and a roughly 46-second repeated fixture retaining all six occurrences of the reference phrase. Warm worker calls took 92 ms and 61 ms on this synthetic 7.6-second clip; this excludes microphone, model startup, text processing and paste.

`cargo clippy --locked --all-targets -- -D warnings` passed through `scripts/build-haas.ps1 -Lint`. The checks use the exact locally built native library prefix; no substitute native DLL or compatibility shim is used.

An existing settings test assumed an English host and failed on Japanese Windows before any Haas behavior ran. Its fixture now explicitly sets the English current selection it tests; production defaults are unchanged. The wider workspace has a separate existing failure in `scrawl-router::opencode::session_lookup::opencode2_output_is_bounded`: its test mock starts `/bin/sh`, which does not exist at that path on Windows. That unrelated router source was not changed and the full workspace is not claimed to pass.

`scripts/test-haas-native.ps1` synthesizes a Japanese WAV using a locally installed Windows SAPI voice, then transcribes it. It never opens the microphone or plays audio. A successful transcription is runtime proof, not a Japanese accuracy benchmark.

Observed on this PC:

- The release build succeeds with the exact original transcribe.cpp fork and Vulkan enabled. The packaged Hex CLI loads Haas through speech.cpp and transcribes the synthetic Japanese WAV; the same package also loads and transcribes with Whisper on the RTX 3060 Ti Vulkan backend.
- The first cold speech.cpp run took 18.1 s for 7.6 s audio. A subsequent standalone run with one CPU decoder thread took 0.192 s; this is not an end-to-end dictation latency measurement.
- The light and dark Japanese picker fixture renders were generated and visually inspected; Haas appears as a selectable model. Linux golden comparisons were not run on this Windows host.
- Earlier checks used a launcher that refused to start while the installed Hex was running. That fork-specific restriction has since been removed; repeated launches now use ordinary Hex single-instance handling. The earlier check is not proof of the new launch behavior.
- This small synthetic fixture is not an accuracy benchmark. Haas misrecognized the initial `今日は` as `共和`, while Whisper transcribed it correctly. Better accuracy than Whisper has not been established.

A physical shortcut/microphone/paste test has not been run. The packaged CLI and fixture renders do not prove that the current foreground application consumes a pasted transcript.

## CPU variant (Windows x64, no Vulkan loader)

`Hex-Haas-<version>-windows-x64-cpu.zip` is separate from the existing GPU
package. Get it from the manually dispatched **Haas CPU build** workflow's
artifact, and `haas-v*` tag Releases include it next to the GPU zip with a
combined SHA256SUMS. Extract all
three binaries together and run `hex.exe`. Do not mix CPU and GPU files.
Requirements: Windows 10/11 x64, a CPU from 2013 onward (Haswell / Excavator
or newer, AVX2); upstream 3.3.0 actually tests AVX2, BMI2, FMA and F16C
(x86-64-v3), not just AVX2 as the original CPU plan assumed. Inference needs no GPU,
Vulkan SDK, or `vulkan-1.dll`, but is slower on the CPU. Rune/wgpu display
support (including DX12 WARP on AVD) is separate and needs a real-PC check.

```powershell
./scripts/setup-haas-cpu-build-tools.ps1
./scripts/build-haas-cpu.ps1
./scripts/build-haas-cpu.ps1 -Check
./scripts/build-haas-cpu.ps1 -Lint
./scripts/build-haas-cpu.ps1 -TestFilter haas
./scripts/package-haas-cpu.ps1
./dist/haas-cpu/hex.exe --version
./dist/haas-cpu/hex.exe model devices
./dist/haas-cpu/hex.exe model install haas_ja_v1 --language ja
./dist/haas-cpu/hex.exe model transcribe path/to/fixture.wav --model haas_ja_v1 --language ja
```

The CPU setup hash-verifies CMake 4.4.5 (the same pin as the GPU tools),
speech.cpp v0.10.0 source at `06590811a90041f935a470ba0e2b85346d2fc041`
(zip SHA-256 `111cc186abb0e20fbdfa222c0f9f5e52976da1fc51680a0b834a43606a02667e`),
and nyosegawa/ggml at `700c9d743ed7cc583c0f1b6507afebb6c0e88bc0`
(zip SHA-256 `3b2722df7f79f02b3f9e5c0acccce2bfa8ff0c687abbd3090ac206bc60fed9b9`).
It builds/installs static ggml without Vulkan, native-host tuning, OpenMP,
tests or examples, then builds speech with `SPEECH_INSTALLED_GGML=ON`.
Both stages use x64 and the static MSVC CRT. Tools stay in `tools/haas-cpu/`.

The CPU Hex build clears `VULKAN_SDK` and `TRANSCRIBE_DIR`, explicitly sets
`TRANSCRIBE_CMAKE_ARGS=-DGGML_NATIVE=OFF -DTRANSCRIBE_VULKAN=OFF`, and
overrides Cargo's default `CMAKE_ARGS` with only the static CRT option.
`CARGO_TARGET_DIR=target-cpu` isolates every build/check/lint/test path;
none reuses a GPU native prefix. Outputs are `dist/haas-cpu/` and
`dist/package-cpu/`. Use a fresh shell when switching back to GPU builds.
The Windows x64 Cargo default is now CPU, including unused upstream generic
Windows build workflows; only `build-haas.ps1` enables Haas Vulkan builds.
The Linux Cargo Vulkan feature and the transcribe.cpp pin remain unchanged.

Both packages share `%APPDATA%\hex-haas` (settings, history),
`%LOCALAPPDATA%\hex-haas` (models, logs; the fork's own Local folder, named
like the Roaming one since upstream #1432 moved Windows models there), and
`~/.config/hex-haas`, and both
disable automatic updates. Quit Hex before switching variants. Upstream
3.3.0's GGUF Auto/GPU/CPU health/fallback policy is retained; Haas still
uses its separate speech.cpp worker before transcribe.cpp initialization.
That GGUF device setting does not control the Haas worker; the packaged
speech.cpp build determines whether Haas has a GPU backend.

Verification definition: setup checks speech.exe/speech.dll imports, and
the CPU build checks all three packaged binaries with dumpbin. The new CI
workflow additionally runs the CLI version
and device commands, a real Haas model download, and inference on a 3-second
non-silent synthetic PCM WAV. Japanese SAPI is not assumed; the waveform is
generated without SAPI, recording or playback. Empty recognition text is
allowed, but download/load/inference failure blocks the artifact. This
proves neither Japanese accuracy nor GUI/paste behavior. These Windows
checks passed in CI on 2026-11-04 (both the CPU workflow and the GPU
release workflow): Windows compilation, real import scans, CLI startup and
synthetic inference, and the GPU-link regression check. What remains
unverified is the AVD side: physical dictation/paste and GUI display.

## ARM64 CPU variant (Windows ARM64, no Vulkan loader)

`Hex-Haas-<version>-windows-arm64-cpu.zip` is a native ARM64 (aarch64)
build for Windows 11 ARM64 PCs (Snapdragon X / Surface and similar). It
mirrors upstream's Windows ARM64 configuration: ggml's ARM CPU backend
refuses MSVC, so the native core builds with clang-cl through Ninja with
`GGML_NATIVE=OFF` and ggml's own threadpool (no OpenMP). Inference is CPU
only: no Vulkan driver, SDK, or `vulkan-1.dll`, and no x86 emulation or
AVX2 requirement (the x86-64-v3 gate in `transcription/cpu_floor` is
`#[cfg(target_arch = "x86_64")]` and does not run on ARM64). CPU inference
is slower than the x64 GPU build. GPU/Vulkan packages for ARM64 do not
exist: speech.cpp Vulkan would need shaderc, which has no ARM64 Windows
release. The build must run on an ARM64 Windows host (the pinned LLVM is
an ARM64 binary); CI uses the `windows-11-arm` runner.

```powershell
./scripts/setup-haas-cpu-build-tools.ps1 -Arch arm64
./scripts/build-haas-cpu.ps1 -Arch arm64
./scripts/build-haas-cpu.ps1 -Arch arm64 -Check
./scripts/build-haas-cpu.ps1 -Arch arm64 -Lint
./scripts/build-haas-cpu.ps1 -Arch arm64 -TestFilter haas
./scripts/package-haas-cpu.ps1 -Arch arm64
./dist/haas-cpu-arm64/hex.exe --version
./dist/haas-cpu-arm64/hex.exe model devices
./dist/haas-cpu-arm64/hex.exe model install haas_ja_v1 --language ja
./dist/haas-cpu-arm64/hex.exe model transcribe path/to/fixture.wav --model haas_ja_v1 --language ja
```

The ARM64 setup additionally pins CMake 4.4.5 (`cmake-4.4.5-windows-arm64.zip`),
Ninja 1.12.1 (`ninja-winarm64.zip`), and LLVM 20.1.8
(`clang+llvm-20.1.8-aarch64-pc-windows-msvc.tar.xz`), all hash-verified,
beside the same speech.cpp and ggml pins as x64. MSVC ARM64 build tools
(link.exe, import libraries, Windows SDK) come from Visual Studio; the
scripts accept the ARM64 component or any install that actually ships
`arm64` compiler binaries. Tools stay in `tools/haas-cpu-arm64/`, Cargo
output in `target-cpu-arm64/` (`CARGO_BUILD_TARGET=aarch64-pc-windows-msvc`
keeps every cargo entry point there), and binaries in `dist/haas-cpu-arm64/`.
The build script refuses to run on a non-ARM64 host because the pinned
clang-cl is an ARM64 binary. Both dumpbin import scans and an ARM64
machine-type header check gate the build; the CI ARM64 job adds the same
CLI startup, model download, and synthetic-WAV inference gates as x64.

Observed on 2026-11-08, on the Snapdragon X 10-core (X1P64100 @ 3.40 GHz,
16 GB) ARM64 Windows 11 PC this fork is built on: the setup script produced
speech.exe/speech.dll whose only imports are KERNEL32 and ADVAPI32, all
three binaries pass the ARM64 machine-type header check, `cargo check`,
`cargo clippy -D warnings`, and every haas-filtered test pass, and the
packaged hex.exe (3.3.1) reports its version, enumerates the CPU device,
downloads the 656 MB Haas model from Hugging Face, and transcribes the
3-second synthetic WAV on the CPU in about 2 seconds including the model
load. This proves neither Japanese accuracy nor GUI/paste behavior; those
still need the real microphone, shortcut, and paste checks on this PC.
One fork adaptation was required by this verification: upstream's move of
Windows logs and models to Local AppData (#1432) added a Local folder that
the fork's `hex-haas` rename had not covered, so a Haas build would have
mixed its models into the installed Hex's `%LOCALAPPDATA%\hex`; the rename
now covers the Local folder (`%LOCALAPPDATA%\hex-haas`) with a test.
