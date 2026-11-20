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
                let count = config
                    .extra_headers
                    .split(['\n', ';'])
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
                config.extra_headers = value
                    .split(';')
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
    Some(section)
}

fn field_row(ui: &Ui, p: Paint, field: Field, config: &CloudApiConfig) -> El {
    if let Some((editing, editor)) = &ui.picker.cloud.editing
        && *editing == field
    {
        return row()
            .key(field.key())
            .gap(8.0)
            .center_y()
            .child(
                TextInput::new(editor)
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
                    .on_submit(move |ui: &mut Ui, _| save(ui))
                    .build(p.k)
                    .key("field"),
            )
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
                let config = crate::app_settings::cloud_api_config();
                ui.picker.cloud.editing = Some((field, TextEdit::new(field.value(&config))));
            },
        ))
}

fn save(ui: &mut Ui) {
    let Some((field, editor)) = ui.picker.cloud.editing.take() else {
        return;
    };
    if !ui.live() {
        return;
    }
    let value = editor.text.clone();
    ui.update(|settings| field.set(&mut settings.cloud_api, value));
}
