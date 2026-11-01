//! Describes the GPU and the machine an app runs on, from its panel or from the
//! command line.

use std::sync::Arc;

use ravenrs_core::{DiagnosticReport, SurfaceReport, SystemReport};
use ravenrs_render::{GpuContext, gpu_report, preferred_surface_format, surface_report, wgpu};
use winit::application::ApplicationHandler;
use winit::event::WindowEvent;
use winit::event_loop::{ActiveEventLoop, EventLoop, OwnedDisplayHandle};
use winit::window::{Window, WindowId};

/// The variables that change which GPU is used or how it is reached.
const OVERRIDES: [&str; 9] = [
    "WGPU_BACKEND",
    "WGPU_ADAPTER_NAME",
    "WGPU_POWER_PREF",
    "WGPU_ALLOW_UNDERLYING_NONCOMPLIANT_ADAPTER",
    "VK_ICD_FILENAMES",
    "VK_DRIVER_FILES",
    "LIBGL_ALWAYS_SOFTWARE",
    "MESA_LOADER_DRIVER_OVERRIDE",
    "GALLIUM_DRIVER",
];

/// An instance for `display`, which lets the GL backend reach the window system.
/// `permissive` also accepts drivers that are not conformant.
pub(crate) fn new_instance(display: Option<&OwnedDisplayHandle>, permissive: bool) -> wgpu::Instance {
    let mut descriptor = match display {
        Some(display) => {
            wgpu::InstanceDescriptor::new_with_display_handle_from_env(Box::new(display.clone()))
        }
        None => wgpu::InstanceDescriptor::new_without_display_handle_from_env(),
    };
    if permissive {
        descriptor.flags |= wgpu::InstanceFlags::ALLOW_UNDERLYING_NONCOMPLIANT_ADAPTER;
    }
    wgpu::Instance::new(descriptor)
}

/// The report for an app that ended up on `adapter`.
pub(crate) fn gather(
    display: Option<&OwnedDisplayHandle>,
    adapter: &wgpu::Adapter,
    surface: Option<SurfaceReport>,
) -> DiagnosticReport {
    let (instance, permissive) = (new_instance(display, false), new_instance(display, true));
    let gpu = pollster::block_on(gpu_report(adapter, &instance, &permissive, surface));
    DiagnosticReport { gpu: Ok(gpu), system: system_report() }
}

fn system_report() -> SystemReport {
    SystemReport {
        version: env!("CARGO_PKG_VERSION").into(),
        os: os_name(),
        session: std::env::var("XDG_SESSION_TYPE").ok(),
        desktop: std::env::var("XDG_CURRENT_DESKTOP").ok(),
        vulkan_drivers: vulkan_drivers(),
        overrides: OVERRIDES
            .iter()
            .filter_map(|name| Some(format!("{name}={}", std::env::var(name).ok()?)))
            .collect(),
        optimized: !cfg!(debug_assertions),
    }
}

/// The distribution's name on Linux; the system's elsewhere.
fn os_name() -> String {
    let release = std::fs::read_to_string("/etc/os-release").unwrap_or_default();
    pretty_name(&release).unwrap_or_else(|| std::env::consts::OS.to_owned())
}

fn pretty_name(os_release: &str) -> Option<String> {
    let line = os_release.lines().find_map(|line| line.strip_prefix("PRETTY_NAME="))?;
    Some(line.trim_matches('"').to_owned())
}

/// The driver files the Vulkan loader looks at on Linux.
fn vulkan_drivers() -> Vec<String> {
    let mut drivers: Vec<String> = ["/usr/share/vulkan/icd.d", "/etc/vulkan/icd.d"]
        .iter()
        .filter_map(|dir| std::fs::read_dir(dir).ok())
        .flatten()
        .filter_map(|entry| Some(entry.ok()?.file_name().to_string_lossy().into_owned()))
        .collect();
    drivers.sort();
    drivers
}

/// Finds the GPU the way a window would, and describes it.
fn probe(display: Option<&OwnedDisplayHandle>, window: Option<Arc<Window>>) -> DiagnosticReport {
    let instance = new_instance(display, false);
    let surface = window.and_then(|window| instance.create_surface(window).ok());
    let failed = |e: String| DiagnosticReport { gpu: Err(e), system: system_report() };
    let gpu = match pollster::block_on(GpuContext::new(&instance, surface.as_ref())) {
        Ok(gpu) => gpu,
        Err(e) => return failed(e.to_string()),
    };
    let described = surface.as_ref().and_then(|surface| {
        let capabilities = surface.get_capabilities(&gpu.adapter);
        let format = preferred_surface_format(&capabilities.formats)?;
        Some(surface_report(&capabilities, format, wgpu::PresentMode::AutoVsync))
    });
    gather(display, &gpu.adapter, described)
}

struct Probe {
    display: OwnedDisplayHandle,
    report: Option<DiagnosticReport>,
}

impl ApplicationHandler for Probe {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.report.is_some() {
            return;
        }
        // Never shown: a surface needs a window, and which GPU works depends on it.
        let attributes = Window::default_attributes().with_visible(false);
        let window = event_loop.create_window(attributes).ok().map(Arc::new);
        self.report = Some(probe(Some(&self.display), window));
        event_loop.exit();
    }

    fn window_event(&mut self, _event_loop: &ActiveEventLoop, _id: WindowId, _event: WindowEvent) {}
}

/// Finds the GPU the way a window would and describes it, along with the machine
/// and the build, without showing anything. For a command line option such as
/// `--diagnostics`, which still answers when a window would not open or would be
/// too slow to use. Printing the result gives text to paste into a bug report.
///
/// Call it instead of running the app: a process has only one event loop.
pub fn diagnose() -> DiagnosticReport {
    let Ok(event_loop) = EventLoop::new() else {
        // No window system at all, as over SSH: adapters can still be listed.
        return probe(None, None);
    };
    let mut probing = Probe { display: event_loop.owned_display_handle(), report: None };
    match event_loop.run_app(&mut probing) {
        Ok(()) => probing.report.unwrap_or_else(|| probe(None, None)),
        Err(e) => {
            tracing::warn!(error = %e, "could not run the event loop");
            probe(None, None)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_distribution_name_comes_from_os_release() {
        let release =
            "NAME=\"Fedora Linux\"\nPRETTY_NAME=\"Fedora Linux 44 (Workstation Edition)\"\nID=fedora\n";
        assert_eq!(pretty_name(release).as_deref(), Some("Fedora Linux 44 (Workstation Edition)"));
        assert_eq!(pretty_name("ID=fedora\n"), None);
    }
}
