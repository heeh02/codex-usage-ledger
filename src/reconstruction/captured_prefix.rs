//! A line-complete, immutable-by-verification view of an appendable source.
use super::*;
use std::io::{self, Read, Seek, SeekFrom};

pub(super) struct CapturedPrefix {
    file: fs::File,
    identity: String,
    pub len: u64,
    pub sha256: String,
    position: u64,
    read_digest: Sha256,
}

fn digest(file: &mut fs::File, len: u64) -> io::Result<String> {
    file.seek(SeekFrom::Start(0))?;
    let mut hash = Sha256::new();
    let mut remaining = len;
    let mut buffer = [0; 64 * 1024];
    while remaining > 0 {
        let wanted = remaining.min(buffer.len() as u64) as usize;
        let count = file.read(&mut buffer[..wanted])?;
        if count == 0 {
            return Err(io::Error::new(
                io::ErrorKind::UnexpectedEof,
                "captured source truncated",
            ));
        }
        hash.update(&buffer[..count]);
        remaining -= count as u64;
    }
    Ok(hex::encode(hash.finalize()))
}

impl CapturedPrefix {
    pub fn capture(path: &Path, expected_identity: &str, max_len: u64) -> Result<Self> {
        let mut file = fs::File::open(path)?;
        let metadata = file.metadata()?;
        let identity = physical_file_identity(path, &metadata)?;
        if identity != expected_identity {
            return Err(anyhow!("source replaced before capture"));
        }
        let end = metadata.len().min(max_len);
        // Exclude a trailing unfinished JSONL line, without scanning unbounded
        // bytes backward. A valid line cannot exceed the parser's line limit.
        let start = end.saturating_sub(DEFAULT_MAX_LINE_BYTES as u64 + 2);
        file.seek(SeekFrom::Start(start))?;
        let mut tail = vec![0; (end - start) as usize];
        file.read_exact(&mut tail)?;
        let len = match tail.iter().rposition(|byte| *byte == b'\n') {
            Some(offset) => start + offset as u64 + 1,
            None if start == 0 => 0,
            None => return Err(anyhow!("captured source has an oversized trailing line")),
        };
        let sha256 = digest(&mut file, len)?;
        file.seek(SeekFrom::Start(0))?;
        Ok(Self {
            file,
            identity,
            len,
            sha256,
            position: 0,
            read_digest: Sha256::new(),
        })
    }

    pub fn revalidate(&self, path: &Path) -> Result<bool> {
        if self.position != self.len
            || hex::encode(self.read_digest.clone().finalize()) != self.sha256
        {
            return Ok(false);
        }
        let mut current = fs::File::open(path)?;
        let metadata = current.metadata()?;
        if physical_file_identity(path, &metadata)? != self.identity || metadata.len() < self.len {
            return Ok(false);
        }
        let matches = digest(&mut current, self.len)? == self.sha256;
        let after = fs::metadata(path)?;
        Ok(matches
            && after.len() >= self.len
            && physical_file_identity(path, &after)? == self.identity)
    }
}

impl Read for CapturedPrefix {
    fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
        let wanted = (self.len - self.position).min(buffer.len() as u64) as usize;
        let count = self.file.read(&mut buffer[..wanted])?;
        if wanted > 0 && count == 0 {
            return Err(io::Error::new(
                io::ErrorKind::UnexpectedEof,
                "captured source truncated during read",
            ));
        }
        self.read_digest.update(&buffer[..count]);
        self.position += count as u64;
        Ok(count)
    }
}

impl Seek for CapturedPrefix {
    fn seek(&mut self, position: SeekFrom) -> io::Result<u64> {
        // Tailer resumes exactly where it stopped. Rewinding would invalidate
        // the byte-for-byte proof of the stream actually parsed.
        if position != SeekFrom::Start(self.position) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "captured reader requires sequential offsets",
            ));
        }
        self.file.seek(position)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    #[test]
    fn restored_file_cannot_hide_different_bytes_seen_by_parser() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("source.jsonl");
        fs::write(&path, b"one\n").unwrap();
        let identity = physical_file_identity(&path, &fs::metadata(&path).unwrap()).unwrap();
        let mut prefix = CapturedPrefix::capture(&path, &identity, 100).unwrap();
        fs::write(&path, b"two\n").unwrap();
        prefix.read_to_end(&mut Vec::new()).unwrap();
        fs::write(&path, b"one\n").unwrap();
        assert!(!prefix.revalidate(&path).unwrap());
        assert!(prefix.seek(SeekFrom::Start(0)).is_err());
    }

    #[test]
    fn append_is_excluded_but_prefix_edits_truncation_and_replacement_fail() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("source.jsonl");
        fs::write(&path, b"one\ntwo\npartial").unwrap();
        let identity = physical_file_identity(&path, &fs::metadata(&path).unwrap()).unwrap();
        let mut prefix = CapturedPrefix::capture(&path, &identity, 100).unwrap();
        assert_eq!(prefix.len, 8);
        fs::OpenOptions::new()
            .append(true)
            .open(&path)
            .unwrap()
            .write_all(b" completed\n")
            .unwrap();
        let mut bytes = Vec::new();
        prefix.read_to_end(&mut bytes).unwrap();
        assert_eq!(bytes, b"one\ntwo\n");
        assert!(prefix.revalidate(&path).unwrap());
        fs::OpenOptions::new()
            .write(true)
            .open(&path)
            .unwrap()
            .write_all(b"ONE")
            .unwrap();
        assert!(!prefix.revalidate(&path).unwrap());
        fs::write(&path, b"one\n").unwrap();
        assert!(!prefix.revalidate(&path).unwrap());
        fs::rename(&path, temp.path().join("old")).unwrap();
        fs::write(&path, b"one\ntwo\n").unwrap();
        assert!(!prefix.revalidate(&path).unwrap());
    }
}
