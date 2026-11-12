//! Bounded JSONL projection; never retains a transcript or raw diagnostics.
use crate::{validation, Error};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DiagnosticIdentity {
    pub run_id: String,
    pub block_id: String,
    pub epoch: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DiagnosticRecord {
    pub kind: String,
    pub text: String,
    pub command: Option<String>,
    pub status: Option<String>,
    pub exit_code: Option<i32>,
}
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Publication {
    pub version: u32,
    pub identity: DiagnosticIdentity,
    pub records: Vec<DiagnosticRecord>,
    #[serde(rename = "final")]
    pub final_response: Option<String>,
    pub incomplete: bool,
    pub completed: bool,
    pub exit: i32,
}
impl Publication {
    pub fn acknowledged(identity: DiagnosticIdentity) -> Self {
        Self {
            version: 1,
            identity,
            records: vec![],
            final_response: None,
            incomplete: false,
            completed: false,
            exit: 0,
        }
    }
    pub fn decode(
        bytes: &[u8],
        identity: &DiagnosticIdentity,
        secrets: &[String],
    ) -> Result<Self, Error> {
        if bytes.len() > 1_048_576 {
            return Err(validation("Diagnostic publication limit exceeded."));
        }
        let mut result: Self =
            serde_json::from_slice(bytes).map_err(|_| validation("Invalid diagnostic schema."))?;
        if result.version != 1
            || result.identity != *identity
            || result.records.len() > 64
            || result
                .final_response
                .as_ref()
                .is_some_and(|s| s.len() > 65536)
            || ((result.incomplete || !result.completed) && result.final_response.is_some())
        {
            return Err(validation("Diagnostic scope or limit mismatch."));
        }
        for record in &mut result.records {
            if !["command", "stderr", "error"].contains(&record.kind.as_str())
                || record.text.len() > 65536
                || record.command.as_ref().is_some_and(|s| s.len() > 65536)
                || (record.kind == "command"
                    && (record.command.is_none()
                        || !record
                            .status
                            .as_deref()
                            .is_some_and(|s| ["in_progress", "completed", "failed"].contains(&s))))
            {
                return Err(validation("Invalid diagnostic record."));
            }
            for secret in secrets.iter().filter(|s| !s.is_empty()) {
                record.text = record.text.replace(secret, "[REDACTED]");
                if let Some(command) = &mut record.command {
                    *command = command.replace(secret, "[REDACTED]");
                }
            }
        }
        if let Some(text) = &mut result.final_response {
            for secret in secrets.iter().filter(|s| !s.is_empty()) {
                *text = text.replace(secret, "[REDACTED]");
            }
            if text.len() > 65536 {
                return Err(validation("Redacted final limit exceeded."));
            }
        }
        Ok(result)
    }
}
#[derive(Default)]
pub struct LiveDecoder {
    line: Vec<u8>,
    seq: u64,
    bytes: u64,
    received: usize,
    stopped: bool,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct LiveFrame {
    version: u32,
    r#type: String,
    seq: u64,
    kind: String,
    bytes: u64,
    text: String,
    incomplete: bool,
}
impl LiveDecoder {
    pub fn push(&mut self, bytes: &[u8]) -> Result<(), Error> {
        self.received = self.received.saturating_add(bytes.len());
        if self.received > 1_048_576 {
            return Err(validation("Live diagnostic transport limit exceeded."));
        }
        for byte in bytes {
            if *byte == b'\n' {
                let frame: LiveFrame = serde_json::from_slice(&self.line)
                    .map_err(|_| validation("Invalid live diagnostic envelope."))?;
                self.line.clear();
                if self.stopped
                    || frame.version != 1
                    || frame.r#type != "wf.diagnostic"
                    || frame.seq != self.seq + 1
                    || frame.seq > 4097
                    || frame.bytes < self.bytes
                    || frame.bytes > 1_048_577
                    || frame.text != "WITHHELD"
                    || !["command", "stderr", "error", "limit", "stopped"]
                        .contains(&frame.kind.as_str())
                    || frame.kind == "limit" && !frame.incomplete
                {
                    return Err(validation("Invalid live diagnostic ordering or metadata."));
                }
                self.seq = frame.seq;
                self.bytes = frame.bytes;
                self.stopped = frame.kind == "stopped";
            } else {
                if self.line.len() >= 1024 {
                    return Err(validation("Live diagnostic frame limit exceeded."));
                }
                self.line.push(*byte);
            }
        }
        Ok(())
    }
    pub fn finish(&self) -> Result<(), Error> {
        if !self.stopped || !self.line.is_empty() {
            Err(validation("Incomplete live diagnostic stream."))
        } else {
            Ok(())
        }
    }
}
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
