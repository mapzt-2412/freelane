//! Folder state and explicit reopening of configured, previously unavailable roots.
use serde::{Deserialize, Serialize};
pub const VERSION: u16 = 13;
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum State {
    Disabled,
    Ready,
    Unavailable,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Report {
    pub inbox: State,
    pub outbox: State,
    pub busy: bool,
}
