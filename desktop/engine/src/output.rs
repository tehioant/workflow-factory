//! Bounded JSONL projection; never retains a transcript or raw diagnostics.
use crate::{validation, Error};
#[derive(Default)]
pub struct Decoder {
    bytes: usize,
    frames: usize,
    final_text: Option<String>,
    completed: bool,
}
impl Decoder {
    pub fn decode(bytes: &[u8], exit: i32, secrets: &[String]) -> Result<String, Error> {
        if bytes.len() > 1_048_576 {
            return Err(validation("Structured output limit exceeded."));
        }
        let mut decoder = Self::default();
        for line in bytes.split(|b| *b == b'\n').filter(|s| !s.is_empty()) {
            decoder.frame(line)?;
        }
        decoder.finish(exit, secrets)
    }
    pub fn frame(&mut self, line: &[u8]) -> Result<(), Error> {
        self.bytes = self.bytes.saturating_add(line.len());
        self.frames += 1;
        if line.len() > 65536 || self.bytes > 1048576 || self.frames > 4096 || self.completed {
            return Err(validation(
                "Structured output limit or event ordering violation.",
            ));
        }
        let v: serde_json::Value =
            serde_json::from_slice(line).map_err(|_| validation("Malformed structured output."))?;
        match v["type"].as_str() {
            Some("item.completed") if v["item"]["type"] == "agent_message" => {
                let text = v["item"]["text"]
                    .as_str()
                    .ok_or_else(|| validation("Missing final text."))?;
                if text.len() > 65536 {
                    return Err(validation("Final response exceeds 64 KiB."));
                }
                self.final_text = Some(text.into());
            }
            Some("turn.completed") => self.completed = true,
            Some("turn.failed" | "error") => {
                return Err(validation(
                    "Codex reported failure. Raw diagnostics withheld.",
                ))
            }
            Some(
                "thread.started" | "turn.started" | "item.started" | "item.updated"
                | "item.completed",
            ) => {}
            _ => return Err(validation("Unsupported structured event.")),
        }
        Ok(())
    }
    pub fn finish(self, exit: i32, secrets: &[String]) -> Result<String, Error> {
        if exit != 0 || !self.completed {
            return Err(validation("Codex did not complete successfully."));
        }
        let mut text = self
            .final_text
            .ok_or_else(|| validation("No validated final response."))?;
        for secret in secrets.iter().filter(|s| !s.is_empty()) {
            text = text.replace(secret, "[REDACTED]");
        }
        Ok(text)
    }
}
