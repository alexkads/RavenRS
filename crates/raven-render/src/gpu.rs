use std::fmt;

/// A device and queue ready for rendering.
#[derive(Debug, Clone)]
pub struct GpuContext {
    pub adapter: wgpu::Adapter,
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
}

#[derive(Debug)]
pub enum GpuError {
    /// No adapter matched; on the web this usually means WebGPU is disabled.
    NoAdapter(String),
    Device(String),
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
        tracing::info!(adapter = ?adapter.get_info(), "GPU ready");
        Ok(Self { adapter, device, queue })
    }
}

/// Picks the surface format. Non-sRGB formats are preferred so blending happens
/// in sRGB space, matching browsers and design tools; `*Srgb` formats still work
/// because the shaders linearize colors when told to.
pub fn preferred_surface_format(formats: &[wgpu::TextureFormat]) -> Option<wgpu::TextureFormat> {
    formats.iter().copied().find(|f| !f.is_srgb()).or_else(|| formats.first().copied())
}
