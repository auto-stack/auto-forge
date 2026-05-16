//! Incremental SSE parser ported from auto-code-rs.
use crate::provider::types::{ApiError, StreamEvent};

pub struct SseParser { buffer: Vec<u8> }

impl SseParser {
    pub fn new() -> Self { Self { buffer: Vec::new() } }

    pub fn push(&mut self, chunk: &[u8]) -> Result<Vec<StreamEvent>, ApiError> {
        self.buffer.extend_from_slice(chunk);
        self.drain_frames()
    }

    pub fn finish(mut self) -> Result<Vec<StreamEvent>, ApiError> {
        // Append trailing newline to flush any final frame
        self.buffer.extend_from_slice(b"\n\n");
        self.drain_frames()
    }

    fn drain_frames(&mut self) -> Result<Vec<StreamEvent>, ApiError> {
        let mut events = Vec::new();
        while let Some(frame) = self.next_frame() {
            if let Some(event) = self.parse_frame(&frame)? {
                events.push(event);
            }
        }
        Ok(events)
    }

    fn next_frame(&mut self) -> Option<String> {
        // Find frame boundary in raw bytes, then decode only the complete frame
        if let Some(pos) = Self::find_boundary(&self.buffer, b"\r\n\r\n") {
            let frame_bytes = self.buffer.drain(..pos).collect::<Vec<u8>>();
            // drain also removes the boundary
            self.buffer.drain(..4);
            return String::from_utf8(frame_bytes).ok();
        }
        if let Some(pos) = Self::find_boundary(&self.buffer, b"\n\n") {
            let frame_bytes = self.buffer.drain(..pos).collect::<Vec<u8>>();
            self.buffer.drain(..2);
            return String::from_utf8(frame_bytes).ok();
        }
        None
    }

    fn find_boundary(buf: &[u8], boundary: &[u8]) -> Option<usize> {
        buf.windows(boundary.len()).position(|w| w == boundary)
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
