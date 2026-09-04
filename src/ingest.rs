//! Incremental JSONL tailing primitives.
//!
//! A checkpoint records both the next unread byte and any unterminated line
//! that has already been read. Persisting both lets callers resume without
//! rescanning large rollout files or parsing a partial JSON value.

use serde::{Deserialize, Serialize};
use serde_json::Value;
#[cfg(not(unix))]
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::fs::{File, Metadata};
use std::io::{self, Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
#[cfg(not(unix))]
use std::time::UNIX_EPOCH;

pub const DEFAULT_READ_CHUNK_BYTES: usize = 4 * 1024 * 1024;
pub const DEFAULT_MAX_LINE_BYTES: usize = 64 * 1024 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct TailLimits {
    pub read_chunk_bytes: usize,
    pub max_line_bytes: usize,
}

impl Default for TailLimits {
    fn default() -> Self {
        Self {
            read_chunk_bytes: DEFAULT_READ_CHUNK_BYTES,
            max_line_bytes: DEFAULT_MAX_LINE_BYTES,
        }
    }
}

impl TailLimits {
    fn validate(self) -> io::Result<Self> {
        if self.read_chunk_bytes == 0 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "read_chunk_bytes must be greater than zero",
            ));
        }
        if self.max_line_bytes == 0 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "max_line_bytes must be greater than zero",
            ));
        }
        Ok(self)
    }
}

/// Durable state needed to resume tailing one physical file.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct TailCheckpoint {
    /// Identity of the physical file, not the Codex thread/session id.
    pub file_identity: Option<String>,
    /// Byte position at which the next read begins.
    pub next_offset: u64,
    /// Number of newline-terminated records already emitted.
    pub completed_lines: u64,
    /// Byte position at which `partial_line` begins.
    pub partial_offset: u64,
    /// Bytes read from disk which do not yet end in a newline.
    #[serde(default)]
    pub partial_line: Vec<u8>,
}

/// Why an existing tail checkpoint was discarded.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TailReset {
    FileIdentityChanged,
    FileTruncated,
}

/// One complete JSONL record with stable file-local provenance.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JsonlLine {
    pub byte_offset: u64,
    pub line_number: u64,
    pub raw: Vec<u8>,
}

impl JsonlLine {
    /// Parse the record as JSON. A UTF-8 BOM is tolerated on the first line.
    pub fn parse_json(&self) -> serde_json::Result<Value> {
        let raw = if self.line_number == 1 {
            self.raw.strip_prefix(b"\xef\xbb\xbf").unwrap_or(&self.raw)
        } else {
            &self.raw
        };
        serde_json::from_slice(raw)
    }

    pub fn is_blank(&self) -> bool {
        self.raw.iter().all(u8::is_ascii_whitespace)
    }
}

/// Newly completed lines and the checkpoint after reading them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TailBatch {
    pub lines: Vec<JsonlLine>,
    pub checkpoint: TailCheckpoint,
    pub reset: Option<TailReset>,
    /// Bytes fetched from the reader during this call, excluding saved partial.
    pub bytes_read: usize,
    /// The snapshotted file length still extends beyond this checkpoint.
    pub has_more: bool,
}

/// A per-file error does not prevent other rollouts from advancing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScanIssue {
    pub path: PathBuf,
    pub error: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CodexHomeScan {
    pub files_seen: usize,
    pub files_advanced: usize,
    pub lines_emitted: usize,
    pub bytes_read: u64,
    pub has_more: bool,
    pub issues: Vec<ScanIssue>,
}

#[derive(Debug, Clone, Default)]
pub struct IncrementalJsonlTailer {
    checkpoint: TailCheckpoint,
    limits: TailLimits,
}

/// Enumerate current and archived rollout files and advance each saved tail.
///
/// This helper deliberately owns no watcher or retry policy. Callers retain
/// `checkpoints` between invocations; consequently, existing files are read
/// only from their saved byte offsets. Missing source directories are normal.
pub fn scan_codex_home_once(
    codex_home: impl AsRef<Path>,
    checkpoints: &mut BTreeMap<PathBuf, TailCheckpoint>,
    consume: impl FnMut(&Path, &TailBatch) -> io::Result<()>,
) -> io::Result<CodexHomeScan> {
    scan_codex_home_once_with_limits(codex_home, checkpoints, TailLimits::default(), consume)
}

pub fn scan_codex_home_once_with_limits(
    codex_home: impl AsRef<Path>,
    checkpoints: &mut BTreeMap<PathBuf, TailCheckpoint>,
    limits: TailLimits,
    mut consume: impl FnMut(&Path, &TailBatch) -> io::Result<()>,
) -> io::Result<CodexHomeScan> {
    let limits = limits.validate()?;
    let codex_home = codex_home.as_ref();
    let mut paths = Vec::new();
    for source in ["sessions", "archived_sessions"] {
        collect_rollout_paths(&codex_home.join(source), &mut paths)?;
    }
    paths.sort();

    let mut scan = CodexHomeScan::default();
    for path in paths {
        scan.files_seen += 1;
        let checkpoint = checkpoints.get(&path).cloned().unwrap_or_default();
        let mut tailer = IncrementalJsonlTailer::with_limits(checkpoint, limits)?;
        match tailer.poll_path(&path) {
            Ok(batch) => {
                // The consumer must durably process the lines before the
                // caller-visible checkpoint advances.
                consume(&path, &batch)?;
                checkpoints.insert(path.clone(), batch.checkpoint.clone());
                scan.files_advanced += usize::from(batch.bytes_read > 0 || batch.reset.is_some());
                scan.lines_emitted += batch.lines.len();
                scan.bytes_read = scan.bytes_read.saturating_add(batch.bytes_read as u64);
                scan.has_more |= batch.has_more;
            }
            Err(error) => scan.issues.push(ScanIssue {
                path,
                error: error.to_string(),
            }),
        }
    }
    Ok(scan)
}

fn collect_rollout_paths(root: &Path, paths: &mut Vec<PathBuf>) -> io::Result<()> {
    if !root.exists() {
        return Ok(());
    }
    let mut directories = vec![root.to_path_buf()];
    while let Some(directory) = directories.pop() {
        let entries = match std::fs::read_dir(&directory) {
            Ok(entries) => entries,
            Err(error) if error.kind() == io::ErrorKind::NotFound => continue,
            Err(error) => return Err(error),
        };
        for entry in entries {
            let entry = entry?;
            let file_type = entry.file_type()?;
            if file_type.is_dir() {
                directories.push(entry.path());
                continue;
            }
            if !file_type.is_file() {
                continue;
            }
            let name = entry.file_name();
            let name = name.to_string_lossy();
            if name.starts_with("rollout-") && name.ends_with(".jsonl") {
                paths.push(entry.path());
            }
        }
    }
    Ok(())
}

impl IncrementalJsonlTailer {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn from_checkpoint(checkpoint: TailCheckpoint) -> Self {
        Self {
            checkpoint,
            limits: TailLimits::default(),
        }
    }

    pub fn with_limits(checkpoint: TailCheckpoint, limits: TailLimits) -> io::Result<Self> {
        Ok(Self {
            checkpoint,
            limits: limits.validate()?,
        })
    }

    pub fn checkpoint(&self) -> &TailCheckpoint {
        &self.checkpoint
    }

    pub fn into_checkpoint(self) -> TailCheckpoint {
        self.checkpoint
    }

    pub fn limits(&self) -> TailLimits {
        self.limits
    }

    /// Read only bytes appended since the previous call.
    pub fn poll_path(&mut self, path: impl AsRef<Path>) -> io::Result<TailBatch> {
        let path = path.as_ref();
        let mut file = File::open(path)?;
        let metadata = file.metadata()?;
        let identity = physical_file_identity(path, &metadata)?;
        self.poll_reader(&mut file, metadata.len(), identity)
    }

    /// Reader-oriented form used by tests and callers that already own a file.
    ///
    /// `current_len` and `file_identity` must describe the reader at the time
    /// of the call. The reader is always repositioned to the checkpoint.
    pub fn poll_reader<R: Read + Seek>(
        &mut self,
        reader: &mut R,
        current_len: u64,
        file_identity: impl Into<String>,
    ) -> io::Result<TailBatch> {
        let file_identity = file_identity.into();
        let mut reset = None;
        let mut checkpoint = self.checkpoint.clone();

        if checkpoint
            .file_identity
            .as_deref()
            .is_some_and(|known| known != file_identity)
        {
            checkpoint = TailCheckpoint::default();
            reset = Some(TailReset::FileIdentityChanged);
        } else if current_len < checkpoint.next_offset {
            checkpoint = TailCheckpoint::default();
            reset = Some(TailReset::FileTruncated);
        }
        checkpoint.file_identity = Some(file_identity);

        let read_start = checkpoint.next_offset;
        reader.seek(SeekFrom::Start(read_start))?;
        let remaining = current_len.saturating_sub(read_start);
        let read_limit = remaining.min(self.limits.read_chunk_bytes as u64);
        let mut appended = Vec::with_capacity(read_limit as usize);
        reader.take(read_limit).read_to_end(&mut appended)?;
        checkpoint.next_offset = read_start.saturating_add(appended.len() as u64);

        let had_partial = !checkpoint.partial_line.is_empty();
        let base_offset = if had_partial {
            checkpoint.partial_offset
        } else {
            read_start
        };
        let mut combined = std::mem::take(&mut checkpoint.partial_line);
        combined.extend_from_slice(&appended);

        let mut lines = Vec::new();
        let mut line_start = 0usize;
        for (index, byte) in combined.iter().copied().enumerate() {
            if byte != b'\n' {
                continue;
            }
            if index - line_start > self.limits.max_line_bytes {
                return Err(line_too_long(self.limits.max_line_bytes));
            }
            let mut raw = combined[line_start..index].to_vec();
            if raw.last() == Some(&b'\r') {
                raw.pop();
            }
            checkpoint.completed_lines += 1;
            lines.push(JsonlLine {
                byte_offset: base_offset + line_start as u64,
                line_number: checkpoint.completed_lines,
                raw,
            });
            line_start = index + 1;
        }

        if combined.len() - line_start > self.limits.max_line_bytes {
            return Err(line_too_long(self.limits.max_line_bytes));
        }
        checkpoint.partial_line = combined[line_start..].to_vec();
        checkpoint.partial_offset = if checkpoint.partial_line.is_empty() {
            checkpoint.next_offset
        } else {
            base_offset + line_start as u64
        };
        let has_more = checkpoint.next_offset < current_len;
        self.checkpoint = checkpoint.clone();

        Ok(TailBatch {
            lines,
            checkpoint,
            reset,
            bytes_read: appended.len(),
            has_more,
        })
    }
}

fn line_too_long(limit: usize) -> io::Error {
    io::Error::new(
        io::ErrorKind::InvalidData,
        format!("JSONL line exceeds configured {limit}-byte limit"),
    )
}

/// Best available cross-platform identity for a physical file.
///
/// Unix uses `(device, inode)`. Other platforms hash the canonical path and
/// creation time, which remains stable across appends and changes on normal
/// replacement/rotation.
pub fn physical_file_identity(_path: &Path, metadata: &Metadata) -> io::Result<String> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        Ok(format!("unix:{}:{}", metadata.dev(), metadata.ino()))
    }

    #[cfg(not(unix))]
    {
        let canonical = _path
            .canonicalize()
            .unwrap_or_else(|_| PathBuf::from(_path));
        let created_nanos = metadata
            .created()
            .ok()
            .and_then(|created| created.duration_since(UNIX_EPOCH).ok())
            .map(|duration| duration.as_nanos())
            .unwrap_or_default();
        let mut digest = Sha256::new();
        digest.update(canonical.to_string_lossy().as_bytes());
        digest.update([0]);
        digest.update(created_nanos.to_le_bytes());
        Ok(format!("portable:{}", hex::encode(digest.finalize())))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    #[test]
    fn emits_only_complete_lines_and_resumes_partial_line() {
        let first = br#"{"a":1}
{"b":"#
            .to_vec();
        let complete = br#"{"a":1}
{"b":2}
"#
        .to_vec();
        let mut tailer = IncrementalJsonlTailer::new();

        let mut reader = Cursor::new(first.clone());
        let batch = tailer
            .poll_reader(&mut reader, first.len() as u64, "file-a")
            .unwrap();
        assert_eq!(batch.lines.len(), 1);
        assert_eq!(batch.lines[0].byte_offset, 0);
        assert_eq!(batch.lines[0].line_number, 1);
        assert_eq!(batch.lines[0].parse_json().unwrap()["a"], 1);
        assert_eq!(batch.checkpoint.partial_line, br#"{"b":"#);

        let encoded = serde_json::to_string(&batch.checkpoint).unwrap();
        let restored: TailCheckpoint = serde_json::from_str(&encoded).unwrap();
        let mut tailer = IncrementalJsonlTailer::from_checkpoint(restored);

        let mut reader = Cursor::new(complete.clone());
        let batch = tailer
            .poll_reader(&mut reader, complete.len() as u64, "file-a")
            .unwrap();
        assert_eq!(batch.lines.len(), 1);
        assert_eq!(batch.lines[0].line_number, 2);
        assert_eq!(batch.lines[0].parse_json().unwrap()["b"], 2);
        assert!(batch.checkpoint.partial_line.is_empty());
        assert_eq!(batch.checkpoint.next_offset, complete.len() as u64);
    }

    #[test]
    fn strips_crlf_but_preserves_offsets_and_blank_line_numbers() {
        let content = b"\xef\xbb\xbf{\"a\":1}\r\n\r\n{\"c\":3}\r\n".to_vec();
        let mut reader = Cursor::new(content.clone());
        let mut tailer = IncrementalJsonlTailer::new();
        let batch = tailer
            .poll_reader(&mut reader, content.len() as u64, "file-a")
            .unwrap();

        assert_eq!(batch.lines.len(), 3);
        assert_eq!(batch.lines[0].parse_json().unwrap()["a"], 1);
        assert!(batch.lines[1].is_blank());
        assert_eq!(batch.lines[2].line_number, 3);
        assert_eq!(batch.lines[2].parse_json().unwrap()["c"], 3);
        assert_eq!(batch.lines[1].byte_offset, 12);
        assert_eq!(batch.lines[2].byte_offset, 14);
    }

    #[test]
    fn resets_when_file_is_truncated_or_replaced() {
        let original = b"one\ntwo\n".to_vec();
        let mut tailer = IncrementalJsonlTailer::new();
        let mut reader = Cursor::new(original.clone());
        tailer
            .poll_reader(&mut reader, original.len() as u64, "file-a")
            .unwrap();

        let truncated = b"new\n".to_vec();
        let mut reader = Cursor::new(truncated.clone());
        let batch = tailer
            .poll_reader(&mut reader, truncated.len() as u64, "file-a")
            .unwrap();
        assert_eq!(batch.reset, Some(TailReset::FileTruncated));
        assert_eq!(batch.lines[0].line_number, 1);
        assert_eq!(batch.lines[0].raw, b"new");

        let replacement = b"replacement\n".to_vec();
        let mut reader = Cursor::new(replacement.clone());
        let batch = tailer
            .poll_reader(&mut reader, replacement.len() as u64, "file-b")
            .unwrap();
        assert_eq!(batch.reset, Some(TailReset::FileIdentityChanged));
        assert_eq!(batch.lines[0].byte_offset, 0);
        assert_eq!(batch.lines[0].line_number, 1);
    }

    #[test]
    fn codex_home_scan_advances_only_appended_bytes() {
        let temp = tempfile::tempdir().unwrap();
        let current = temp.path().join("sessions/2026/08/31");
        let archived = temp.path().join("archived_sessions");
        std::fs::create_dir_all(&current).unwrap();
        std::fs::create_dir_all(&archived).unwrap();
        let current_file = current.join("rollout-current.jsonl");
        let archived_file = archived.join("rollout-archived.jsonl");
        std::fs::write(&current_file, b"{\"n\":1}\n{\"n\":").unwrap();
        std::fs::write(&archived_file, b"{\"n\":9}\n").unwrap();
        std::fs::write(current.join("ignore.txt"), b"ignored").unwrap();

        let mut checkpoints = BTreeMap::new();
        let mut first_values = Vec::new();
        let first = scan_codex_home_once(temp.path(), &mut checkpoints, |_, batch| {
            first_values.extend(
                batch
                    .lines
                    .iter()
                    .filter(|line| !line.is_blank())
                    .map(|line| line.parse_json().unwrap()),
            );
            Ok(())
        })
        .unwrap();
        assert_eq!(first.files_seen, 2);
        assert_eq!(first.lines_emitted, 2);
        assert_eq!(first_values.len(), 2);

        std::fs::write(&current_file, b"{\"n\":1}\n{\"n\":2}\n").unwrap();
        let mut observed = Vec::new();
        let second = scan_codex_home_once(temp.path(), &mut checkpoints, |path, batch| {
            observed.push((path.to_path_buf(), batch.lines.clone()));
            Ok(())
        })
        .unwrap();
        assert_eq!(second.lines_emitted, 1);
        let current_lines = &observed
            .iter()
            .find(|(path, _)| path == &current_file)
            .unwrap()
            .1;
        assert_eq!(current_lines.len(), 1);
        assert_eq!(current_lines[0].parse_json().unwrap()["n"], 2);
        let archived_lines = &observed
            .iter()
            .find(|(path, _)| path == &archived_file)
            .unwrap()
            .1;
        assert!(archived_lines.is_empty());
    }

    #[test]
    fn bounded_batches_can_reconstruct_a_large_reader() {
        let content = (0..40)
            .map(|value| format!("{{\"n\":{value}}}\n"))
            .collect::<String>()
            .into_bytes();
        let limits = TailLimits {
            read_chunk_bytes: 31,
            max_line_bytes: 64,
        };
        let mut tailer =
            IncrementalJsonlTailer::with_limits(TailCheckpoint::default(), limits).unwrap();
        let mut values = Vec::new();

        loop {
            let mut reader = Cursor::new(content.clone());
            let batch = tailer
                .poll_reader(&mut reader, content.len() as u64, "large-file")
                .unwrap();
            assert!(batch.bytes_read <= limits.read_chunk_bytes);
            values.extend(
                batch
                    .lines
                    .iter()
                    .map(|line| line.parse_json().unwrap()["n"].as_u64().unwrap()),
            );
            if !batch.has_more {
                break;
            }
        }
        assert_eq!(values, (0..40).collect::<Vec<_>>());
        assert!(tailer.checkpoint().partial_line.is_empty());
        assert_eq!(tailer.checkpoint().next_offset, content.len() as u64);
    }

    #[test]
    fn oversized_partial_line_errors_without_advancing_checkpoint() {
        let content = b"01234567890\n".to_vec();
        let limits = TailLimits {
            read_chunk_bytes: 8,
            max_line_bytes: 10,
        };
        let mut tailer =
            IncrementalJsonlTailer::with_limits(TailCheckpoint::default(), limits).unwrap();
        let mut reader = Cursor::new(content.clone());
        let first = tailer
            .poll_reader(&mut reader, content.len() as u64, "long-line")
            .unwrap();
        assert_eq!(first.bytes_read, 8);
        assert_eq!(tailer.checkpoint().next_offset, 8);

        let mut reader = Cursor::new(content);
        let error = tailer
            .poll_reader(&mut reader, 12, "long-line")
            .unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::InvalidData);
        assert_eq!(tailer.checkpoint().next_offset, 8);
        assert_eq!(tailer.checkpoint().partial_line, b"01234567");
    }
}
