//! Summarize measured frame durations. No projected performance numbers.
use crate::{Error, Result};
use serde::Serialize;
#[derive(Debug, Serialize)]
pub struct FrameReport {
    pub frames: usize,
    pub duration_seconds: f64,
    pub average_fps: f64,
    pub one_percent_low_fps: f64,
    pub median_frame_ms: f64,
    pub p95_frame_ms: f64,
    pub p99_frame_ms: f64,
    pub worst_frame_ms: f64,
    pub frames_over_50_ms: usize,
}
impl FrameReport {
    /// Input has the single column `frame_ms`; each subsequent row is a positive measured duration.
    pub fn from_csv(csv: &str) -> Result<Self> {
        let mut lines = csv.lines();
        if lines.next().map(str::trim) != Some("frame_ms") {
            return Err(Error(
                "Expected a single-column CSV headed frame_ms (milliseconds)".into(),
            ));
        }
        let mut frames = Vec::new();
        for (index, line) in lines.enumerate() {
            if line.trim().is_empty() {
                continue;
            }
            let value: f64 = line
                .trim()
                .parse()
                .map_err(|_| Error(format!("Invalid frame duration at CSV line {}", index + 2)))?;
            if !value.is_finite() || value <= 0.0 {
                return Err(Error(format!(
                    "Frame duration at CSV line {} must be finite and positive",
                    index + 2
                )));
            }
            frames.push(value);
        }
        Self::from_frames(&frames)
    }
    pub fn from_frames(frames: &[f64]) -> Result<Self> {
        if frames.len() < 100 {
            return Err(Error(
                "At least 100 measured frames are required for a 1% low report".into(),
            ));
        }
        if frames.iter().any(|v| !v.is_finite() || *v <= 0.0) {
            return Err(Error("Frame durations must be finite and positive".into()));
        }
        // Compensated summation reduces rounding loss across long captures.
        let sum = |values: &[f64]| {
            let (mut total, mut correction) = (0.0, 0.0);
            for value in values {
                let adjusted = value - correction;
                let next = total + adjusted;
                correction = (next - total) - adjusted;
                total = next;
            }
            total
        };
        let total_ms = sum(frames);
        if !total_ms.is_finite() {
            return Err(Error("Total capture duration overflowed".into()));
        }
        let mut sorted = frames.to_vec();
        sorted.sort_by(f64::total_cmp);
        let slow_count = frames.len().div_ceil(100);
        let slow_mean = sum(&sorted[sorted.len() - slow_count..]) / slow_count as f64;
        let percentile = |p: f64| {
            let rank = (sorted.len() - 1) as f64 * p;
            let lo = rank.floor() as usize;
            let hi = rank.ceil() as usize;
            sorted[lo] + (sorted[hi] - sorted[lo]) * (rank - lo as f64)
        };
        let result = Self {
            frames: frames.len(),
            duration_seconds: total_ms / 1000.0,
            average_fps: 1000.0 * frames.len() as f64 / total_ms,
            one_percent_low_fps: 1000.0 / slow_mean,
            median_frame_ms: percentile(0.5),
            p95_frame_ms: percentile(0.95),
            p99_frame_ms: percentile(0.99),
            worst_frame_ms: *sorted.last().unwrap(),
            frames_over_50_ms: frames.iter().filter(|v| **v > 50.0).count(),
        };
        if !result.average_fps.is_finite() || !result.one_percent_low_fps.is_finite() {
            return Err(Error("Frame rate calculation overflowed".into()));
        }
        Ok(result)
    }
}
