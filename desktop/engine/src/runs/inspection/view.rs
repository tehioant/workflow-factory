use super::*;
use ts_rs::TS;
#[derive(Clone, Debug, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct Notice {
    pub run: String,
    pub subscription: String,
    pub generation: String,
    pub floor: String,
    pub high: String,
}
#[derive(Debug, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct InspectionPage {
    pub run: String,
    pub subscription: String,
    pub cursor: String,
    pub generation: String,
    pub floor: String,
    pub high: String,
    pub resync: bool,
    pub incomplete: bool,
    pub available: bool,
    pub more: bool,
    pub entries: Vec<LogEntry>,
}
#[derive(Debug, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct LogEntry {
    pub sequence: String,
    pub block: String,
    pub kind: String,
    pub text: String,
}
#[derive(Debug, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct ArtifactPage {
    pub run: String,
    pub generation: String,
    pub available: bool,
    pub complete: bool,
    pub cursor: Option<String>,
    pub entries: Vec<ArtifactItem>,
}
#[derive(Debug, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct ArtifactItem {
    pub id: String,
    pub name: String,
}
#[derive(Debug, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct ArtifactText {
    pub run: String,
    pub generation: String,
    pub id: String,
    pub text: String,
    pub cursor: Option<String>,
}
/// No initialization, migration, permissions changes, execution, or recovery.
pub fn scoped(root: &Path, workflow: &str, run: &str) -> Result<(), Error> {
    crate::runs::history::HistoryReader::open(root)?
        .run_detail(workflow, run)
        .map(|_| ())
}
