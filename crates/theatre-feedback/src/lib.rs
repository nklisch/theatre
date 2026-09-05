//! Project-local human evidence. Godot publishes immutable directories; readers
//! share only a separate handled annotation. No live engine connection is needed.
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::{
    fs,
    io::{Read, Seek, SeekFrom},
    path::{Path, PathBuf},
};

pub mod mcp;

pub const MAX_ITEMS: usize = 64;
pub const MAX_STORAGE_BYTES: u64 = 128 * 1024 * 1024;
pub const MAX_IMAGE_BYTES: u64 = 8 * 1024 * 1024;
pub const MAX_NOTE_BYTES: usize = 16 * 1024;
const MAX_METADATA_BYTES: u64 = 64 * 1024;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("feedback storage: {0}")]
    Io(#[from] std::io::Error),
    #[error("invalid feedback metadata: {0}")]
    Json(#[from] serde_json::Error),
    #[error("{0}")]
    Invalid(String),
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Source {
    Runtime,
    Editor,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct Dimensions {
    pub width: u32,
    pub height: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum Pointer {
    Inside { position: [f64; 2] },
    Outside,
    Unavailable,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct Selection {
    pub path: String,
    pub class: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum Capture {
    Available {
        source_dimensions: Dimensions,
        output_dimensions: Dimensions,
    },
    Unavailable {
        reason: String,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct FeedbackItem {
    pub feedback_id: String,
    pub source: Source,
    pub timestamp_ms: u64,
    pub project_path: String,
    pub process_id: u32,
    pub run_id: Option<String>,
    pub scene: String,
    pub surface: String,
    pub selection: Vec<Selection>,
    pub pointer: Pointer,
    pub capture: Capture,
    /// Latest completed render, read synchronously before opening the composer.
    pub readback_render_frame: u64,
    pub readback_physics_frame: u64,
    pub note: String,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct FeedbackSummary {
    pub feedback_id: String,
    pub source: Source,
    pub timestamp_ms: u64,
    pub scene: String,
    pub handled: bool,
    pub has_image: bool,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct IncompleteItem {
    pub directory: String,
    pub storage_bytes: u64,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct Status {
    pub items: Vec<FeedbackSummary>,
    pub pending_count: usize,
    pub storage_bytes: u64,
    pub incomplete: Vec<IncompleteItem>,
    pub max_items: usize,
    pub max_storage_bytes: u64,
}

#[derive(Debug, Deserialize, Serialize, JsonSchema)]
#[serde(tag = "action", rename_all = "snake_case", deny_unknown_fields)]
#[schemars(transform = object_schema)]
pub enum Operation {
    /// List retained evidence, shared handling and incomplete publication storage.
    Status,
    /// Read metadata and image without handling or deleting the item.
    Retrieve { feedback_id: String },
    /// Suppress pending notices for every reader; retain evidence.
    Handle { feedback_id: String },
    /// Explicitly delete retained evidence and its handled annotation.
    Delete { feedback_id: String },
    /// Delete one incomplete directory shown by status. May interrupt a writer;
    /// only request after confirming that capture is no longer wanted.
    Cleanup { directory: String },
}

#[derive(Debug, Serialize, JsonSchema)]
#[serde(tag = "action", rename_all = "snake_case")]
#[schemars(transform = object_schema)]
pub enum Response {
    Status(Status),
    Retrieve {
        item: FeedbackItem,
        handled: bool,
        image_path: Option<PathBuf>,
    },
    Handle {
        feedback_id: String,
    },
    Delete {
        feedback_id: String,
    },
    Cleanup {
        directory: String,
    },
}

// Every tagged variant is an object. MCP requires this explicit root type,
// while the generated oneOf still owns the variant-specific field contracts.
fn object_schema(schema: &mut schemars::Schema) {
    schema.insert("type".into(), "object".into());
}

#[derive(Debug, Clone)]
pub struct Queue {
    project: PathBuf,
    root: PathBuf,
}

impl Queue {
    pub fn open(project: &Path) -> Result<Self, Error> {
        let project = fs::canonicalize(project)?;
        if !project.join("project.godot").is_file() {
            return Err(Error::Invalid(
                "Select a Godot project containing project.godot".into(),
            ));
        }
        Ok(Self {
            root: project.join(".theatre/feedback"),
            project,
        })
    }

    pub fn status(&self) -> Result<Status, Error> {
        let mut status = Status {
            items: vec![],
            pending_count: 0,
            storage_bytes: 0,
            incomplete: vec![],
            max_items: MAX_ITEMS,
            max_storage_bytes: MAX_STORAGE_BYTES,
        };
        let entries = match fs::read_dir(&self.root) {
            Ok(entries) => entries,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(status),
            Err(e) => return Err(e.into()),
        };
        for entry in entries {
            let entry = entry?;
            let name = entry.file_name().to_string_lossy().into_owned();
            let bytes = storage_bytes(&entry.path())?;
            status.storage_bytes = status.storage_bytes.saturating_add(bytes);
            if !entry.file_type()?.is_dir() || name == "handled" {
                continue;
            }
            if name.starts_with(".pending-") {
                status.incomplete.push(IncompleteItem {
                    directory: name,
                    storage_bytes: bytes,
                });
                continue;
            }
            match self.item(&name) {
                Ok(item) => {
                    let handled = self.is_handled(&name);
                    status.pending_count += usize::from(!handled);
                    status.items.push(FeedbackSummary {
                        feedback_id: name,
                        source: item.source,
                        timestamp_ms: item.timestamp_ms,
                        scene: item.scene,
                        handled,
                        has_image: matches!(item.capture, Capture::Available { .. }),
                    });
                }
                // A corrupt publication is visible for deliberate cleanup, never a
                // pending human message and never silently removed by a reader.
                Err(_) => status.incomplete.push(IncompleteItem {
                    directory: name,
                    storage_bytes: bytes,
                }),
            }
        }
        status.items.sort_by(|a, b| {
            a.timestamp_ms
                .cmp(&b.timestamp_ms)
                .then(a.feedback_id.cmp(&b.feedback_id))
        });
        status
            .incomplete
            .sort_by(|a, b| a.directory.cmp(&b.directory));
        Ok(status)
    }

    pub fn item(&self, feedback_id: &str) -> Result<FeedbackItem, Error> {
        validate_id(feedback_id)?;
        let dir = self.root.join(feedback_id);
        let item: FeedbackItem =
            serde_json::from_slice(&read_bounded(&dir.join("item.json"), MAX_METADATA_BYTES)?)?;
        if item.feedback_id != feedback_id || item.note.len() > MAX_NOTE_BYTES {
            return Err(Error::Invalid(
                "Feedback identifier mismatch or oversized note".into(),
            ));
        }
        if fs::canonicalize(&item.project_path)? != self.project {
            return Err(Error::Invalid("Feedback belongs to another project".into()));
        }
        if let Capture::Available {
            ref source_dimensions,
            ref output_dimensions,
        } = item.capture
        {
            if source_dimensions.width == 0
                || source_dimensions.height == 0
                || output_dimensions.width == 0
                || output_dimensions.height == 0
            {
                return Err(Error::Invalid("Invalid image dimensions".into()));
            }
            // Notices inspect only framing and size, not every retained image's
            // megabytes. Explicit retrieval performs the bounded image read.
            let mut file = fs::File::open(dir.join("image.jpg"))?;
            let length = file.metadata()?.len();
            if !(4..=MAX_IMAGE_BYTES).contains(&length) {
                return Err(Error::Invalid("Invalid JPEG size".into()));
            }
            let mut start = [0; 2];
            let mut end = [0; 2];
            file.read_exact(&mut start)?;
            file.seek(SeekFrom::End(-2))?;
            file.read_exact(&mut end)?;
            if start != [0xff, 0xd8] || end != [0xff, 0xd9] {
                return Err(Error::Invalid("Incomplete JPEG image".into()));
            }
        }
        Ok(item)
    }

    pub fn execute(&self, operation: Operation) -> Result<Response, Error> {
        match operation {
            Operation::Status => Ok(Response::Status(self.status()?)),
            Operation::Retrieve { feedback_id } => {
                let item = self.item(&feedback_id)?;
                let image_path = matches!(item.capture, Capture::Available { .. })
                    .then(|| self.root.join(&feedback_id).join("image.jpg"));
                Ok(Response::Retrieve {
                    handled: self.is_handled(&feedback_id),
                    item,
                    image_path,
                })
            }
            Operation::Handle { feedback_id } => {
                self.item(&feedback_id)?;
                fs::create_dir_all(self.root.join("handled"))?;
                // Presence alone is the annotation; no mutable evidence or receipt registry.
                fs::write(self.root.join("handled").join(&feedback_id), [])?;
                Ok(Response::Handle { feedback_id })
            }
            Operation::Delete { feedback_id } => {
                self.item(&feedback_id)?;
                fs::remove_dir_all(self.root.join(&feedback_id))?;
                match fs::remove_file(self.root.join("handled").join(&feedback_id)) {
                    Ok(()) => (),
                    Err(e) if e.kind() == std::io::ErrorKind::NotFound => (),
                    Err(e) => return Err(e.into()),
                }
                Ok(Response::Delete { feedback_id })
            }
            Operation::Cleanup { directory } => {
                let status = self.status()?;
                if !status
                    .incomplete
                    .iter()
                    .any(|item| item.directory == directory)
                {
                    return Err(Error::Invalid("Cleanup requires an incomplete directory from feedback status; use delete for published evidence".into()));
                }
                fs::remove_dir_all(self.root.join(&directory))?;
                Ok(Response::Cleanup { directory })
            }
        }
    }

    fn is_handled(&self, feedback_id: &str) -> bool {
        self.root.join("handled").join(feedback_id).is_file()
    }
}

fn validate_id(value: &str) -> Result<(), Error> {
    // IDs are input to destructive filesystem operations. Limit them to the
    // producer's single directory component, preventing path traversal.
    if !value.starts_with("feedback_")
        || value.len() > 128
        || !value
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || c == b'_' || c == b'-')
    {
        return Err(Error::Invalid("Invalid feedback_id".into()));
    }
    Ok(())
}

fn storage_bytes(path: &Path) -> Result<u64, Error> {
    let metadata = fs::symlink_metadata(path)?;
    if !metadata.is_dir() {
        return Ok(metadata.len());
    }
    let mut bytes = 0u64;
    for entry in fs::read_dir(path)? {
        bytes = bytes.saturating_add(storage_bytes(&entry?.path())?);
    }
    Ok(bytes)
}

pub fn read_bounded(path: &Path, limit: u64) -> Result<Vec<u8>, Error> {
    let mut bytes = Vec::new();
    fs::File::open(path)?
        .take(limit + 1)
        .read_to_end(&mut bytes)?;
    if bytes.len() as u64 > limit {
        return Err(Error::Invalid(format!(
            "Feedback file exceeds {limit} bytes"
        )));
    }
    Ok(bytes)
}

/// Best effort only: an inaccessible queue must not change an unrelated tool's outcome.
pub fn pending_notice(project: &Path) -> Option<String> {
    let status = Queue::open(project).and_then(|q| q.status()).ok()?;
    (status.pending_count > 0).then(|| format!("{} pending human feedback item(s). Use feedback action=status, then retrieve the matching feedback_id for selection, pointer, image and note. Reads do not handle evidence; handle explicitly when addressed.", status.pending_count))
}

pub fn append_json_notice(value: &mut serde_json::Value, project: &Path) {
    if let Some(notice) = pending_notice(project)
        && let Some(object) = value.as_object_mut()
    {
        object.insert("feedback_notice".into(), notice.into());
    }
}
