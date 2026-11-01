//! What a running app can say about its GPU and its frames, as plain data: the
//! renderer and the platform fill it in, and widgets show it without either
//! knowing about the other.

use std::collections::VecDeque;
use std::fmt;
use std::time::Duration;

/// Frames a [`FrameLog`] keeps.
const FRAME_LOG_SIZE: usize = 240;
/// The longest URL [`new_issue_url`] builds. Servers answer longer ones with an
/// error instead of the page, and GitHub's limit is not documented, so this stays
/// well under the 8 KB many servers allow.
pub const MAX_ISSUE_URL: usize = 6000;
/// Where a path starts to name a person's own folder.
const USER_FOLDERS: [&str; 4] = ["/home/", "/Users/", "\\Users\\", "\\users\\"];

/// One GPU as the graphics API describes it.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct AdapterReport {
    /// The GPU's name.
    pub name: String,
    /// The graphics API: Vulkan, Metal, DX12, GL or WebGPU.
    pub backend: String,
    /// Integrated, discrete, virtual or CPU.
    pub device_type: String,
    /// The driver's name.
    pub driver: String,
    /// The driver's version and details.
    pub driver_info: String,
    /// It draws on the CPU, with no GPU behind it.
    pub software: bool,
}

impl fmt::Display for AdapterReport {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} ({}, {}", self.name, self.backend, self.device_type)?;
        if !self.driver.is_empty() {
            write!(f, ", {}", self.driver)?;
        }
        if !self.driver_info.is_empty() {
            write!(f, " {}", self.driver_info)?;
        }
        f.write_str(")")
    }
}

/// How frames reach the window.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SurfaceReport {
    /// The pixel format in use.
    pub format: String,
    /// The present mode in use.
    pub present_mode: String,
    /// The present modes the surface supports.
    pub supported_present_modes: Vec<String>,
}

/// The GPU an app ended up on and the ones it could have used.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct GpuReport {
    /// The adapter in use.
    pub active: AdapterReport,
    /// Every adapter the graphics API offers.
    pub available: Vec<AdapterReport>,
    /// Adapters that exist but are left out because their driver is not
    /// conformant, which an old GPU's driver may not be.
    pub hidden: Vec<AdapterReport>,
    /// The window's surface; `None` when no window could be opened.
    pub surface: Option<SurfaceReport>,
}

/// The machine and the build around the GPU.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SystemReport {
    /// The RavenRS version.
    pub version: String,
    /// The operating system.
    pub os: String,
    /// The window system session, such as `wayland` or `x11`, when known.
    pub session: Option<String>,
    /// The desktop, such as `GNOME`, when known.
    pub desktop: Option<String>,
    /// Vulkan drivers installed on the system, on Linux.
    pub vulkan_drivers: Vec<String>,
    /// Environment variables set that change which GPU is used, as `NAME=value`.
    pub overrides: Vec<String>,
    /// Built with optimizations. Without them everything is many times slower.
    pub optimized: bool,
}

/// Something in a [`DiagnosticReport`] that explains a slow or broken app.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DiagnosticFinding {
    /// Frames are drawn on the CPU.
    SoftwareRendering,
    /// The app was built without optimizations.
    UnoptimizedBuild,
    /// These adapters exist but their drivers are not conformant, so they are not used.
    HiddenAdapters(Vec<String>),
    /// No GPU could be used at all, for this reason.
    NoGpu(String),
}

impl fmt::Display for DiagnosticFinding {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::SoftwareRendering => f.write_str("frames are drawn on the CPU"),
            Self::UnoptimizedBuild => f.write_str("built without optimizations"),
            Self::HiddenAdapters(names) => write!(f, "non-conformant drivers left out: {}", names.join(", ")),
            Self::NoGpu(reason) => write!(f, "no usable GPU: {reason}"),
        }
    }
}

/// Everything known about where an app runs, to read or to paste into a bug report.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DiagnosticReport {
    /// The GPU, or why there is none.
    pub gpu: Result<GpuReport, String>,
    /// The machine and the build.
    pub system: SystemReport,
}

impl Default for DiagnosticReport {
    /// No GPU was looked for yet.
    fn default() -> Self {
        Self { gpu: Err("not checked".into()), system: SystemReport::default() }
    }
}

impl DiagnosticReport {
    /// What in this report explains a slow or broken app, most serious first.
    ///
    /// ```
    /// use ravenrs_core::{DiagnosticFinding, DiagnosticReport};
    ///
    /// let report = DiagnosticReport { gpu: Err("no driver".into()), ..Default::default() };
    /// assert_eq!(report.findings()[0], DiagnosticFinding::NoGpu("no driver".into()));
    /// ```
    pub fn findings(&self) -> Vec<DiagnosticFinding> {
        let mut findings = Vec::new();
        match &self.gpu {
            Err(reason) => findings.push(DiagnosticFinding::NoGpu(reason.clone())),
            Ok(gpu) => {
                if gpu.active.software {
                    findings.push(DiagnosticFinding::SoftwareRendering);
                }
                if !gpu.hidden.is_empty() {
                    let names = gpu.hidden.iter().map(ToString::to_string).collect();
                    findings.push(DiagnosticFinding::HiddenAdapters(names));
                }
            }
        }
        if !self.system.optimized {
            findings.push(DiagnosticFinding::UnoptimizedBuild);
        }
        findings
    }
}

impl fmt::Display for DiagnosticReport {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let system = &self.system;
        writeln!(f, "RavenRS: {}", system.version)?;
        writeln!(f, "Build: {}", if system.optimized { "optimized" } else { "NOT optimized" })?;
        writeln!(f, "System: {}", system.os)?;
        if let Some(session) = &system.session {
            writeln!(f, "Session: {session}")?;
        }
        if let Some(desktop) = &system.desktop {
            writeln!(f, "Desktop: {desktop}")?;
        }
        match &self.gpu {
            Err(reason) => writeln!(f, "GPU: none ({reason})")?,
            Ok(gpu) => {
                writeln!(f, "GPU in use: {}", gpu.active)?;
                for adapter in &gpu.available {
                    writeln!(f, "Available: {adapter}")?;
                }
                for adapter in &gpu.hidden {
                    writeln!(f, "Hidden, non-conformant: {adapter}")?;
                }
                match &gpu.surface {
                    Some(surface) => writeln!(
                        f,
                        "Surface: {}, {} (supports {})",
                        surface.format,
                        surface.present_mode,
                        surface.supported_present_modes.join(", ")
                    )?,
                    None => writeln!(f, "Surface: none, no window could be opened")?,
                }
            }
        }
        if !system.vulkan_drivers.is_empty() {
            writeln!(f, "Vulkan drivers installed: {}", system.vulkan_drivers.join(", "))?;
        }
        if !system.overrides.is_empty() {
            writeln!(f, "Overrides: {}", system.overrides.join(" "))?;
        }
        for finding in self.findings() {
            writeln!(f, "Warning: {finding}")?;
        }
        Ok(())
    }
}

/// `text` with every path inside a person's own folder replaced, since those
/// paths usually carry the person's name. A path can have spaces, so what follows
/// goes up to the end of the line or a quote or list separator: too much hidden is
/// better than a surname left behind.
///
/// ```
/// use ravenrs_core::redact_user_paths;
///
/// let text = "Overrides: VK_DRIVER_FILES=/home/ana/icd.json\nGPU: Intel";
/// assert_eq!(redact_user_paths(text), "Overrides: VK_DRIVER_FILES=<user folder>\nGPU: Intel");
/// ```
pub fn redact_user_paths(text: &str) -> String {
    let mut redacted = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(at) = USER_FOLDERS.iter().filter_map(|folder| rest.find(folder)).min() {
        redacted.push_str(&rest[..at]);
        redacted.push_str("<user folder>");
        rest = &rest[at..];
        let end = rest.find(['\n', '"', '\'', ',', ';']).unwrap_or(rest.len());
        rest = &rest[end..];
    }
    redacted.push_str(rest);
    redacted
}

/// Whether a byte stands for itself in a URL's query.
fn is_unreserved(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'.' | b'_' | b'~')
}

fn percent_encode(text: &str) -> String {
    let mut encoded = String::with_capacity(text.len());
    for byte in text.bytes() {
        if is_unreserved(byte) {
            encoded.push(char::from(byte));
        } else {
            encoded.push_str(&format!("%{byte:02X}"));
        }
    }
    encoded
}

/// The URL of a new issue with `intro` and then `report` in a code block as its
/// body, for the person to review and send on the page it opens. It never passes
/// [`MAX_ISSUE_URL`]: a report that would make it longer loses its last lines, whole
/// where possible, and `truncated_note` says so after the block.
///
/// `new_issue` is the page for new issues and may already have a query, such as
/// `https://github.com/owner/repo/issues/new`.
///
/// ```
/// use ravenrs_core::new_issue_url;
///
/// let url = new_issue_url("https://example.com/new", "Steps:", "GPU: Intel\n", "");
/// assert_eq!(url, "https://example.com/new?body=Steps%3A%0A%0A%60%60%60text%0AGPU%3A%20Intel%0A%60%60%60%0A");
/// ```
pub fn new_issue_url(new_issue: &str, intro: &str, report: &str, truncated_note: &str) -> String {
    let joiner = if new_issue.contains('?') { '&' } else { '?' };
    let url = |report: &str, note: &str| {
        let body = format!("{intro}\n\n```text\n{report}\n```\n{note}");
        format!("{new_issue}{joiner}body={}", percent_encode(&body))
    };
    let report = report.trim_end();
    let whole = url(report, "");
    if whole.len() <= MAX_ISSUE_URL {
        return whole;
    }
    let room = MAX_ISSUE_URL.saturating_sub(url("", truncated_note).len());
    let mut used = 0;
    let mut end = 0;
    for (at, c) in report.char_indices() {
        used += percent_encode(c.encode_utf8(&mut [0; 4])).len();
        if used > room {
            break;
        }
        end = at + c.len_utf8();
    }
    let kept = &report[..end];
    url(kept.rfind('\n').map_or(kept, |line| &kept[..line]), truncated_note)
}

/// What one frame cost. All durations are time on the CPU: waiting for a busy
/// GPU shows up in [`Self::acquire`] and [`Self::present`], not in [`Self::render`].
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct FrameSample {
    /// Rebuilding, laying out and painting the UI.
    pub update: Duration,
    /// Waiting for a texture to draw on, which vsync and a busy GPU make longer.
    pub acquire: Duration,
    /// Recording and submitting the GPU's work.
    pub render: Duration,
    /// Handing the texture to the window system.
    pub present: Duration,
    /// Quads drawn.
    pub quads: usize,
    /// Glyphs drawn.
    pub glyphs: usize,
    /// Draw calls issued.
    pub draw_calls: usize,
    /// The drawing surface in physical pixels.
    pub size: (u32, u32),
    /// Physical pixels per logical pixel.
    pub scale: f32,
}

impl FrameSample {
    /// The whole frame.
    pub fn total(&self) -> Duration {
        self.update + self.acquire + self.render + self.present
    }
}

/// The median and the 95th percentile of a duration.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Percentiles {
    /// Half of the frames took less than this.
    pub median: Duration,
    /// 95% of the frames took less than this.
    pub p95: Duration,
}

impl Percentiles {
    fn of(mut values: Vec<Duration>) -> Self {
        values.sort_unstable();
        let at = |fraction: f64| {
            let last = values.len().saturating_sub(1);
            values.get((last as f64 * fraction).round() as usize).copied().unwrap_or_default()
        };
        Self { median: at(0.5), p95: at(0.95) }
    }
}

/// The recent frames of a [`FrameLog`], summed up.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct FrameSummary {
    /// Frames summed up.
    pub frames: usize,
    /// See [`FrameSample::update`].
    pub update: Percentiles,
    /// See [`FrameSample::acquire`].
    pub acquire: Percentiles,
    /// See [`FrameSample::render`].
    pub render: Percentiles,
    /// See [`FrameSample::present`].
    pub present: Percentiles,
    /// See [`FrameSample::total`].
    pub total: Percentiles,
    /// The latest frame, for its counts and its size.
    pub latest: FrameSample,
}

impl fmt::Display for FrameSummary {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let latest = &self.latest;
        writeln!(f, "Frames: {} (median / p95)", self.frames)?;
        let stages = [
            ("Total", self.total),
            ("Update", self.update),
            ("Acquire", self.acquire),
            ("Render", self.render),
            ("Present", self.present),
        ];
        for (name, stage) in stages {
            writeln!(f, "  {name}: {:.1?} / {:.1?}", stage.median, stage.p95)?;
        }
        writeln!(
            f,
            "Latest frame: {}x{} at scale {}, {} quads, {} glyphs, {} draw calls",
            latest.size.0, latest.size.1, latest.scale, latest.quads, latest.glyphs, latest.draw_calls
        )
    }
}

/// The latest frames an app drew. Recording one is a push into a fixed buffer,
/// so it can stay on while nobody is looking.
///
/// ```
/// use std::time::Duration;
/// use ravenrs_core::{FrameLog, FrameSample};
///
/// let mut log = FrameLog::default();
/// log.record(FrameSample { update: Duration::from_millis(4), ..Default::default() });
/// assert_eq!(log.summary().total.median, Duration::from_millis(4));
/// ```
#[derive(Clone, Debug, Default)]
pub struct FrameLog {
    samples: VecDeque<FrameSample>,
}

impl FrameLog {
    /// Adds a frame, forgetting the oldest once the log is full.
    pub fn record(&mut self, sample: FrameSample) {
        if self.samples.len() == FRAME_LOG_SIZE {
            self.samples.pop_front();
        }
        self.samples.push_back(sample);
    }

    /// The frames in the log, summed up.
    pub fn summary(&self) -> FrameSummary {
        let of =
            |stage: fn(&FrameSample) -> Duration| Percentiles::of(self.samples.iter().map(stage).collect());
        FrameSummary {
            frames: self.samples.len(),
            update: of(|s| s.update),
            acquire: of(|s| s.acquire),
            render: of(|s| s.render),
            present: of(|s| s.present),
            total: of(FrameSample::total),
            latest: self.samples.back().copied().unwrap_or_default(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn gpu(software: bool) -> GpuReport {
        GpuReport {
            active: AdapterReport { name: "llvmpipe".into(), software, ..Default::default() },
            ..Default::default()
        }
    }

    #[test]
    fn a_healthy_machine_has_nothing_to_report() {
        let report = DiagnosticReport {
            gpu: Ok(gpu(false)),
            system: SystemReport { optimized: true, ..Default::default() },
        };
        assert!(report.findings().is_empty());
        assert!(!report.to_string().contains("Warning"));
    }

    #[test]
    fn drawing_on_the_cpu_comes_first() {
        let report = DiagnosticReport { gpu: Ok(gpu(true)), system: SystemReport::default() };
        assert_eq!(
            report.findings(),
            vec![DiagnosticFinding::SoftwareRendering, DiagnosticFinding::UnoptimizedBuild]
        );
        assert!(report.to_string().contains("Warning: frames are drawn on the CPU"));
    }

    #[test]
    fn an_adapter_hidden_for_its_driver_is_named() {
        let mut gpu = gpu(true);
        gpu.hidden.push(AdapterReport { name: "Intel Haswell".into(), ..Default::default() });
        let report = DiagnosticReport { gpu: Ok(gpu), ..Default::default() };
        assert!(report.to_string().contains("Hidden, non-conformant: Intel Haswell"));
        assert!(
            matches!(&report.findings()[1], DiagnosticFinding::HiddenAdapters(names) if names.len() == 1)
        );
    }

    #[test]
    fn percentiles_ignore_a_lone_slow_frame_in_the_median() {
        let mut log = FrameLog::default();
        for ms in 1..=99 {
            log.record(FrameSample { update: Duration::from_millis(ms), ..Default::default() });
        }
        log.record(FrameSample { update: Duration::from_secs(5), ..Default::default() });
        let summary = log.summary();
        assert_eq!(summary.frames, 100);
        assert_eq!(summary.update.median, Duration::from_millis(51));
        assert_eq!(summary.update.p95, Duration::from_millis(95));
    }

    #[test]
    fn the_log_forgets_the_oldest_frames() {
        let mut log = FrameLog::default();
        for _ in 0..FRAME_LOG_SIZE + 10 {
            log.record(FrameSample::default());
        }
        assert_eq!(log.summary().frames, FRAME_LOG_SIZE);
    }

    #[test]
    fn an_empty_log_sums_up_to_nothing() {
        assert_eq!(FrameLog::default().summary(), FrameSummary::default());
    }
}
