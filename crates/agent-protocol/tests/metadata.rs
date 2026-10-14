use freelane_agent_protocol::{
    Action, AppEntry, AppPage, Request, SharedMount, CATALOG_VERSION, MAX_FRAME,
    SHARE_STATUS_VERSION,
};
#[test]
fn metadata_versions_cannot_launch_or_accept_legacy_actions() {
    for version in [CATALOG_VERSION, SHARE_STATUS_VERSION] {
        assert!(Request {
            version,
            id: 1,
            action: Action::Launch {
                executable: "/bin/true".into(),
                arguments: vec![]
            }
        }
        .validate()
        .is_err());
        assert!(Request {
            version,
            id: 1,
            action: Action::Ping
        }
        .validate()
        .is_err());
    }
    for version in [1, 2, 3, 4, 5, 6, 7] {
        assert!(Request {
            version,
            id: 1,
            action: Action::AppCatalog { cursor: 0 }
        }
        .validate()
        .is_err());
    }
    assert!(Request {
        version: CATALOG_VERSION,
        id: 1,
        action: Action::AppCatalog { cursor: 513 }
    }
    .validate()
    .is_err());
    assert!(SharedMount {
        path: "relative".into(),
        readonly: true
    }
    .validate()
    .is_err());
}
#[test]
fn maximum_catalog_page_fits_bounded_wire_frame_with_escaped_paths() {
    let entry = AppEntry {
        name: "x".repeat(128),
        executable: format!("C:\\{}", "\\".repeat(2045)),
    };
    entry.validate().unwrap();
    let page = AppPage {
        apps: vec![entry; 12],
        next: Some(12),
    };
    let response = freelane_agent_protocol::Response {
        version: CATALOG_VERSION,
        id: 1,
        outcome: freelane_agent_protocol::Outcome::AppCatalog { page },
    };
    let mut wire = Vec::new();
    freelane_agent_protocol::write_frame(&mut wire, &response).unwrap();
    assert!(wire.len() < MAX_FRAME);
}
