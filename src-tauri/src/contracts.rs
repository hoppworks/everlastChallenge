//! Stable renderer contracts. Raw native objects and paths never cross IPC.
use serde::{Deserialize, Serialize};
use ts_rs::TS;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
pub enum SetupPhase {
    Missing,
    Checking,
    Downloading,
    Loading,
    Cancelling,
    Cancelled,
    Ready,
    Failed,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
pub enum SessionPhase {
    Idle,
    Starting,
    Recording,
    Finalizing,
    Delivering,
    Cancelling,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub enum PermissionStatus {
    Granted,
    Denied,
    NotDetermined,
    Restricted,
    Unavailable,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum SessionOutcome {
    Local,
    Empty,
    Sent,
    Cancelled,
    Held { message: String },
    Failed { message: String },
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Model {
    pub name: String,
    pub download_bytes: u32,
    pub source_url: String,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Setup {
    pub phase: SetupPhase,
    pub model: Model,
    pub downloaded_bytes: u32,
    pub total_bytes: u32,
    pub can_cancel: bool,
    pub error: Option<String>,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Session {
    pub phase: SessionPhase,
    #[ts(type = "number | null")]
    pub id: Option<u64>,
    pub text: String,
    pub outcome: Option<SessionOutcome>,
    pub maximum_warning: bool,
    #[ts(type = "number")]
    pub elapsed_millis: u64,
    pub level_permille: u16,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(deny_unknown_fields)]
pub struct Permissions {
    pub microphone: PermissionStatus,
    pub accessibility: PermissionStatus,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(deny_unknown_fields)]
pub struct Labels {
    pub device: String,
    pub language: String,
    pub hotkey: String,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Preferences {
    pub current: crate::preferences::SettingsDraft,
    pub devices: Vec<crate::devices::InputDevice>,
    pub error: Option<String>,
    pub refreshing: bool,
    pub needs_restart: bool,
    pub recovery_required: bool,
    pub languages: Vec<crate::settings::SettingsLanguage>,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Snapshot {
    #[ts(type = "number")]
    pub revision: u64,
    pub running: bool,
    pub setup: Setup,
    pub session: Session,
    pub permissions: Permissions,
    pub labels: Labels,
    pub settings_busy: bool,
    pub settings_request: u32,
    pub preferences: Preferences,
    pub error: Option<String>,
}
