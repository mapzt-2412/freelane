#[test]
fn host_process_memory_is_observed_not_configured_ram() {
    let first = freelane::telemetry::process(std::process::id()).unwrap();
    assert!(first.resident_bytes > 0);
    let second = freelane::telemetry::process(std::process::id()).unwrap();
    assert!(second.cpu_nanoseconds >= first.cpu_nanoseconds);
}
#[cfg(target_os = "linux")]
#[test]
fn proc_stat_parser_handles_spaces_and_rejects_invalid_counters() {
    let fields = (3..=24)
        .map(|n| match n {
            3 => "R".to_string(),
            14 => "20".into(),
            15 => "10".into(),
            22 => "100".into(),
            24 => "4".into(),
            _ => "0".into(),
        })
        .collect::<Vec<_>>()
        .join(" ");
    let value = freelane::telemetry::parse_linux_stat(
        &format!("1 (name with ) parentheses) {fields}"),
        100,
        4096,
    )
    .unwrap();
    assert_eq!(value.resident_bytes, 16384);
    assert_eq!(value.cpu_nanoseconds, 300_000_000);
    assert!(freelane::telemetry::parse_linux_stat("truncated", 100, 4096).is_err());
}
