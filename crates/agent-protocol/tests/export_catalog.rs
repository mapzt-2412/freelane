use freelane_agent_protocol::Request;
#[test]
fn outbox_catalog_has_its_own_read_only_protocol() {
    let wire = r#"{"version":12,"id":42,"action":{"type":"export_catalog"}}"#;
    let request: Request = serde_json::from_str(wire).expect("catalog action");
    request.validate().unwrap();
    for version in [1, 5, 11] {
        let mut request: Request = serde_json::from_str(wire).unwrap();
        request.version = version;
        assert!(request.validate().is_err());
    }
    let launch = r#"{"version":12,"id":42,"action":{"type":"launch","executable":"program","arguments":[]}}"#;
    let request: Request = serde_json::from_str(launch).unwrap();
    assert!(request.validate().is_err());
}
#[test]
fn catalog_rejects_duplicates_paths_and_oversized_frames() {
    use freelane_agent_protocol::export_catalog::{Entry, Page};
    let valid = Entry {
        name: "Result.txt".into(),
        size: 123,
    };
    assert!(Page {
        files: vec![valid.clone()],
        limited: false
    }
    .validate()
    .is_ok());
    assert!(Page {
        files: vec![valid.clone(), valid.clone()],
        limited: false
    }
    .validate()
    .is_err());
    assert!(Page {
        files: vec![Entry {
            name: "../secret".into(),
            size: 0
        }],
        limited: false
    }
    .validate()
    .is_err());
    assert!(Page {
        files: vec![valid; 129],
        limited: true
    }
    .validate()
    .is_err());
    assert!(Page {
        files: vec![Entry {
            name: "big.bin".into(),
            size: u64::MAX
        }],
        limited: false
    }
    .validate()
    .is_err());
}
