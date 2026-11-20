//! This fork's Cloud API model setup, on its card in the Local
//! transcription picker: the endpoint, the API key and its header, the
//! model name, and any extra headers. The card is always listed (an
//! unconfigured cloud model is "not installed", like an undownloaded
//! one), so setup is reachable before the model can be chosen; saving a
//! field applies the settings, which republishes the configuration the
//! transcriber reads on every request.

use rune::{El, TextEdit, col, edit_access, row, text};
use rune_kit::TextInput;

use super::Ui;
use super::controls::{ButtonKind, Paint, button};
use crate::app_settings::CloudApiConfig;
use crate::desktop::i18n::{tr, tr_named};
use crate::transcription::models::{ModelDefinition, TranscriptionModelId};

#[derive(Default)]
pub(super) struct CloudSetupState {
    /// The field being edited, with its text field (the rename pattern
    /// from `model_import`): one editor at a time.
    editing: Option<(Field, TextEdit)>,
    /// Why the saved configuration is not usable yet, after a save.
    error: Option<String>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Field {
    Endpoint,
    ApiKey,
    KeyHeader,
    ModelName,
    ExtraHeaders,
}

impl Field {
    const ALL: [Field; 5] = [
        Field::Endpoint,
        Field::ApiKey,
        Field::KeyHeader,
        Field::ModelName,
        Field::ExtraHeaders,
    ];

    fn label(self) -> &'static str {
        match self {
            Field::Endpoint => "Endpoint",
            Field::ApiKey => "API key",
            Field::KeyHeader => "Key header",
            Field::ModelName => "Model",
            Field::ExtraHeaders => "Extra headers",
        }
    }

    fn placeholder(self) -> &'static str {
        match self {
            Field::Endpoint => "https://…/audio/transcriptions",
            Field::ApiKey => "API key",
            Field::KeyHeader => "Authorization",
            Field::ModelName => "gpt-transcribe",
            Field::ExtraHeaders => "Name: Value; Name: Value",
        }
    }

    fn key(self) -> &'static str {
        match self {
            Field::Endpoint => "cloud-endpoint",
            Field::ApiKey => "cloud-api-key",
            Field::KeyHeader => "cloud-key-header",
            Field::ModelName => "cloud-model-name",
            Field::ExtraHeaders => "cloud-extra-headers",
        }
    }

    fn value(self, config: &CloudApiConfig) -> String {
        match self {
            Field::Endpoint => config.endpoint.clone(),
            Field::ApiKey => config.api_key.clone(),
            Field::KeyHeader => config.key_header.clone(),
            Field::ModelName => config.model_name.clone(),
            // The editor is one line; headers join with `;`, which the
            // request parser accepts alongside one-per-line.
            Field::ExtraHeaders => config
                .extra_headers
                .lines()
                .map(str::trim)
                .filter(|line| !line.is_empty())
                .collect::<Vec<_>>()
                .join("; "),
        }
    }

    fn display(self, config: &CloudApiConfig) -> String {
        match self {
            // The key itself is never rendered back.
            Field::ApiKey => {
                if config.api_key.is_empty() {
                    tr("Not set").into()
                } else {
                    "••••••••".into()
                }
            }
            Field::ExtraHeaders => {
                let count =
                    crate::transcription::local_transcriber::cloud_api::split_entries(
                        &config.extra_headers,
                    )
                    .into_iter()
                    .filter(|line| !line.trim().is_empty())
                    .count();
                if count == 0 {
                    tr("None").into()
                } else {
                    tr_named("{count} headers", &[("count", &count.to_string())])
                }
            }
            _ => {
                let value = self.value(config);
                if value.trim().is_empty() {
                    tr("Not set").into()
                } else {
                    value
                }
            }
        }
    }

    fn set(self, config: &mut CloudApiConfig, value: String) {
        match self {
            Field::Endpoint => config.endpoint = value.trim().to_owned(),
            Field::ApiKey => config.api_key = value.trim().to_owned(),
            Field::KeyHeader => config.key_header = value.trim().to_owned(),
            Field::ModelName => config.model_name = value.trim().to_owned(),
            // Store one per line, however they were typed.
            Field::ExtraHeaders => {
                config.extra_headers =
                    crate::transcription::local_transcriber::cloud_api::split_entries(&value)
                        .into_iter()
                        .map(str::trim)
                        .filter(|line| !line.is_empty())
                        .collect::<Vec<_>>()
                        .join("\n")
            }
        }
    }
}

/// The setup form on the Cloud API card, below its details.
pub(super) fn section(ui: &Ui, p: Paint, def: &ModelDefinition) -> Option<El> {
    if def.id != TranscriptionModelId::CloudApi {
        return None;
    }
    let config = crate::app_settings::cloud_api_config();
    let mut section = col()
        .key("cloud-setup")
        .role("none")
        .gap(6.0)
        .pt(4.0)
        // Reading or editing the setup never chooses the card's model.
        .on_press(|_: &mut Ui| {})
        .child(
            row().key("cloud-scope").child(
                text(tr(
                    "Choosing this model sends all transcription — dictation, meetings and files — to this endpoint.",
                ))
                .key("label")
                .font(p.k.ty.small)
                .color(p.muted())
                .wrap(),
            ),
        );
    for field in Field::ALL {
        section = section.child(field_row(ui, p, field, &config));
    }
    if let Some(error) = &ui.picker.cloud.error {
        section = section.child(
            row().key("cloud-error").child(
                text(error.clone())
                    .key("label")
                    .font(p.k.ty.small)
                    .color(p.warning())
                    .wrap(),
            ),
        );
    }
    Some(section)
}

fn field_row(ui: &Ui, p: Paint, field: Field, config: &CloudApiConfig) -> El {
    if let Some((editing, editor)) = &ui.picker.cloud.editing
        && *editing == field
    {
        let mut input = TextInput::new(editor)
            .placeholder(tr(field.placeholder()))
            .width(Some(280.0))
            .on_edit(edit_access(move |ui: &mut Ui| {
                &mut ui
                    .picker
                    .cloud
                    .editing
                    .get_or_insert_with(|| (field, TextEdit::default()))
                    .1
            }))
            .on_submit(move |ui: &mut Ui, _| save(ui));
        // The key and the extra headers can both carry credentials:
        // their editors mask the text and refuse copy.
        if matches!(field, Field::ApiKey | Field::ExtraHeaders) {
            input = input.secure();
        }
        return row()
            .key(field.key())
            .gap(8.0)
            .center_y()
            .child(input.build(p.k).key("field"))
            .child(button(
                p,
                "cloud-save",
                tr("Save"),
                ButtonKind::Secondary,
                |ui: &mut Ui, _| save(ui),
            ));
    }
    row()
        .key(field.key())
        .gap(8.0)
        .center_y()
        .child(
            text(format!("{}: {}", tr(field.label()), field.display(config)))
                .key("label")
                .font(p.k.ty.small)
                .wrap(),
        )
        .child(button(
            p,
            field.key(),
            tr("Edit"),
            ButtonKind::Secondary,
            move |ui: &mut Ui, _| {
                // Switching fields commits the draft in progress rather
                // than dropping it.
                save(ui);
                let config = crate::app_settings::cloud_api_config();
                ui.picker.cloud.editing = Some((field, TextEdit::new(field.value(&config))));
            },
        ))
}

fn save(ui: &mut Ui) {
    if !ui.live() {
        return;
    }
    let Some((field, editor)) = ui.picker.cloud.editing.take() else {
        return;
    };
    let value = editor.text.clone();
    // Credentials belong to the endpoint they were entered for: moving
    // to a different host clears the key and the extra headers in the
    // same save, so the next transcription cannot send them to the new
    // host before they are replaced.
    let clear_credentials = field == Field::Endpoint && {
        let current = crate::app_settings::cloud_api_config();
        crate::net::host(&current.endpoint) != crate::net::host(&value)
    };
    ui.update(|settings| {
        field.set(&mut settings.cloud_api, value.clone());
        if clear_credentials {
            settings.cloud_api.api_key.clear();
            settings.cloud_api.extra_headers.clear();
        }
    });
    // Say on the card when what is saved cannot be used yet; an empty
    // endpoint mid-setup is not an error, just unfinished.
    let mut candidate = crate::app_settings::cloud_api_config();
    field.set(&mut candidate, value);
    if clear_credentials {
        candidate.api_key.clear();
        candidate.extra_headers.clear();
    }
    ui.picker.cloud.error = if candidate.configured() {
        crate::transcription::local_transcriber::cloud_api::validate_config(&candidate)
            .err()
            .map(|error| format!("{error:#}"))
    } else {
        None
    };
}
