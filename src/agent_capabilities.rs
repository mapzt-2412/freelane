//! Read-only capability negotiation; absent legacy features are unknown, not unsupported.
use crate::{Error, Result};
use freelane_agent_protocol::{Action, Feature, Response};
use serde::Serialize;
use std::path::Path;
#[derive(Debug, Serialize)]
pub struct Report {
    pub negotiated_protocol: u16,
    pub minimum_agent_protocol: u16,
    pub agent_version: Option<String>,
    pub os: Option<String>,
    pub arch: Option<String>,
    pub protocols: Option<Vec<u16>>,
    pub features: Vec<Feature>,
    pub capability_list_complete: bool,
    pub max_frame: usize,
}
pub fn inspect(config: &Path) -> Result<Report> {
    negotiate(|version| crate::agent::request_vm(config, Action::Ping, version))
}
fn negotiate(mut ping: impl FnMut(u16) -> Result<Response>) -> Result<Report> {
    use freelane_agent_protocol::{
        Outcome, CAPABILITY_VERSION, MAX_FRAME, VERSION, WINDOW_VERSION,
    };
    for version in [CAPABILITY_VERSION, WINDOW_VERSION, VERSION] {
        let Ok(response) = ping(version) else {
            continue;
        };
        if response.version != version {
            continue;
        }
        let base = Report {
            negotiated_protocol: version,
            minimum_agent_protocol: VERSION,
            agent_version: None,
            os: None,
            arch: None,
            protocols: None,
            features: Vec::new(),
            capability_list_complete: false,
            max_frame: MAX_FRAME,
        };
        match response.outcome {
            Outcome::AgentInfo { info }
                if version == CAPABILITY_VERSION && info.validate().is_ok() =>
            {
                return Ok(Report {
                    agent_version: Some(info.agent_version),
                    os: Some(info.os),
                    arch: Some(info.arch),
                    protocols: Some(info.protocols),
                    features: info.features,
                    capability_list_complete: true,
                    ..base
                });
            }
            Outcome::Capabilities { window_snapshots } if version == WINDOW_VERSION => {
                return Ok(Report {
                    features: if window_snapshots {
                        vec![Feature {
                            name: "window_snapshots".into(),
                            protocol: WINDOW_VERSION,
                        }]
                    } else {
                        vec![]
                    },
                    ..base
                });
            }
            Outcome::Pong { os, arch }
                if version == VERSION
                    && [&os, &arch].into_iter().all(|v| {
                        !v.is_empty()
                            && v.len() <= 64
                            && v.is_ascii()
                            && !v.chars().any(char::is_control)
                    }) =>
            {
                return Ok(Report {
                    os: Some(os),
                    arch: Some(arch),
                    ..base
                });
            }
            _ => {}
        }
    }
    Err(Error("Compatible guest agent unavailable; minimum protocol v1 required. Enable the channel and run the agent in the guest user session.".into()))
}
#[cfg(test)]
mod tests {
    use super::*;
    use freelane_agent_protocol::{AgentInfo, Outcome};
    #[test]
    fn modern_capabilities_preserve_feature_protocols_and_version_metadata() {
        let report = negotiate(|version| {
            assert_eq!(version, 3);
            Ok(Response {
                version,
                id: 1,
                outcome: Outcome::AgentInfo {
                    info: AgentInfo {
                        agent_version: "0.1.0".into(),
                        os: "linux".into(),
                        arch: "aarch64".into(),
                        protocols: vec![1, 2, 3, 4],
                        features: vec![Feature {
                            name: "launch".into(),
                            protocol: 1,
                        }],
                        max_frame: 65536,
                    },
                },
            })
        })
        .unwrap();
        assert!(report.capability_list_complete);
        assert_eq!(report.negotiated_protocol, 3);
        assert_eq!(report.arch.as_deref(), Some("aarch64"));
        assert_eq!(report.features[0].protocol, 1);
        assert_eq!(
            serde_json::to_value(&report).unwrap()["protocols"],
            serde_json::json!([1, 2, 3, 4])
        );
    }
    #[test]
    fn legacy_negotiation_is_bounded_and_missing_features_stay_unknown() {
        let mut attempts = Vec::new();
        let report = negotiate(|version| {
            attempts.push(version);
            if version == 2 {
                Ok(Response {
                    version,
                    id: 1,
                    outcome: Outcome::Capabilities {
                        window_snapshots: true,
                    },
                })
            } else {
                Err(Error("unsupported".into()))
            }
        })
        .unwrap();
        assert_eq!(attempts, vec![3, 2]);
        assert!(!report.capability_list_complete);
        assert!(report.agent_version.is_none());
        assert_eq!(report.features[0].name, "window_snapshots");
        let mut attempts = Vec::new();
        assert!(negotiate(|version| {
            attempts.push(version);
            Err(Error("disconnected".into()))
        })
        .is_err());
        assert_eq!(attempts, vec![3, 2, 1]);
    }
}

impl Report {
    pub fn supports(&self, name: &str, protocol: u16) -> Option<bool> {
        if self
            .features
            .iter()
            .any(|feature| feature.name == name && feature.protocol == protocol)
        {
            Some(true)
        } else if self.capability_list_complete
            || (self.negotiated_protocol == freelane_agent_protocol::WINDOW_VERSION
                && name == "window_snapshots"
                && protocol == freelane_agent_protocol::WINDOW_VERSION)
        {
            Some(false)
        } else {
            None
        }
    }
}
#[cfg(test)]
mod support_tests {
    use super::*;
    use freelane_agent_protocol::Outcome;
    #[test]
    fn legacy_explicit_window_denial_is_false_and_unreported_features_are_unknown() {
        let report = negotiate(|version| {
            if version == 2 {
                Ok(Response {
                    version,
                    id: 1,
                    outcome: Outcome::Capabilities {
                        window_snapshots: false,
                    },
                })
            } else {
                Err(Error("unsupported".into()))
            }
        })
        .unwrap();
        assert_eq!(report.supports("window_snapshots", 2), Some(false));
        assert_eq!(report.supports("clipboard_text", 4), None);
    }
}
