//! Isolated speech.cpp worker for the experimental Windows Haas model.
//! Audio goes through anonymous pipes only; no recording is written to disk.

use std::io::{BufRead, BufReader, Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, Stdio};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, SyncSender};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use base64::{Engine as _, engine::general_purpose::STANDARD};
use color_eyre::eyre::{Result, WrapErr, bail, eyre};
use serde_json::{Value, json};

const MAX_LINE_BYTES: usize = 1024 * 1024;
const SAMPLE_RATE: usize = 16_000;
const MAX_PIECE_SAMPLES: usize = 15 * SAMPLE_RATE;
const POLL: Duration = Duration::from_millis(50);

struct Request {
    id: String,
    samples: Vec<f32>,
}

pub(super) struct SpeechCpp {
    model_path: PathBuf,
    child: Option<Child>,
    requests: Option<SyncSender<Request>>,
    events: Receiver<Result<Value, String>>,
    threads: Vec<JoinHandle<()>>,
    next_id: u64,
}

impl SpeechCpp {
    pub fn load(model_path: &Path) -> Result<Self> {
        let executable = executable()?;
        let mut child = crate::background_process::command(&executable)
            .arg("worker")
            .arg(model_path)
            .args(["--threads", "1"])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .wrap_err_with(|| format!("could not start {}", executable.display()))?;
        let stdin = child.stdin.take().expect("piped worker stdin");
        let stdout = child.stdout.take().expect("piped worker stdout");
        let (messages, events) = mpsc::sync_channel(16);
        let (requests, incoming) = mpsc::sync_channel::<Request>(1);
        let mut worker = Self {
            model_path: model_path.to_owned(),
            child: Some(child),
            requests: Some(requests),
            events,
            threads: Vec::new(),
            next_id: 0,
        };
        let reader_messages = messages.clone();
        worker.threads.push(
            thread::Builder::new()
                .name("haas-worker-output".into())
                .spawn(move || {
                    let mut reader = BufReader::new(stdout);
                    loop {
                        let message = match read_message(&mut reader) {
                            Ok(Some(value)) => Ok(value),
                            Ok(None) => Err("speech.cpp worker closed its output".into()),
                            Err(error) => Err(format!("speech.cpp protocol error: {error:#}")),
                        };
                        let failed = message.is_err();
                        // Never leave this thread blocked on a full channel at shutdown.
                        if reader_messages.try_send(message).is_err() || failed {
                            break;
                        }
                    }
                })?,
        );
        worker.threads.push(
            thread::Builder::new()
                .name("haas-worker-input".into())
                .spawn(move || {
                    let mut stdin = stdin;
                    while let Ok(request) = incoming.recv() {
                        if let Err(error) = write_request(&mut stdin, &request) {
                            let _ = messages.try_send(Err(format!(
                                "could not send audio to speech.cpp: {error:#}"
                            )));
                            break;
                        }
                    }
                })?,
        );
        let ready = worker.receive(Instant::now() + Duration::from_secs(120))?;
        validate_ready(&ready)?;
        tracing::info!(engine = "speech.cpp", version = ?ready.get("version"), "loaded Haas-ja worker");
        Ok(worker)
    }

    pub fn transcribe(&mut self, samples: &[f32]) -> Result<String> {
        if !has_pcm_audio(samples) {
            return Ok(String::new());
        }
        if self.child.is_none() {
            *self = Self::load(&self.model_path.clone())?;
        }
        let mut result = String::new();
        let ranges = piece_ranges(samples);
        let split = ranges.len() > 1;
        for range in ranges {
            let piece = &samples[range];
            if !should_transcribe_piece(piece, split) {
                continue;
            }
            let text = self.transcribe_piece(piece)?;
            // Japanese needs no artificial space between adjacent utterances.
            result.push_str(text.trim());
        }
        Ok(result)
    }

    fn transcribe_piece(&mut self, samples: &[f32]) -> Result<String> {
        self.next_id += 1;
        let id = self.next_id.to_string();
        let request = Request {
            id: id.clone(),
            samples: samples.to_vec(),
        };
        let sent = self
            .requests
            .as_ref()
            .is_some_and(|requests| requests.try_send(request).is_ok());
        if !sent {
            self.stop();
            bail!("speech.cpp worker is unavailable");
        }
        let deadline = Instant::now()
            + Duration::from_secs(120)
            + Duration::from_secs_f64(samples.len() as f64 / SAMPLE_RATE as f64 * 4.0);
        loop {
            let message = match self.receive(deadline) {
                Ok(message) => message,
                Err(error) => {
                    self.stop();
                    return Err(error);
                }
            };
            match response(&message, &id) {
                Ok(Some(text)) => return Ok(text),
                Ok(None) => {}
                Err(error) => {
                    self.stop();
                    return Err(error);
                }
            }
        }
    }

    fn receive(&self, deadline: Instant) -> Result<Value> {
        loop {
            if crate::gguf_session::job_cancel().is_some_and(|cancel| cancel.is_cancelled()) {
                bail!("Haas transcription cancelled");
            }
            if Instant::now() >= deadline {
                bail!("speech.cpp worker timed out");
            }
            match self
                .events
                .recv_timeout(POLL.min(deadline.saturating_duration_since(Instant::now())))
            {
                Ok(Ok(message)) => return Ok(message),
                Ok(Err(error)) => bail!("{error}"),
                Err(RecvTimeoutError::Timeout) => {}
                Err(RecvTimeoutError::Disconnected) => bail!("speech.cpp worker disconnected"),
            }
        }
    }

    fn stop(&mut self) {
        self.requests.take();
        if let Some(mut child) = self.child.take() {
            let _ = child.kill();
            let _ = child.wait();
        }
        for thread in self.threads.drain(..) {
            let _ = thread.join();
        }
    }
}

impl Drop for SpeechCpp {
    fn drop(&mut self) {
        self.stop();
    }
}

fn executable() -> Result<PathBuf> {
    let executable = if let Some(path) = std::env::var_os("HEX_SPEECH_CPP_EXE") {
        PathBuf::from(path)
    } else {
        std::env::current_exe()?.with_file_name("speech.exe")
    };
    if !executable.is_file() {
        bail!(
            "speech.cpp is missing: {}. Put speech.exe and speech.dll beside Hex or set HEX_SPEECH_CPP_EXE.",
            executable.display()
        );
    }
    Ok(executable)
}

fn read_message(reader: &mut impl BufRead) -> Result<Option<Value>> {
    let mut line = Vec::new();
    let bytes = reader
        .take((MAX_LINE_BYTES + 1) as u64)
        .read_until(b'\n', &mut line)?;
    if bytes == 0 {
        return Ok(None);
    }
    if bytes > MAX_LINE_BYTES || line.last() != Some(&b'\n') {
        bail!("oversized or unterminated worker message");
    }
    Ok(Some(serde_json::from_slice(&line)?))
}

fn validate_ready(message: &Value) -> Result<()> {
    if message["type"] != "ready" || message["protocol"] != 4 {
        bail!("speech.cpp protocol 4 is required: {message}");
    }
    Ok(())
}

fn write_message(writer: &mut impl Write, message: &Value) -> Result<()> {
    serde_json::to_writer(&mut *writer, message)?;
    writer.write_all(b"\n")?;
    writer.flush()?;
    Ok(())
}

fn write_request(writer: &mut impl Write, request: &Request) -> Result<()> {
    for (seq, chunk) in request.samples.chunks(SAMPLE_RATE).enumerate() {
        let pcm: Vec<u8> = chunk
            .iter()
            .flat_map(|sample| pcm_sample(*sample).to_le_bytes())
            .collect();
        write_message(
            writer,
            &json!({"type":"chunk", "id":request.id, "seq":seq, "pcm":STANDARD.encode(pcm)}),
        )?;
    }
    write_message(
        writer,
        &json!({"type":"transcribe", "id":request.id, "sample_rate":SAMPLE_RATE}),
    )
}

fn pcm_sample(sample: f32) -> i16 {
    if sample.is_finite() {
        (sample.clamp(-1.0, 1.0) * 32767.0).round() as i16
    } else {
        0
    }
}

fn has_pcm_audio(samples: &[f32]) -> bool {
    samples.iter().any(|sample| pcm_sample(*sample) != 0)
}

fn should_transcribe_piece(samples: &[f32], split: bool) -> bool {
    has_pcm_audio(samples)
        // Long-clip partitioning can leave room noise after the final words.
        // Reuse Hex's conservative speech gate rather than decoding that tail
        // into invented words. An unavailable detector never drops audio.
        && (!split || crate::speech_detector::contains_speech(samples) != Some(false))
}

fn response(message: &Value, id: &str) -> Result<Option<String>> {
    if message["id"].as_str() != Some(id) {
        bail!("unexpected speech.cpp response id");
    }
    match message["type"].as_str() {
        Some("progress") => Ok(None),
        Some("end") => {
            if message["stop"] != "complete" {
                bail!("speech.cpp returned an incomplete transcription");
            }
            Ok(Some(
                message["text"]
                    .as_str()
                    .ok_or_else(|| eyre!("speech.cpp returned no transcript"))?
                    .to_owned(),
            ))
        }
        Some("error") => bail!("speech.cpp: {}", message["error"]),
        Some("cancelled") => bail!("speech.cpp transcription cancelled"),
        _ => bail!("unexpected speech.cpp response type"),
    }
}

/// Preserve normal dictations whole: early pause cuts changed punctuation and
/// made a noisy tail a separate utterance. Only partition past the size bound,
/// preferring the latest natural pause; keep every sample in the planned ranges.
fn piece_ranges(samples: &[f32]) -> Vec<std::ops::Range<usize>> {
    use crate::pause_segmenter::{FRAME_SAMPLES, PauseSegmenter};
    if samples.is_empty() {
        return Vec::new();
    }
    if samples.len() <= MAX_PIECE_SAMPLES {
        return std::iter::once(0..samples.len()).collect();
    }
    let mut detector = PauseSegmenter::default();
    let mut pauses = Vec::new();
    for frame in samples.chunks(FRAME_SAMPLES) {
        if let Some(cut) = detector.push(frame) {
            pauses.push(cut);
        }
    }
    let mut start = 0;
    let mut ranges = Vec::new();
    while samples.len() - start > MAX_PIECE_SAMPLES {
        let limit = start + MAX_PIECE_SAMPLES;
        let natural = pauses
            .iter()
            .rev()
            .copied()
            .find(|cut| *cut >= start + 3 * SAMPLE_RATE && *cut <= limit);
        let cut = natural.unwrap_or_else(|| {
            let search = limit
                .saturating_sub(2 * SAMPLE_RATE)
                .max(start + SAMPLE_RATE);
            let index = samples[search..limit]
                .chunks(FRAME_SAMPLES)
                .enumerate()
                .min_by(|(_, a), (_, b)| energy(a).total_cmp(&energy(b)))
                .map_or(0, |(index, _)| index);
            search + index * FRAME_SAMPLES + FRAME_SAMPLES / 2
        });
        ranges.push(start..cut);
        start = cut;
    }
    ranges.push(start..samples.len());
    ranges
}

fn energy(samples: &[f32]) -> f32 {
    samples.iter().map(|s| s * s).sum::<f32>() / samples.len() as f32
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn haas_worker_requires_the_supported_protocol() {
        assert!(validate_ready(&json!({"type":"ready","protocol":4})).is_ok());
        assert!(validate_ready(&json!({"type":"ready","protocol":3})).is_err());
        assert!(validate_ready(&json!({"type":"fatal","error":"bad model"})).is_err());
    }

    #[test]
    fn haas_worker_never_accepts_partial_or_mismatched_text() {
        assert_eq!(
            response(
                &json!({"type":"end","id":"1","text":"日本語です。","stop":"complete"}),
                "1"
            )
            .unwrap(),
            Some("日本語です。".into())
        );
        assert!(
            response(
                &json!({"type":"end","id":"1","text":"途中","stop":"model_limit"}),
                "1"
            )
            .is_err()
        );
        assert!(
            response(
                &json!({"type":"end","id":"2","text":"別の録音","stop":"complete"}),
                "1"
            )
            .is_err()
        );
        assert!(
            response(
                &json!({"type":"error","id":"1","error":{"message":"failed"}}),
                "1"
            )
            .is_err()
        );
        assert!(
            response(&json!({"type":"progress","id":"1"}), "1")
                .unwrap()
                .is_none()
        );
    }

    #[test]
    fn haas_worker_messages_are_bounded_and_validated() {
        assert!(read_message(&mut &b"not json\n"[..]).is_err());
        assert!(read_message(&mut &b"{}"[..]).is_err());
        assert!(read_message(&mut &vec![b' '; MAX_LINE_BYTES + 1][..]).is_err());
        assert!(read_message(&mut &b""[..]).unwrap().is_none());
    }

    #[test]
    fn haas_worker_audio_is_pcm16_with_no_disk_artifact() {
        let mut bytes = Vec::new();
        write_request(
            &mut bytes,
            &Request {
                id: "7".into(),
                samples: vec![0.0, 1.0, -1.0, f32::NAN],
            },
        )
        .unwrap();
        let lines: Vec<Value> = String::from_utf8(bytes)
            .unwrap()
            .lines()
            .map(|line| serde_json::from_str(line).unwrap())
            .collect();
        assert_eq!(lines.len(), 2);
        assert_eq!(
            STANDARD.decode(lines[0]["pcm"].as_str().unwrap()).unwrap(),
            [0, 0, 255, 127, 1, 128, 0, 0]
        );
        assert_eq!(
            lines[1],
            json!({"type":"transcribe","id":"7","sample_rate":16000})
        );
        assert!(!has_pcm_audio(&[0.0, f32::NAN, 1e-8]));
        assert!(has_pcm_audio(&[0.0, 1e-4]));
    }

    #[test]
    fn haas_pieces_cover_all_audio_once_and_stay_bounded() {
        for length in [0, 1, 319, 16_000, 240_001, 1_013_333] {
            let samples = vec![0.1; length];
            let ranges = piece_ranges(&samples);
            let mut end = 0;
            for range in ranges {
                assert_eq!(range.start, end);
                assert!(range.len() <= MAX_PIECE_SAMPLES + 320);
                assert!(!range.is_empty());
                end = range.end;
            }
            assert_eq!(end, length);
        }
    }

    #[test]
    fn haas_short_dictations_are_not_split_at_pauses() {
        let mut samples = vec![0.0; 1_600];
        samples.extend(vec![0.1; 48_000]);
        samples.extend(vec![0.0; 16_000]);
        samples.extend(vec![0.1; 48_000]);
        samples.extend(vec![0.0; 16_000]);
        assert_eq!(
            piece_ranges(&samples),
            std::iter::once(0..samples.len()).collect::<Vec<_>>()
        );
    }

    #[test]
    fn haas_split_tails_use_the_speech_gate_without_gating_whole_clips() {
        let samples = vec![0.001; SAMPLE_RATE];
        {
            let _guard = crate::speech_detector::override_for_test(Some(false));
            assert!(!should_transcribe_piece(&samples, true));
            assert!(should_transcribe_piece(&samples, false));
        }
        let _guard = crate::speech_detector::override_for_test(None);
        assert!(should_transcribe_piece(&samples, true));
        assert!(!should_transcribe_piece(&vec![0.0; SAMPLE_RATE], true));
    }

    #[test]
    #[ignore = "requires HEX_HAAS_TEST_MODEL, HEX_HAAS_TEST_PUBLIC_DIR and speech.cpp"]
    fn haas_public_clips_preserve_default_punctuation() {
        let model = std::env::var_os("HEX_HAAS_TEST_MODEL").expect("HEX_HAAS_TEST_MODEL");
        let directory =
            std::env::var_os("HEX_HAAS_TEST_PUBLIC_DIR").expect("HEX_HAAS_TEST_PUBLIC_DIR");
        let mut worker = SpeechCpp::load(Path::new(&model)).unwrap();
        for name in [
            "10020345318418093976-pcm16.wav",
            "10086073819084529371-pcm16.wav",
            "10112379500600537933-pcm16.wav",
        ] {
            let samples =
                super::super::read_wav_for_parakeet(&Path::new(&directory).join(name)).unwrap();
            assert!(samples.len() <= MAX_PIECE_SAMPLES);
            let whole = worker.transcribe_piece(&samples).unwrap();
            let adapted = worker.transcribe(&samples).unwrap();
            assert!(
                whole.contains('、') && whole.contains('。'),
                "{name}: {whole}"
            );
            assert_eq!(
                adapted, whole,
                "the adapter changed the native transcript: {name}"
            );
            eprintln!("{name}: {adapted}");
        }
    }

    #[test]
    #[ignore = "requires an explicitly supplied Haas model, speech.cpp executable, and WAV fixture"]
    fn haas_worker_transcribes_a_fixture_twice() {
        let model = std::env::var_os("HEX_HAAS_TEST_MODEL").expect("HEX_HAAS_TEST_MODEL");
        let wav = std::env::var_os("HEX_HAAS_TEST_WAV").expect("HEX_HAAS_TEST_WAV");
        let samples = super::super::read_wav_for_parakeet(Path::new(&wav)).unwrap();
        let mut worker = SpeechCpp::load(Path::new(&model)).unwrap();
        let start = Instant::now();
        let first = worker.transcribe(&samples).unwrap();
        let first_ms = start.elapsed().as_millis();
        let start = Instant::now();
        let second = worker.transcribe(&samples).unwrap();
        assert!(!first.trim().is_empty());
        assert_eq!(first, second);
        eprintln!(
            "Haas fixture: first={first_ms} ms, second={} ms, text={first}",
            start.elapsed().as_millis()
        );
        let cancel = transcribe_cpp::CancelToken::new();
        cancel.cancel();
        let start = Instant::now();
        assert!(
            crate::gguf_session::with_job_cancel(&cancel, || worker.transcribe(&samples)).is_err()
        );
        assert!(start.elapsed() < Duration::from_secs(5));
        assert!(worker.child.is_none());
        assert_eq!(worker.transcribe(&samples).unwrap(), first);
        let long = samples.repeat(6);
        let transcript = worker.transcribe(&long).unwrap();
        let reference_term = "音声認識";
        if first.contains(reference_term) {
            assert_eq!(
                transcript.matches(reference_term).count(),
                6,
                "{transcript}"
            );
        }
    }
}
