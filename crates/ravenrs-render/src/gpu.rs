use std::fmt;

use ravenrs_core::{AdapterReport, GpuReport, SurfaceReport};

/// A device and queue ready for rendering.
#[derive(Debug, Clone)]
pub struct GpuContext {
    /// Physical GPU the device was created on.
    pub adapter: wgpu::Adapter,
    /// Logical device that owns all GPU resources.
    pub device: wgpu::Device,
    /// Command queue for uploads and submissions.
    pub queue: wgpu::Queue,
}

/// Failure to set up the GPU for rendering.
#[derive(Debug)]
pub enum GpuError {
    /// No adapter matched; on the web this usually means WebGPU is disabled.
    NoAdapter(String),
    /// The device request was rejected by the adapter.
    Device(String),
    /// The window or canvas surface could not be created or configured.
    Surface(String),
}

impl fmt::Display for GpuError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NoAdapter(e) => write!(f, "no compatible GPU adapter was found ({e})"),
            Self::Device(e) => write!(f, "the GPU device could not be created ({e})"),
            Self::Surface(e) => write!(f, "the drawing surface could not be created ({e})"),
        }
    }
}

impl std::error::Error for GpuError {}

impl GpuContext {
    /// Requests an adapter (compatible with `surface`, when given) and a device
    /// with default limits, which every WebGPU implementation supports.
    pub async fn new(
        instance: &wgpu::Instance,
        surface: Option<&wgpu::Surface<'_>>,
    ) -> Result<Self, GpuError> {
        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::default(),
                force_fallback_adapter: false,
                compatible_surface: surface,
                ..Default::default()
            })
            .await
            .map_err(|e| GpuError::NoAdapter(e.to_string()))?;
        let limits = if cfg!(target_arch = "wasm32") {
            wgpu::Limits::default().using_resolution(adapter.limits())
        } else {
            wgpu::Limits::default()
        };
        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor {
                label: Some("raven device"),
                required_limits: limits,
                ..Default::default()
            })
            .await
            .map_err(|e| GpuError::Device(e.to_string()))?;
        let info = adapter.get_info();
        if info.device_type == wgpu::DeviceType::Cpu {
            tracing::warn!(
                adapter = %info.name,
                driver = %info.driver,
                "rendering on the CPU, so the interface will be slow; no usable GPU driver was found"
            );
        }
        tracing::info!(adapter = ?info, "GPU ready");
        Ok(Self { adapter, device, queue })
    }
}

fn adapter_report(adapter: &wgpu::Adapter) -> AdapterReport {
    let info = adapter.get_info();
    AdapterReport {
        name: info.name,
        backend: info.backend.to_string(),
        device_type: format!("{:?}", info.device_type),
        driver: info.driver,
        driver_info: info.driver_info,
        software: info.device_type == wgpu::DeviceType::Cpu,
    }
}

/// Describes `format` and `present_mode` on a surface with these capabilities.
pub fn surface_report(
    capabilities: &wgpu::SurfaceCapabilities,
    format: wgpu::TextureFormat,
    present_mode: wgpu::PresentMode,
) -> SurfaceReport {
    SurfaceReport {
        format: format!("{format:?}"),
        present_mode: format!("{present_mode:?}"),
        supported_present_modes: capabilities.present_modes.iter().map(|m| format!("{m:?}")).collect(),
    }
}

/// Describes the adapter in use and the alternatives. `permissive` is an instance
/// that also accepts non-conformant drivers, since `instance` never shows those.
pub async fn gpu_report(
    active: &wgpu::Adapter,
    instance: &wgpu::Instance,
    permissive: &wgpu::Instance,
    surface: Option<SurfaceReport>,
) -> GpuReport {
    let list = async |instance: &wgpu::Instance| -> Vec<AdapterReport> {
        instance.enumerate_adapters(wgpu::Backends::all()).await.iter().map(adapter_report).collect()
    };
    let available = list(instance).await;
    let hidden = list(permissive).await.into_iter().filter(|adapter| !available.contains(adapter)).collect();
    GpuReport { active: adapter_report(active), available, hidden, surface }
}

/// Picks the surface format. Non-sRGB formats are preferred so blending happens
/// in sRGB space, matching browsers and design tools; `*Srgb` formats still work
/// because the shaders linearize colors when told to.
pub fn preferred_surface_format(formats: &[wgpu::TextureFormat]) -> Option<wgpu::TextureFormat> {
    formats.iter().copied().find(|f| !f.is_srgb()).or_else(|| formats.first().copied())
}
