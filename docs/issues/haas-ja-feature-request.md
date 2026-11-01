### What are you trying to do?

Use [Third Intelligence's Haas-ja-v1.0](https://huggingface.co/third-intelligence/Haas-ja-v1.0) for local Japanese dictation in Hex, while keeping Hex's existing shortcut, recording, History, and paste workflow. The immediate use case is Windows 11 x64, but availability on other supported platforms would also be welcome where the inference backend can support it.

Haas is a roughly 616M-parameter Japanese ASR model released under Apache-2.0. It uses FastConformer/TDT, but replaces the ordinary Parakeet prediction network's LSTM with Kimi Delta Attention (KDA), so being Parakeet-derived does not make it a drop-in replacement for a supported Parakeet artifact.

The inspected baseline is Hex v3.0.0, commit `7b05b5fd1fa0e3b9c333209cd1644d9603693d5c`. Its compiled model catalog and bundled runtime do not offer Haas for normal dictation. This request is about supporting that specific model, not assuming arbitrary Hugging Face ASR repositories or GGUF files are compatible.

### What would help?

Offer Haas-ja-v1.0 as an optional Japanese transcription model once a supported backend can prepare and run it safely. Existing model choices and the default recommendation should remain unchanged. Unknown accuracy/speed figures should not inherit another model's benchmark scores.

The requested outcome is engine-agnostic: native support in the existing backend would be fine. `speech.cpp` was used for a local feasibility prototype, not proposed as a mandatory dependency or as a request to introduce a general plugin framework.

#### Local feasibility findings

A local experimental integration based on v3.0.0 successfully ran Haas via [speech.cpp v0.10.0](https://github.com/nyosegawa/speech.cpp/releases/tag/v0.10.0), using the [sakasegawa Q8_0 conversion](https://huggingface.co/sakasegawa/Haas-ja-v1.0-GGUF) at revision `6b965d5007ba780585de51549d57a8c1c0d67c7e` (`Haas-616M-ja-v1.0-Q8_0.gguf`, 656,381,472 bytes).

- The recognizer runs in a warm local `speech worker` process, receiving 16 kHz mono PCM through anonymous pipes; captured audio is not saved to a temporary file.
- The adapter reuses Hex's existing transcription job, processing, and output paths, with response deadlines and worker teardown on cancellation/error.
- A synthetic Japanese WAV transcribed successfully on an RTX 3060 Ti through Vulkan. Two warm worker requests took 92 ms and 61 ms for a 7.6-second clip. These are small smoke-test observations, **not** a comparative accuracy benchmark or end-to-end dictation latency measurement; cold initialization is excluded.
- Explicit tests exercised repeated requests, cancellation followed by worker recovery, and a roughly 46-second repeated fixture retaining all six occurrences of a reference phrase.
- Native Japanese commas and full stops were confirmed on three public FLEURS clips. Keeping ordinary short dictations whole preserved the native output exactly; no separate punctuation-restoration model was used.
- The packaged Hex CLI continued to transcribe with Whisper through the original pinned transcribe.cpp backend. Physical shortcut/microphone/paste validation and other operating systems were not covered by those automated checks.

Haas is an offline/full-context model. The prototype bounds inference pieces at natural pauses, with a size cap, rather than claiming native streaming support. Correct handling of long recordings, punctuation, and custom vocabulary would need their own evaluation before shipping.

These findings suggest a viable local implementation, but do not establish that this worker-based approach is the best upstream design. Is supporting this Japanese model of interest, and should the runtime work preferably happen in Hex's existing backend?

Related: #648 requests import of **already compatible** non-catalog models and explicitly excludes new inference backends. That import workflow alone would not supply Haas's KDA implementation, so this is a separate model/runtime support request.
