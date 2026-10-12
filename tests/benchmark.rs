use freelane::benchmark::FrameReport;
#[test]
fn hitch_affects_low_fps_without_hiding_duration() {
    // Synthetic test data only, not a project benchmark.
    let mut frames = vec![10.0; 99];
    frames.push(100.0);
    let report = FrameReport::from_frames(&frames).unwrap();
    assert_eq!(report.frames, 100);
    assert!((report.duration_seconds - 1.09).abs() < 1e-12);
    assert!((report.average_fps - 100_000.0 / 1090.0).abs() < 1e-10);
    assert_eq!(report.one_percent_low_fps, 10.0);
    assert!((report.p99_frame_ms - 10.9).abs() < 1e-10);
    assert_eq!(report.frames_over_50_ms, 1);
    assert_eq!(report.median_frame_ms, 10.0);
}
#[test]
fn low_fps_averages_slowest_frames_instead_of_instantaneous_rates() {
    let mut frames = vec![10.0; 198];
    frames.extend([20.0, 100.0]);
    let report = FrameReport::from_frames(&frames).unwrap();
    assert!((report.one_percent_low_fps - 1000.0 / 60.0).abs() < 1e-12);
}
#[test]
fn malformed_or_insufficient_measurements_are_rejected() {
    for text in [
        "",
        "fps\n60",
        "frame_ms\nNaN",
        "frame_ms\ninf",
        "frame_ms\n0",
        "frame_ms\n-1",
        "frame_ms\n10,20",
        "frame_ms\n10",
    ] {
        assert!(FrameReport::from_csv(text).is_err(), "{text}");
    }
    assert!(FrameReport::from_frames(&vec![f64::MAX; 100]).is_err());
    assert!(FrameReport::from_frames(&vec![f64::MIN_POSITIVE; 100]).is_err());
}
