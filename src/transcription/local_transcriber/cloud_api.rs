//! This fork's cloud transcription backend: an OpenAI-compatible
//! transcription API (OpenAI, Azure OpenAI, or a compatible endpoint),
//! configured in Settings and edited from the model picker's Cloud API
//! card.
//!
//! The endpoint is the user's own (typically a company-internal Azure
//! OpenAI deployment on a GPU-less VDI), so the request shape is kept
//! generic: a full URL, an API key under a configurable header name, an
//! optional model field, and arbitrary extra headers.
//!
//! The API key and the extra header values are credentials: they are
//! never written to logs or error messages, and error text carries only
//! the endpoint's host, the HTTP status, and the API's own error code
//! and message.

use std::io::Cursor;
use std::time::Duration;

use color_eyre::eyre::{Result, WrapErr, eyre};

use crate::app_settings::CloudApiConfig;
use crate::transcription::models::AUTO_LANGUAGE;

/// Audio is sent in chunks of this many samples (30 s at 16 kHz), like
/// the speech.cpp backend's timed path: a 30 s WAV is about 1 MB, far
/// under the transcription API's 25 MB request limit, whatever the
/// clip's total length.
const CHUNK_SAMPLES: usize = 30 * 16_000;

/// One request's total budget. The dictation engine allows a job 180 s
/// plus 8 s per second of audio, so a single chunk's request stays well
/// inside it; a stall surfaces as this error instead of a wedged engine.
const REQUEST_TIMEOUT: Duration = Duration::from_secs(120);
/// Response bodies are transcripts: small. The cap keeps a broken or
/// hostile endpoint from streaming into memory until the timeout.
const MAX_RESPONSE_BYTES: u64 = 8 * 1024 * 1024;
/// Only the head of an error body is parsed for its error code/message.
const MAX_ERROR_BYTES: u64 = 64 * 1024;

/// A loaded cloud "session". It holds no connection and no settings
/// snapshot: every transcription reads the current configuration, so
/// editing the endpoint or key takes effect without a model reload.
pub struct CloudSession {
    /// The selection's language, already mapped to the API's code by the
    /// model definition (`fil` arrives as `tl`). `AUTO_LANGUAGE` and
    /// codes the API does not document are sent as no hint at all.
    language: String,
}

impl CloudSession {
    pub fn new(language: String) -> Self {
        Self { language }
    }

    /// Transcribes a complete 16 kHz clip, in chunks, joined as the
    /// speech.cpp backend joins its chunks. Silent chunks are skipped:
    /// the API invents words for silence, and Hex's speech detector has
    /// already gated the clip as a whole.
    pub fn transcribe(&mut self, samples: &[f32]) -> Result<String> {
        let config = crate::app_settings::cloud_api_config();
        if !config.configured() {
            return Err(eyre!(
                "the Cloud API model has no endpoint yet; add one on its card in the model picker"
            ));
        }
        let agent = agent_for(&config.endpoint);
        let mut texts = Vec::new();
        for chunk in samples.chunks(CHUNK_SAMPLES) {
            if !has_pcm_audio(chunk) {
                continue;
            }
            let text = transcribe_chunk(&agent, &config, &self.language, chunk)?;
            let text = text.trim();
            if !text.is_empty() {
                texts.push(text.to_owned());
            }
        }
        Ok(texts.join(" "))
    }
}

/// The language hint to send, if any: Auto sends none, and only the
/// two-letter ISO codes the API documents are passed through.
fn language_hint(language: &str) -> Option<&str> {
    if language == AUTO_LANGUAGE || language.len() != 2 {
        return None;
    }
    Some(language)
}

/// The shared Hex agent for this endpoint: platform TLS and roots (so a
/// company's own root certificate is trusted) and the proxy this
/// computer uses for the URL, Windows' system proxy included — what a
/// managed VDI needs. Redirects are off: one could carry the audio and
/// the credentials to another host, and a transcription endpoint never
/// needs one.
fn agent_for(endpoint: &str) -> ureq::Agent {
    let configure = |config: crate::net::Config| {
        config
            .timeout_global(Some(REQUEST_TIMEOUT))
            .max_redirects(0)
            .http_status_as_error(false)
    };
    if is_loopback(endpoint) {
        // A plaintext loopback endpoint must not be routed through a
        // system proxy: the direct agent also refuses any resolution
        // that leaves this computer.
        crate::net::loopback_agent(configure)
    } else {
        crate::net::agent(endpoint, configure)
    }
}

fn is_loopback(endpoint: &str) -> bool {
    let host = crate::net::host(endpoint);
    host == "localhost" || host == "127.0.0.1" || host == "::1"
}

/// HTTPS is required, except for loopback, which is how the local mock
/// server in tests is reached.
fn check_endpoint(endpoint: &str) -> Result<()> {
    if endpoint.starts_with("https://") {
        return Ok(());
    }
    let host = crate::net::host(endpoint);
    if endpoint.starts_with("http://") && is_loopback(endpoint) {
        return Ok(());
    }
    Err(eyre!(
        "the Cloud API endpoint must use HTTPS (got {host}); transcription sends your voice and API key"
    ))
}

/// Everything that must hold for the configuration to be usable,
/// checked when the model reports itself ready and again before a
/// request: a well-formed HTTPS (or loopback) endpoint, a usable key
/// header, and parseable extra headers.
pub(crate) fn validate_config(config: &CloudApiConfig) -> Result<()> {
    if !config.configured() {
        return Err(eyre!("the Cloud API endpoint is not configured"));
    }
    check_endpoint(&config.endpoint)?;
    url::Url::parse(&config.endpoint)
        .map_err(|_| eyre!("the Cloud API endpoint is not a valid URL"))?;
    request_headers(config).map(|_| ())
}

fn valid_header_name(name: &str) -> bool {
    !name.is_empty()
        && name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || "!#$%&'*+-.^_`|~".contains(c))
}

/// Headers the transport owns and a setting may never override.
fn is_transport_header(name: &str) -> bool {
    matches!(
        name.to_ascii_lowercase().as_str(),
        "content-type" | "content-length" | "host"
    )
}

/// The headers a request carries: the API key under its configured
/// header, plus the parsed extra headers.
fn request_headers(config: &CloudApiConfig) -> Result<Vec<(String, String)>> {
    let extra = parse_extra_headers(&config.extra_headers)?;
    let mut headers = Vec::new();
    // The key header comes from a setting too: it gets the same
    // validation as an extra header, so it can never become a
    // transport-owned header carrying the key.
    let key_name = config.key_header_name();
    if !config.key_header.trim().is_empty()
        && (!valid_header_name(key_name) || is_transport_header(key_name))
    {
        return Err(eyre!(
            "the key header name cannot be used; name a header the request does not own"
        ));
    }
    if !config.api_key.is_empty() {
        let name = key_name;
        if extra
            .iter()
            .any(|(extra_name, _)| extra_name.eq_ignore_ascii_case(name))
        {
            return Err(eyre!(
                "the extra headers already set {name}; remove it there or clear the API key"
            ));
        }
        let value = if name.eq_ignore_ascii_case("Authorization") {
            format!("Bearer {}", config.api_key)
        } else {
            config.api_key.clone()
        };
        headers.push((name.to_owned(), value));
    }
    headers.extend(extra);
    Ok(headers)
}

/// Splits the extra-headers text into entries: one per line, or
/// several on one line separated by `;` — but a `;` only separates
/// where a new `Name:` entry follows it, so a value that contains
/// `;` (a Cookie header) survives.
pub(crate) fn split_entries(text: &str) -> Vec<&str> {
    fn is_name_byte(byte: u8) -> bool {
        byte.is_ascii_alphanumeric() || b"!#$%&'*+-.^_`|~".contains(&byte)
    }
    let mut entries = Vec::new();
    for line in text.lines() {
        let bytes = line.as_bytes();
        let mut start = 0;
        let mut i = 0;
        while i < bytes.len() {
            if bytes[i] == b';' {
                let mut j = i + 1;
                while j < bytes.len() && (bytes[j] == b' ' || bytes[j] == b'\t') {
                    j += 1;
                }
                let name_start = j;
                while j < bytes.len() && is_name_byte(bytes[j]) {
                    j += 1;
                }
                if j > name_start && j < bytes.len() && bytes[j] == b':' {
                    entries.push(&line[start..i]);
                    start = i + 1;
                }
            }
            i += 1;
        }
        entries.push(&line[start..]);
    }
    entries
}

/// Parses the `Name: Value` entries of the extra-headers setting,
/// separated by lines or `;`. A malformed entry is an error, never
/// silently dropped: a missing header on an internal endpoint fails
/// authentication in ways that are hard to see. Headers the transport
/// owns (Content-Type, Content-Length, Host) cannot be overridden.
fn parse_extra_headers(text: &str) -> Result<Vec<(String, String)>> {
    let mut headers = Vec::new();
    for (index, line) in split_entries(text).into_iter().enumerate() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let Some((name, value)) = line.split_once(':') else {
            return Err(eyre!(
                "extra header {} is not in `Name: Value` form",
                index + 1
            ));
        };
        let name = name.trim();
        let value = value.trim();
        if !valid_header_name(name) {
            return Err(eyre!("extra header {} has an invalid name", index + 1));
        }
        if value.contains(['\r', '\n']) {
            return Err(eyre!(
                "extra header {} has an invalid value",
                index + 1
            ));
        }
        if is_transport_header(name) {
            return Err(eyre!(
                "the extra headers cannot set {name}; the request sets it itself"
            ));
        }
        headers.push((name.to_owned(), value.to_owned()));
    }
    Ok(headers)
}

fn transcribe_chunk(
    agent: &ureq::Agent,
    config: &CloudApiConfig,
    language: &str,
    samples: &[f32],
) -> Result<String> {
    check_endpoint(&config.endpoint)?;
    let host = crate::net::host(&config.endpoint);
    let wav = encode_wav(samples).wrap_err("could not encode the clip as WAV")?;
    let boundary = boundary();
    let body = multipart_body(
        &boundary,
        &wav,
        (!config.model_name.trim().is_empty()).then(|| config.model_name.trim()),
        language_hint(language),
    );
    let mut request = agent
        .post(&config.endpoint)
        .header(
            "Content-Type",
            &format!("multipart/form-data; boundary={boundary}"),
        );
    for (name, value) in request_headers(config)? {
        request = request.header(name.as_str(), value.as_str());
    }
    let mut response = request
        .send(body)
        .map_err(|error| eyre!("{}", crate::net::unreachable(&config.endpoint, &error)))?;
    let status = response.status().as_u16();
    if status != 200 {
        let body = read_capped(&mut response, MAX_ERROR_BYTES);
        return Err(api_error(config, status, &body));
    }
    let body = response
        .body_mut()
        .with_config()
        .limit(MAX_RESPONSE_BYTES)
        .read_to_vec()
        .map_err(|_| eyre!("could not read the transcription response from {host}"))?;
    let value: serde_json::Value = serde_json::from_slice(&body)
        .map_err(|_| eyre!("the transcription response from {host} was not JSON"))?;
    value
        .get("text")
        .and_then(serde_json::Value::as_str)
        .map(str::to_owned)
        .ok_or_else(|| eyre!("the transcription response from {host} had no text"))
}

/// An HTTP error, from the API's own error code and message only. The
/// raw body is never quoted: a gateway can echo request headers or the
/// URL into it.
fn api_error(config: &CloudApiConfig, status: u16, body: &[u8]) -> color_eyre::eyre::Report {
    let host = crate::net::host(&config.endpoint);
    let parsed = serde_json::from_slice::<serde_json::Value>(body).ok();
    let error = parsed.as_ref().and_then(|value| value.get("error"));
    let code = error
        .and_then(|error| error.get("code"))
        .map(|code| match code {
            serde_json::Value::String(text) => text.clone(),
            serde_json::Value::Number(number) => number.to_string(),
            _ => String::new(),
        })
        .unwrap_or_default();
    let message = error
        .and_then(|error| error.get("message"))
        .and_then(serde_json::Value::as_str)
        .unwrap_or("");
    // An endpoint can echo the credentials it was sent (a gateway
    // naming the bad key, say): scrub the configured secrets from
    // whatever is quoted, then cap the length.
    let code = redact_secrets(&code, config);
    let message = redact_secrets(message, config);
    let message: String = message.chars().take(200).collect();
    let code = code.as_str();
    match (code.is_empty(), message.is_empty()) {
        (false, false) => eyre!("{host} answered HTTP {status} ({code}): {message}"),
        (false, true) => eyre!("{host} answered HTTP {status} ({code})"),
        (true, false) => eyre!("{host} answered HTTP {status}: {message}"),
        (true, true) => eyre!("{host} answered HTTP {status}"),
    }
}

/// Replaces the configured secrets in `text`: the API key and every
/// extra header value long enough to be distinctive.
fn redact_secrets(text: &str, config: &CloudApiConfig) -> String {
    let mut out = text.to_owned();
    let mut secrets: Vec<String> = vec![config.api_key.clone()];
    if let Ok(headers) = parse_extra_headers(&config.extra_headers) {
        secrets.extend(headers.into_iter().map(|(_, value)| value));
    }
    for secret in &secrets {
        if secret.len() >= 4 {
            out = out.replace(secret.as_str(), "***");
        }
    }
    out
}

fn read_capped(response: &mut ureq::http::Response<ureq::Body>, cap: u64) -> Vec<u8> {
    response
        .body_mut()
        .with_config()
        .limit(cap)
        .read_to_vec()
        .unwrap_or_default()
}

/// A multipart boundary that cannot appear in a WAV body in practice:
/// time- and process-derived, and long.
fn boundary() -> String {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    format!("hex-haas-{:x}-{:x}", std::process::id(), nanos)
}

fn multipart_body(
    boundary: &str,
    wav: &[u8],
    model: Option<&str>,
    language: Option<&str>,
) -> Vec<u8> {
    let mut body = Vec::with_capacity(wav.len() + 512);
    // The file part first: the `field` closure below borrows `body`
    // mutably for the rest of the function.
    body.extend_from_slice(
        format!(
            "--{boundary}\r\nContent-Disposition: form-data; name=\"file\"; filename=\"audio.wav\"\r\nContent-Type: audio/wav\r\n\r\n"
        )
        .as_bytes(),
    );
    body.extend_from_slice(wav);
    body.extend_from_slice(b"\r\n");
    {
        let mut field = |name: &str, value: &str| {
            body.extend_from_slice(
                format!("--{boundary}\r\nContent-Disposition: form-data; name=\"{name}\"\r\n\r\n{value}\r\n").as_bytes(),
            );
        };
        if let Some(model) = model {
            field("model", model);
        }
        if let Some(language) = language {
            field("language", language);
        }
        field("response_format", "json");
    }
    body.extend_from_slice(format!("--{boundary}--\r\n").as_bytes());
    body
}

/// 16 kHz mono PCM16 WAV, the shape every transcription input already
/// has by the time it reaches a backend.
fn encode_wav(samples: &[f32]) -> Result<Vec<u8>> {
    let spec = hound::WavSpec {
        channels: 1,
        sample_rate: 16_000,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };
    let mut cursor = Cursor::new(Vec::new());
    {
        let mut writer = hound::WavWriter::new(&mut cursor, spec)?;
        for &sample in samples {
            writer.write_sample(pcm_sample(sample))?;
        }
        writer.finalize()?;
    }
    Ok(cursor.into_inner())
}

/// The f32-to-PCM16 conversion the speech.cpp backend uses, so silence
/// means the same thing in both: a sample that converts to zero.
fn pcm_sample(sample: f32) -> i16 {
    (sample.clamp(-1.0, 1.0) * i16::MAX as f32) as i16
}

fn has_pcm_audio(samples: &[f32]) -> bool {
    samples.iter().any(|sample| pcm_sample(*sample) != 0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn config(extra_headers: &str) -> CloudApiConfig {
        CloudApiConfig {
            endpoint: "https://example.openai.azure.com/openai/deployments/x/audio/transcriptions?api-version=2025-01-01-preview".into(),
            api_key: "secret".into(),
            key_header: String::new(),
            model_name: String::new(),
            extra_headers: extra_headers.into(),
        }
    }

    #[test]
    fn bearer_is_the_default_key_header() {
        let headers = request_headers(&config("")).unwrap();
        assert_eq!(
            headers,
            vec![("Authorization".to_owned(), "Bearer secret".to_owned())]
        );
    }

    #[test]
    fn azure_style_key_header_sends_the_key_as_is() {
        let mut config = config("");
        config.key_header = "api-key".into();
        let headers = request_headers(&config).unwrap();
        assert_eq!(headers, vec![("api-key".to_owned(), "secret".to_owned())]);
    }

    #[test]
    fn empty_key_sends_no_auth_header() {
        let mut config = config("Authorization: Bearer from-elsewhere");
        config.api_key = String::new();
        let headers = request_headers(&config).unwrap();
        assert_eq!(
            headers,
            vec![(
                "Authorization".to_owned(),
                "Bearer from-elsewhere".to_owned()
            )]
        );
    }

    #[test]
    fn extra_header_colliding_with_the_key_header_is_an_error() {
        assert!(request_headers(&config("authorization: Bearer other")).is_err());
    }

    #[test]
    fn transport_headers_cannot_be_overridden() {
        assert!(parse_extra_headers("Content-Type: text/plain").is_err());
        assert!(parse_extra_headers("Host: example.com").is_err());
    }

    #[test]
    fn malformed_extra_header_lines_are_errors() {
        assert!(parse_extra_headers("no colon here").is_err());
        assert!(parse_extra_headers("bad name: value").is_err());
        assert_eq!(
            parse_extra_headers("X-Api-Version: 2\n\n X-Trace : on ").unwrap(),
            vec![
                ("X-Api-Version".to_owned(), "2".to_owned()),
                ("X-Trace".to_owned(), "on".to_owned()),
            ]
        );
    }

    #[test]
    fn endpoints_must_be_https_except_loopback() {
        assert!(check_endpoint("https://example.com/v1/audio/transcriptions").is_ok());
        assert!(check_endpoint("http://example.com/v1/audio/transcriptions").is_err());
        assert!(check_endpoint("http://127.0.0.1:8080/v1/audio/transcriptions").is_ok());
        assert!(check_endpoint("http://localhost:8080/v1/audio/transcriptions").is_ok());
    }

    #[test]
    fn error_text_never_quotes_the_body_or_the_key() {
        // The endpoint echoes the key it rejected: the message is
        // quoted only after the configured secrets are scrubbed.
        let body = br#"{"error":{"code":"401","message":"bad key secret"}}"#;
        let error = api_error(&config(""), 401, body);
        let text = format!("{error}");
        assert!(text.contains("401"), "{text}");
        assert!(!text.contains("secret"), "{text}");
        assert!(text.contains("***"), "{text}");
    }

    #[test]
    fn semicolons_inside_a_value_are_not_separators() {
        assert_eq!(
            parse_extra_headers("Cookie: a=1; b=2\nX-Trace: on").unwrap(),
            vec![
                ("Cookie".to_owned(), "a=1; b=2".to_owned()),
                ("X-Trace".to_owned(), "on".to_owned()),
            ]
        );
        assert_eq!(
            parse_extra_headers("X-One: 1; X-Two: 2").unwrap(),
            vec![
                ("X-One".to_owned(), "1".to_owned()),
                ("X-Two".to_owned(), "2".to_owned()),
            ]
        );
    }

    #[test]
    fn multipart_carries_file_model_and_language() {
        let body = multipart_body("BOUNDARY", b"RIFF", Some("gpt-transcribe"), Some("ja"));
        let text = String::from_utf8_lossy(&body);
        assert!(text.contains("name=\"file\"; filename=\"audio.wav\""), "{text}");
        assert!(text.contains("name=\"model\"\r\n\r\ngpt-transcribe"), "{text}");
        assert!(text.contains("name=\"language\"\r\n\r\nja"), "{text}");
        assert!(text.contains("name=\"response_format\"\r\n\r\njson"), "{text}");
        assert!(text.ends_with("--BOUNDARY--\r\n"), "{text}");
    }

    #[test]
    fn language_hints_are_two_letter_codes_only() {
        assert_eq!(language_hint("ja"), Some("ja"));
        assert_eq!(language_hint("auto"), None);
        assert_eq!(language_hint("yue"), None);
    }
}
