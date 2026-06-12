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

/// Result of deleting all sessions in a directory.
#[derive(Debug, serde::Serialize)]
pub struct DeleteAllResult {
    pub deleted_count: usize,
    pub new_session_id: String,
}

impl Session {
    /// Create a new session file in ~/.autoforge/sessions/<hash>/<timestamp>.jsonl
    pub fn new(workspace: &str) -> Self {
        let session_dir = sessions_dir_for_workspace(workspace);
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

/// Return the sessions directory for a given workspace path.
/// Pattern: ~/.autoforge/sessions/<hash>/
pub fn sessions_dir_for_workspace(workspace: &str) -> PathBuf {
    let home = std::env::var("HOME")
        .or_else(|_| std::env::var("USERPROFILE"))
        .unwrap_or_else(|_| ".".into());
    let dir = PathBuf::from(home).join(".autoforge").join("sessions");
    let hash = simple_hash(workspace);
    dir.join(hash)
}

/// Delete all session files in the given directory and create a new blank session.
/// Creates the directory if it doesn't exist.
/// Returns the number of deleted sessions and the new session ID.
pub fn delete_all_sessions(sessions_dir: &Path) -> std::io::Result<DeleteAllResult> {
    // Ensure the directory exists
    fs::create_dir_all(sessions_dir)?;

    // Count and delete existing session files
    let mut deleted_count: usize = 0;
    if sessions_dir.exists() {
        for entry in fs::read_dir(sessions_dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.extension().is_some_and(|ext| ext == "jsonl") {
                if fs::remove_file(&path).is_ok() {
                    deleted_count += 1;
                }
            }
        }
    }

    // Create a new blank session
    let new_session_id = format!("session_{}", uuid::Uuid::new_v4());
    let timestamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let new_path = sessions_dir.join(format!("{}.jsonl", timestamp));
    // Create empty file to represent the new session
    File::create(&new_path)?;

    Ok(DeleteAllResult {
        deleted_count,
        new_session_id,
    })
}

/// Simple hash function for workspace paths.
pub fn simple_hash(s: &str) -> String {
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

    // ─── delete_all_sessions tests ────────────────────────────────────────

    #[test]
    fn test_delete_all_sessions_deletes_all_files() {
        let temp_dir = tempfile::tempdir().unwrap();
        let sessions_dir = temp_dir.path().join("sessions");
        fs::create_dir_all(&sessions_dir).unwrap();

        // Create 3 session files
        for i in 1..=3 {
            let file_path = sessions_dir.join(format!("session_{i}.jsonl"));
            fs::write(&file_path, format!("{{\"data\": {i}}}")).unwrap();
        }

        let result = delete_all_sessions(&sessions_dir).unwrap();
        assert_eq!(result.deleted_count, 3);
        assert!(!result.new_session_id.is_empty());

        // Only the new session file should remain
        let entries: Vec<_> = fs::read_dir(&sessions_dir)
            .unwrap()
            .filter_map(|e| e.ok())
            .filter(|e| e.path().extension().is_some_and(|ext| ext == "jsonl"))
            .collect();
        assert_eq!(entries.len(), 1); // Only new session
    }

    #[test]
    fn test_delete_all_creates_directory_if_missing() {
        let temp_dir = tempfile::tempdir().unwrap();
        let sessions_dir = temp_dir.path().join("nonexistent").join("sessions");

        assert!(!sessions_dir.exists());
        let result = delete_all_sessions(&sessions_dir).unwrap();
        assert!(sessions_dir.exists());
        assert!(!result.new_session_id.is_empty());
        assert_eq!(result.deleted_count, 0);
    }

    #[test]
    fn test_delete_all_with_empty_directory() {
        let temp_dir = tempfile::tempdir().unwrap();
        let sessions_dir = temp_dir.path().join("sessions");
        fs::create_dir_all(&sessions_dir).unwrap();

        let result = delete_all_sessions(&sessions_dir).unwrap();
        assert_eq!(result.deleted_count, 0);
        assert!(!result.new_session_id.is_empty());

        // New session file created
        let entries: Vec<_> = fs::read_dir(&sessions_dir)
            .unwrap()
            .filter_map(|e| e.ok())
            .filter(|e| e.path().extension().is_some_and(|ext| ext == "jsonl"))
            .collect();
        assert_eq!(entries.len(), 1);
    }
}
