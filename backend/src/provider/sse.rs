//! Incremental SSE parser ported from auto-code-rs.
use crate::provider::types::{ApiError, StreamEvent};
pub struct SseParser { buffer: String }
impl SseParser {
    pub fn new() -> Self { Self { buffer: String::new() } }
    pub fn push(&mut self, chunk: &[u8]) -> Result<Vec<StreamEvent>, ApiError> {
        let text = std::str::from_utf8(chunk).map_err(|e| ApiError::Sse(format!("invalid utf-8: {e}")))?;
        self.buffer.push_str(text);
        self.drain_frames()
    }
    pub fn finish(mut self) -> Result<Vec<StreamEvent>, ApiError> {
        let remaining = self.buffer.trim().to_owned();
        if remaining.is_empty() { return Ok(vec![]); }
        self.buffer = remaining + "\n\n";
        self.drain_frames()
    }
    fn drain_frames(&mut self) -> Result<Vec<StreamEvent>, ApiError> {
        let mut events = Vec::new();
        while let Some(frame) = self.next_frame() {
            if let Some(event) = self.parse_frame(&frame)? { events.push(event); }
        }
        Ok(events)
    }
    fn next_frame(&mut self) -> Option<String> {
        if let Some(pos) = self.buffer.find("\r\n\r\n") {
            let frame = self.buffer[..pos].to_owned();
            self.buffer = self.buffer[pos + 4..].to_owned();
            return Some(frame);
        }
        if let Some(pos) = self.buffer.find("\n\n") {
            let frame = self.buffer[..pos].to_owned();
            self.buffer = self.buffer[pos + 2..].to_owned();
            return Some(frame);
        }
        None
    }
    fn parse_frame(&self, frame: &str) -> Result<Option<StreamEvent>, ApiError> {
        let mut event_type: Option<String> = None;
        let mut data_parts: Vec<String> = Vec::new();
        for line in frame.lines() {
            if line.is_empty() || line.starts_with(':') { continue; }
            if let Some((key, value)) = line.split_once(':') {
                match key.trim() {
                    "event" => event_type = Some(value.trim_start_matches(' ').to_owned()),
                    "data" => data_parts.push(value.trim_start_matches(' ').to_owned()),
                    _ => {}
                }
            }
        }
        let event_name = match event_type { Some(n) => n, None => return Ok(None) };
        if event_name == "ping" { return Ok(Some(StreamEvent::Ping)); }
        let data = data_parts.join("\n");
        if data.is_empty() { return Ok(None); }
        let event: StreamEvent = serde_json::from_str(&data).map_err(|e| {
            ApiError::Sse(format!("parse SSE '{}' failed: {e}\n  data: {data}", event_name))
        })?;
        Ok(Some(event))
    }
}
impl Default for SseParser { fn default() -> Self { Self::new() } }
