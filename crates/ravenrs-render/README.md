# ravenrs-render

The RavenRS GPU renderer. It takes a `Scene`, the display list that RavenRS views paint into, and draws it on a wgpu texture: rounded rectangles with borders, gradients and soft shadows, images, shapes made of curves, and text.

This crate is part of [RavenRS](https://github.com/alexkads/RavenRS).

## Who should use it directly

Almost nobody. An app built with RavenRS depends on the [`ravenrs`](https://crates.io/crates/ravenrs) facade and runs on a platform crate, [`ravenrs-platform-native`](https://crates.io/crates/ravenrs-platform-native) for desktop windows or [`ravenrs-platform-web`](https://crates.io/crates/ravenrs-platform-web) for a browser canvas. The platform creates the GPU device, the window surface and one `Renderer`, and calls it every frame. The app never sees it. The facade re-exports this crate as `ravenrs::render` for the few who need it.

Reach for it directly when you:

- draw RavenRS scenes into a surface you own: an engine, an editor or a game that already has a wgpu device and wants a RavenRS panel inside it;
- draw without any window: golden-image tests, thumbnails, documentation pictures, a server that renders a page to PNG;
- write a new platform crate.

Where it sits:

- `ravenrs-core` defines `Scene` and its primitives (`Quad`, `GlyphRun`, `ImageQuad`, `PathFill`, clips). It knows nothing of the GPU.
- `ravenrs-text` shapes and lays out text and paints it into a `Scene` as glyph runs. This crate only rasterizes the glyphs those runs name.
- `ravenrs-render`, this crate, turns the `Scene` into GPU work. It owns no window, no surface and no event loop.
- The platform crates own the window or canvas, configure the surface, track size and scale, and present the frame.

## Draw a scene with no window

This renders a card with a shadow and a clipped image into memory and reads the pixels back. It needs a GPU adapter, so the doctest is compiled but not run; the tests of this crate (`tests/offscreen.rs`) and the golden-image tests of the facade (`crates/ravenrs/tests/suite/visual.rs`) do the same thing on every check.

```rust,no_run
use ravenrs_core::{Color, ImageData, ImageQuad, Quad, Rect, Scene, Shadow};
use ravenrs_render::{GpuContext, render_to_rgba, wgpu};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // `_from_env` lets WGPU_BACKEND and the other wgpu variables pick the backend.
    let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle_from_env());
    // No surface, so any adapter will do. `GpuContext::new` is async; block on it outside an executor.
    let gpu = pollster::block_on(GpuContext::new(&instance, None))?;

    // Coordinates are logical pixels; `render_to_rgba` draws at scale 1.0.
    let mut scene = Scene::new();
    scene.set_clear_color(Color::hex(0x0b0d12));
    scene.push_quad(
        Quad::new(Rect::new(20.0, 20.0, 160.0, 60.0), Color::hex(0x3b82f6))
            .radius(12.0)
            .border(2.0, Color::WHITE)
            .shadow(Shadow::soft()),
    );
    let checker = ImageData::from_fn(8, 8, |x, y| if (x + y) % 2 == 0 { Color::WHITE } else { Color::BLACK })
        .ok_or("the image size does not match its pixels")?;
    // Everything between push_clip and pop_clip is cut to the rectangle.
    scene.push_clip(Rect::new(30.0, 30.0, 20.0, 20.0));
    scene.push_image(ImageQuad::new(Rect::new(30.0, 30.0, 40.0, 40.0), checker).radius(4.0));
    scene.pop_clip();

    let (width, height) = (200, 100);
    let pixels = render_to_rgba(&gpu, &scene, width, height)?;
    assert_eq!(pixels.len(), (width * height * 4) as usize);
    // Row by row, 4 bytes per pixel: R, G, B, A.
    let at = |x: u32, y: u32| &pixels[((y * width + x) * 4) as usize..][..4];
    println!("card center: {:?}, corner: {:?}", at(100, 50), at(1, 1));
    Ok(())
}
```

`render_to_rgba` builds a new `Renderer` on each call, so it compiles the shaders and rasterizes every glyph again each time. It is right for one picture. To draw many, keep one `Renderer` and render into a texture of your own, as the next section does, then copy the texture to a buffer the way `render_to_rgba` does in `src/offscreen.rs`.

The pixels are `Rgba8Unorm`, in sRGB, with premultiplied alpha: wherever the clear color is opaque the image is opaque, and a transparent clear color leaves premultiplied edges.

Text needs `ravenrs-text`: lay the string out with its `TextSystem`, call `paint` on the layout with the scene, and the glyph runs land in the scene like any other primitive. A whole interface works the same way through the facade: `Ui::new(view, theme)` from `ravenrs::prelude`, then `set_viewport` and `update`, then render `ui.scene()`. That is what the golden-image tests do.

## Draw into a surface you own

A `Renderer` belongs to one wgpu device and one target format. Create it once and call `render` every frame; it keeps its pipelines, glyph atlas, image textures and instance buffers between frames.

```rust,no_run
use ravenrs_core::Scene;
use ravenrs_render::{GpuContext, GpuError, Renderer, preferred_surface_format, wgpu};

/// What a platform keeps for one window.
struct Target<'w> {
    surface: wgpu::Surface<'w>,
    config: wgpu::SurfaceConfiguration,
    gpu: GpuContext,
    renderer: Renderer,
}

/// `surface` comes from `instance.create_surface(window)`; `width` and `height` are device pixels.
fn create<'w>(
    instance: &wgpu::Instance,
    surface: wgpu::Surface<'w>,
    width: u32,
    height: u32,
) -> Result<Target<'w>, GpuError> {
    // Passing the surface makes sure the adapter can present to it.
    let gpu = pollster::block_on(GpuContext::new(instance, Some(&surface)))?;
    let caps = surface.get_capabilities(&gpu.adapter);
    let format = preferred_surface_format(&caps.formats)
        .ok_or_else(|| GpuError::Surface("the surface reports no formats".into()))?;
    let config = wgpu::SurfaceConfiguration {
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
        format,
        width: width.max(1),
        height: height.max(1),
        present_mode: wgpu::PresentMode::AutoVsync,
        desired_maximum_frame_latency: 2,
        alpha_mode: caps.alpha_modes.first().copied().unwrap_or(wgpu::CompositeAlphaMode::Auto),
        view_formats: vec![],
        color_space: wgpu::SurfaceColorSpace::Auto,
    };
    surface.configure(&gpu.device, &config);
    // The renderer is built for the format the surface was configured with.
    let renderer = Renderer::new(&gpu.device, &gpu.queue, format);
    Ok(Target { surface, config, gpu, renderer })
}

/// Call when the window's size in device pixels changes, which includes a change of scale factor.
fn resize(target: &mut Target<'_>, width: u32, height: u32) {
    target.config.width = width.max(1);
    target.config.height = height.max(1);
    target.surface.configure(&target.gpu.device, &target.config);
}

/// `scale` is device pixels per logical pixel: 2.0 on most high-density screens.
fn draw(target: &mut Target<'_>, scene: &Scene, scale: f32) {
    let frame = match target.surface.get_current_texture() {
        wgpu::CurrentSurfaceTexture::Success(frame) | wgpu::CurrentSurfaceTexture::Suboptimal(frame) => frame,
        // The window changed under the surface: configure it again and draw on the next frame.
        wgpu::CurrentSurfaceTexture::Outdated | wgpu::CurrentSurfaceTexture::Lost => {
            target.surface.configure(&target.gpu.device, &target.config);
            return;
        }
        // Hidden, occluded or timed out: skip this frame.
        _ => return,
    };
    let view = frame.texture.create_view(&wgpu::TextureViewDescriptor::default());
    let size = (target.config.width, target.config.height);
    let stats = target.renderer.render(scene, &view, size, scale);
    target.gpu.queue.present(frame);
    println!("{} draw calls, {} glyphs", stats.draw_calls, stats.glyphs);
}
```

### The contract with the platform

- **Device and queue.** `GpuContext::new` picks the adapter and creates the device and queue. You may bring your own instead: `Renderer::new` takes any `wgpu::Device` and `wgpu::Queue`. It needs only the default limits (WebGL2's on WebGL2) and uses two optional features when the device has them: `FLOAT32_BLENDABLE`, for exact coverage of curved shapes, and `TIMESTAMP_QUERY`, for GPU timing. A device without them still draws everything.
- **Format.** Use `preferred_surface_format`. It prefers a format that is not sRGB, so colors blend in sRGB space as browsers and design tools do. An `*Srgb` format works too: the renderer sees it in the format given to `Renderer::new` and converts colors in the shaders. The texture passed to `render` must have the format the renderer was built for, or wgpu reports a validation error; another format needs another `Renderer`.
- **Scale.** The scene is in logical pixels. `render` takes the target size in device pixels and `scale`, the device pixels per logical pixel, and works out the logical viewport as size divided by scale. Edges, borders and text are rasterized at device resolution, so a scale change needs no new renderer: glyphs at the new size enter the atlas as they are first drawn.
- **When to resize.** Configure the surface again whenever its size in device pixels changes (a resize, or a move to a screen with another scale) and pass the new size to `render` in the same frame. A size of 0 or a scale of 0 or less draws nothing and returns empty stats, so a minimized window is safe; the surface itself must stay configured at 1×1 or more.
- **Threads and blocking.** `render` records one command buffer and submits it. It does not wait for the GPU or read anything back; only `render_to_rgba` waits, for its copy. Glyph rasterization and image uploads happen inside `render`, on the calling thread. The platforms keep the renderer on the thread that runs the event loop.

## How it works

Each call to `render` walks the scene once, in painter's order:

- **Quads** (rounded rectangles, borders, linear gradients, soft shadows) are instances shaded with a signed-distance function (`shaders/quad.wgsl`), so edges are anti-aliased analytically at any scale factor. A shadow is one more instance under its quad.
- **Images** are uploaded once, with a full mip chain, and cached by the image's id. They are drawn with the same rounded-corner mask as quads (`shaders/image.wgsl`). An image larger than the device allows starts at the first mip level that fits.
- **Text** is rasterized once per glyph, size and subpixel offset (four positions per pixel) with `swash` into an R8 atlas, and drawn as textured quads (`shaders/glyph.wgsl`).
- **Shapes made of curves** (`PathFill`) are cut into straight pieces and drawn first into a coverage atlas, where each pixel gets the exact area its edges cover (`shaders/path_coverage.wgsl`). The main pass then paints them from that atlas (`shaders/path.wgsl`). A frame with more shapes than one page of the atlas holds is split into more passes, still in order.
- **Cables** are simulated by a compute shader (`shaders/cable_sim.wgsl`) in a `GpuCables` world and drawn straight from its buffer (`shaders/cable.wgsl`); only four bytes of movement and the points come back, a frame late.
- **Clips** become scissor rectangles in device pixels, each cut to the one around it. A primitive whose clip is empty is not drawn at all.

Consecutive primitives of the same kind, image and clip share one draw call. Instance buffers are reused and only grow.

| Kept between frames | Rebuilt every frame |
|---|---|
| Pipelines, for the renderer's lifetime | Instance lists and batches |
| The glyph atlas, until it fills | The coverage atlas of curved shapes |
| Image textures, until unused for 240 frames | Scissor rectangles |
| Instance buffers, which only grow | |

## What comes for free

- **Images update by identity.** The texture is keyed by `ImageData::id`, and every new `ImageData` gets a new id. Cloning an `ImageData` is cheap and reuses the texture; new pixels need a new `ImageData`. A texture not drawn for 240 frames is released.
- **The glyph atlas grows by itself.** It starts at 1024×1024 and doubles when full, up to the device limit or 8192. At the limit it is cleared and the frame built again; text that still does not fit in that frame is skipped, with a warning in the log.
- **Newer primitives do not break an older renderer.** A primitive from a newer `ravenrs-core` that this renderer does not know is skipped, and an unknown brush is drawn transparent, rather than drawn wrong.
- **Adapter choice.** `GpuContext::new` asks for the GPU that spends the least power, which is all an interface needs. Without a preference wgpu takes the first adapter it finds, which on Linux can be Vulkan on the CPU while the real GPU is reachable through GL. When the chosen adapter refuses the device, the others are tried, CPU adapters last.
- **Counters.** `render` returns `RenderStats`: quads, glyphs, images, curved shapes, draw calls and why each extra one was needed, bytes uploaded, atlas use and resets. `Renderer::set_gpu_timing(true)` adds how long the GPU took, in `RenderStats::gpu_time`. That answer arrives some frames late, never makes a frame wait, and stays `None` on a GPU that cannot tell the time.

## Per platform

| | Desktop (`ravenrs-platform-native`) | Browser (`ravenrs-platform-web`) |
|---|---|---|
| Backends | Whatever wgpu offers: Metal on macOS, Vulkan or GL on Linux, DX12 or Vulkan on Windows | WebGPU, then WebGL2 when WebGPU is missing or fails to start |
| Environment variables | Read, through `InstanceDescriptor::*_from_env` | Not read; the backend comes from the platform's options |
| Limits | wgpu's defaults | WebGPU's defaults or WebGL2's, raised to the adapter's texture size |
| Device loss | Device, surface and renderer rebuilt on the next frame | Rebuilt with the same backends on the next frame |

The crate has no Cargo features. `render_to_rgba` waits on the GPU with `device.poll`, which a browser does not allow; on the web, read pixels back asynchronously yourself.

## When something goes wrong

`GpuContext::new` returns a `GpuError`, and the platforms hand it to the app inside their own GPU error. `render` itself returns no error.

- **`GpuError::NoAdapter`**: no adapter matched. The usual causes are a missing or old driver, a virtual machine without 3D acceleration, or, in a browser, WebGPU and WebGL2 both unavailable. On Linux, `WGPU_BACKEND=gl` tries OpenGL instead of Vulkan, and adding `LIBGL_ALWAYS_SOFTWARE=1` draws on the CPU: slowly, but it draws.
- **`GpuError::Device`**: adapters were found but every one refused the device, usually because none offers the default limits. The message is the first adapter's refusal. An old GPU may do better with `WGPU_BACKEND=gl`.
- **`GpuError::Surface`**: the window or canvas surface could not be created, or reports no formats. This crate creates no surface itself; the variant is there for the caller, as in the example above.

Variables that change the GPU, when the instance was built with a `_from_env` descriptor as the desktop platform does:

- `WGPU_BACKEND`: a comma list of backends to try, such as `vulkan`, `gl`, `metal`, `dx12`.
- `WGPU_POWER_PREF`: `low` (the default here), `high` for the discrete GPU, or `none`. `GpuContext::new` reads this one itself, whatever the instance.
- `WGPU_ALLOW_UNDERLYING_NONCOMPLIANT_ADAPTER=1`: also accept drivers that are not conformant.
- `WGPU_VALIDATION` and `WGPU_DEBUG`: turn on wgpu's validation and debug labels.

`WGPU_ADAPTER_NAME` is not read: `GpuContext::new` ranks adapters by power preference, not by name. To force one adapter, create the device yourself and pass it to `Renderer::new`.

**Device loss** (a driver reset, a GPU removed, a browser that reclaimed the context) is not reported by `render`. Register `device.set_device_lost_callback`, and when it fires drop the `Renderer`, create a new `GpuContext` and a new `Renderer`, configure the surface on the new device, and draw again. Every texture the renderer held belonged to the old device; images and glyphs are uploaded again from the next scene.

**Rendering on the CPU.** When the adapter in use is a software one, the log says so with a warning; the interface works, slowly. The desktop platform's diagnostics report lists the adapter in use, the alternatives (the non-conformant ones too) and the variables set. That report is what to attach to a bug.

**Logs.** The crate logs through `tracing`, so the app must install a subscriber to see anything: the adapter at `info` when the GPU is ready, a refused device, software rendering and a full glyph atlas at `warn`, atlas growth at `debug`, and a `ravenrs::render` span per frame at `trace`. With `tracing-subscriber`, `RUST_LOG=ravenrs_render=debug` shows them.

**Wrong pixels.** The golden-image tests compare scenes drawn on your GPU with references, allowing 24/255 per channel on at most 0.4% of pixels, and a failing run writes `*.actual.png` beside the reference. In a bug report, send the scene (`Scene` prints with `Debug`), the size and scale passed to `render`, the surface format, and the adapter name, backend and driver from the diagnostics report.

## License

MIT OR Apache-2.0.
