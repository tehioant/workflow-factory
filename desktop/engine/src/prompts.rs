//! Original prompt grammar. Inserted values are literal, never parsed again.
use crate::{validation, Definition, Error};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum Segment {
    Literal(String),
    Output(String),
}

pub fn ordered_ids(d: &Definition) -> Result<Vec<String>, Error> {
    crate::validate_graph(d)?;
    let mut order = Vec::new();
    let mut cursor = d
        .blocks
        .iter()
        .find(|b| !d.connections.iter().any(|c| c.target == b.id))
        .map(|b| b.id.clone());
    while let Some(id) = cursor {
        cursor = d
            .connections
            .iter()
            .find(|c| c.source == id)
            .map(|c| c.target.clone());
        order.push(id);
    }
    Ok(order)
}
pub fn compile(
    d: &Definition,
    block: &str,
    values: &BTreeMap<String, String>,
) -> Result<Vec<Segment>, Error> {
    let order = ordered_ids(d)?;
    let position = order
        .iter()
        .position(|id| id == block)
        .ok_or_else(|| validation("Missing prompt block."))?;
    let b = d
        .blocks
        .iter()
        .find(|b| b.id == block)
        .ok_or_else(|| validation("Missing prompt block."))?;
    let mut rest = b.prompt.as_str();
    let mut segments = Vec::new();
    while let Some(index) = rest.find("{{") {
        segments.push(Segment::Literal(rest[..index].into()));
        let tail = &rest[index + 2..];
        let end = tail
            .find("}}")
            .ok_or_else(|| validation("Unclosed prompt reference."))?;
        let token = &tail[..end];
        if let Some(name) = token.strip_prefix("input.") {
            segments.push(Segment::Literal(
                values
                    .get(name)
                    .cloned()
                    .ok_or_else(|| validation("Dangling input reference."))?,
            ));
        } else if let Some(id) = token
            .strip_prefix("output.")
            .and_then(|s| s.strip_suffix(".finalResponse"))
        {
            if !order[..position].iter().any(|ancestor| ancestor == id) {
                return Err(validation(
                    "Output reference must select an existing predecessor finalResponse.",
                ));
            }
            segments.push(Segment::Output(id.into()));
        } else {
            return Err(validation(
                "Use {{input.name}} or {{output.blockId.finalResponse}}.",
            ));
        }
        rest = &tail[end + 2..];
    }
    segments.push(Segment::Literal(rest.into()));
    Ok(segments)
}
pub fn resolve(segments: &[Segment], outputs: &BTreeMap<String, String>) -> Result<String, Error> {
    let mut result = String::new();
    for segment in segments {
        let text = match segment {
            Segment::Literal(text) => text,
            Segment::Output(id) => outputs.get(id).ok_or_else(|| {
                validation("Predecessor has no successful durable final response.")
            })?,
        };
        if result.len().saturating_add(text.len()) > 1_048_576 {
            return Err(validation("Prepared prompt exceeds 1 MiB."));
        }
        result.push_str(text);
    }
    Ok(result)
}
