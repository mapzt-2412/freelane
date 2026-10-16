use freelane_agent_protocol::Request;
#[test]
fn folder_check_and_reconnect_are_versioned_and_cannot_supply_paths() {
    for reconnect in [false, true] {
        let text = format!(
            r#"{{"version":13,"id":2,"action":{{"type":"transfer_folders","reconnect":{reconnect}}}}}"#
        );
        let request: Request = serde_json::from_str(&text).expect("folder action");
        request.validate().unwrap();
        for version in [1, 5, 11, 12] {
            let mut r: Request = serde_json::from_str(&text).unwrap();
            r.version = version;
            assert!(r.validate().is_err());
        }
    }
    assert!(serde_json::from_str::<Request>(r#"{"version":13,"id":2,"action":{"type":"transfer_folders","reconnect":true,"path":"/arbitrary"}}"#).is_err());
    let r: Request =
        serde_json::from_str(r#"{"version":13,"id":2,"action":{"type":"ping"}}"#).unwrap();
    assert!(r.validate().is_err());
}
