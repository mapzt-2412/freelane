use freelane_agent_protocol::Request;
use serde_json::{json, Value};
fn request(action: Value, version: u16) -> Result<(), String> {
    let request: Request =
        serde_json::from_value(json!({"version":version,"id":1,"action":action}))
            .map_err(|e| e.to_string())?;
    request.validate().map_err(|e| e.to_string())
}
fn target() -> Value {
    json!({"pid":2,"handle":3,"created":4,"incarnation":5})
}
fn pointer(gesture: Value) -> Value {
    json!({"type":"window_pointer","target":target(),"gesture":gesture})
}
#[test]
fn bounded_extended_input_is_supported_only_by_v10_with_fresh_identity() {
    let actions = [
        pointer(json!({"kind":"click","at":{"x":100,"y":200},"button":"secondary","count":2})),
        pointer(json!({"kind":"wheel","at":{"x":100,"y":200},"horizontal":false,"delta":-120})),
        pointer(
            json!({"kind":"drag","from":{"x":0,"y":0},"to":{"x":32767,"y":32767},"button":"primary"}),
        ),
        json!({"type":"window_shortcut","target":target(),"key":65,"modifiers":1}),
        json!({"type":"window_resize","target":target(),"width":1280,"height":720}),
    ];
    for action in actions {
        request(action.clone(), 10).unwrap();
        for version in 1..10 {
            assert!(request(action.clone(), version).is_err());
        }
        let mut stale = action;
        stale["target"]["incarnation"] = json!(0);
        assert!(request(stale, 10).is_err());
    }
}
#[test]
fn invalid_extended_input_is_refused_before_mutation() {
    for gesture in [
        json!({"kind":"click","at":{"x":32768,"y":0},"button":"primary","count":1}),
        json!({"kind":"click","at":{"x":0,"y":0},"button":"primary","count":3}),
        json!({"kind":"wheel","at":{"x":0,"y":0},"horizontal":false,"delta":0}),
        json!({"kind":"wheel","at":{"x":0,"y":0},"horizontal":false,"delta":1201}),
        json!({"kind":"drag","from":{"x":0,"y":0},"to":{"x":0,"y":32768},"button":"primary"}),
    ] {
        assert!(request(pointer(gesture), 10).is_err());
    }
    for (key, modifiers) in [(65, 0), (65, 8), (0, 1), (0x5b, 1), (256, 1)] {
        assert!(request(
            json!({"type":"window_shortcut","target":target(),"key":key,"modifiers":modifiers}),
            10
        )
        .is_err());
    }
    for (width, height) in [(0, 600), (63, 600), (1024, 8193)] {
        assert!(request(
            json!({"type":"window_resize","target":target(),"width":width,"height":height}),
            10
        )
        .is_err());
    }
}
