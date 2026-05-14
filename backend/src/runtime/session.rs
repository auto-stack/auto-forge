use std::fs::{self, File, OpenOptions};
use std::io::{BufRead, Write};
use std::path::{Path, PathBuf};

/// Session persistence using JSONL format.
/// Each message is stored as one JSON line.
pub struct Session {
    path: PathBuf,
}

#[derive(serde::Serialize, serde::Deserialize)]
struct SessionEntry {
    role: String,
    content: String,
}

impl Session {
    /// Create a new session file in ~/.autoforge/sessions/<hash>/<timestamp>.jsonl
    pub fn new(workspace: &str) -> Self {
        let home = std::env::var("HOME")
            .or_else(|_| std::env::var("USERPROFILE"))
            .unwrap_or_else(|_| ".".into());
        let dir = PathBuf::from(home).join(".autoforge").join("sessions");
        fs::create_dir_all(&dir).ok();

        let hash = simple_hash(workspace);
        let session_dir = dir.join(hash);
        fs::create_dir_all(&session_dir).ok();

        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        let path = session_dir.join(format!("{}.jsonl", timestamp));

        Self { path }
    }

    /// Create session at a specific path (for testing).
    pub fn at_path(path: PathBuf) -> Self {
        Self { path }
    }

    /// Append a message to the session file.
    pub fn append(&self, role: &str, content: &str) -> std::io::Result<()> {
        let entry = SessionEntry {
            role: role.into(),
            content: content.into(),
        };
        let line = serde_json::to_string(&entry)? + "\n";

        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.path)?;
        file.write_all(line.as_bytes())?;
        Ok(())
    }

    /// Load all messages from a session file.
    pub fn load(path: &Path) -> std::io::Result<Vec<(String, String)>> {
        let file = File::open(path)?;
        let reader = std::io::BufReader::new(file);
        let mut messages = Vec::new();

        for line in reader.lines() {
            let line = line?;
            if let Ok(entry) = serde_json::from_str::<SessionEntry>(&line) {
                messages.push((entry.role, entry.content));
            }
        }

        Ok(messages)
    }

    /// Get the session file path.
    pub fn path(&self) -> &Path {
        &self.path
    }
}

/// Simple hash function for workspace paths.
fn simple_hash(s: &str) -> String {
    let mut hash: u64 = 5381;
    for byte in s.bytes() {
        hash = hash.wrapping_mul(33).wrapping_add(byte as u64);
    }
    format!("{:016x}", hash)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_append_and_load() {
        let dir = std::env::temp_dir().join("af-runtime-session-test");
        let _ = fs::create_dir_all(&dir);
        let path = dir.join("test_session.jsonl");

        // Clean up any previous test run
        let _ = fs::remove_file(&path);

        let session = Session::at_path(path.clone());
        session.append("user", "hello").unwrap();
        session.append("assistant", "hi there").unwrap();
        session.append("user", "how are you?").unwrap();

        let messages = Session::load(&path).unwrap();
        assert_eq!(messages.len(), 3);
        assert_eq!(messages[0], ("user".into(), "hello".into()));
        assert_eq!(messages[1], ("assistant".into(), "hi there".into()));
        assert_eq!(messages[2], ("user".into(), "how are you?".into()));

        // Clean up
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_load_nonexistent() {
        let path = std::env::temp_dir().join("af-noexist-test").join("missing.jsonl");
        let result = Session::load(&path);
        assert!(result.is_err());
    }

    #[test]
    fn test_path() {
        let path = std::env::temp_dir().join("af-path-test").join("session.jsonl");
        let session = Session::at_path(path.clone());
        assert_eq!(session.path(), path.as_path());
    }
}
