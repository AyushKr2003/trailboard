//! GPU compositor for the board: a dot grid, then z-ordered layers. Each layer
//! draws its quads (cards, shadows, page textures, edges) and then its text,
//! so a card's title is covered by cards stacked above it.

use std::collections::HashMap;
use std::sync::Arc;

use glyphon::{
    Attrs, Buffer, Cache, Color as TextColor, ColorMode, Family, FontSystem, Metrics, Resolution, Shaping,
    SwashCache, TextArea, TextAtlas, TextBounds, TextRenderer, Viewport, Weight,
};
use scene::Rect;
use wgpu::util::DeviceExt;
use winit::window::Window;

const FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Bgra8Unorm;

pub type Rgba = [f32; 4];

pub fn rgba(r: u8, g: u8, b: u8, a: f32) -> Rgba {
    [r as f32 / 255.0, g as f32 / 255.0, b as f32 / 255.0, a]
}

/// Background parameters for one frame.
#[derive(Clone, Copy)]
pub struct Background {
    /// The board's colour at its top left; it eases into `color2` towards
    /// the bottom right (the same colour twice: flat).
    pub color: Rgba,
    pub color2: Rgba,
    /// Fine noise over the colour, in 8-bit levels either way (0: none);
    /// it keeps a soft gradient from banding.
    pub grain: f32,
    pub dot: Rgba,
    /// Screen position of world origin, px.
    pub grid_origin: [f32; 2],
    /// Distance between dots, px (dots hidden below ~4px).
    pub grid_spacing: f32,
    pub dot_radius: f32,
    /// Opacity (0–1) of the dots halfway between the main ones: they fade in
    /// as the board zooms in, so the grid never jumps from one density to the next.
    pub fine: f32,
}

#[derive(Clone, Copy, Debug)]
pub enum Shape {
    /// Rounded rectangle in screen px.
    Rect { rect: Rect, radius: f32 },
    /// Rectangle with only its bottom corners rounded (page content under a title bar).
    RoundBottom { rect: Rect, radius: f32 },
    /// Soft shadow under a rounded rectangle.
    Shadow { rect: Rect, radius: f32, blur: f32 },
    /// The outline of a rounded rectangle, `width` px wide, drawn inside `rect`.
    Ring { rect: Rect, radius: f32, width: f32 },
    /// Thick line segment in screen px.
    Segment { from: [f32; 2], to: [f32; 2], width: f32 },
    /// A page texture shown at its own size from the top-left: `extent` is
    /// how many of the texture's widths and heights the rect spans (1, 1
    /// fills it exactly, like `Rect`). Past the texture (extent > 1) the
    /// page's left-edge colour of that row fills in; below 1 it is cropped.
    /// `square`: the corners left square, a mask of [`corners`] (0: all round).
    Page { rect: Rect, radius: f32, square: u8, extent: [f32; 2] },
}

/// Corners of a [`Shape::Page`], to leave square.
pub mod corners {
    pub const TOP_LEFT: u8 = 1;
    pub const TOP_RIGHT: u8 = 2;
    pub const BOTTOM_RIGHT: u8 = 4;
    pub const BOTTOM_LEFT: u8 = 8;
    pub const TOP: u8 = TOP_LEFT | TOP_RIGHT;
    pub const BOTTOM: u8 = BOTTOM_LEFT | BOTTOM_RIGHT;
    pub const LEFT: u8 = TOP_LEFT | BOTTOM_LEFT;
    pub const RIGHT: u8 = TOP_RIGHT | BOTTOM_RIGHT;
}

#[derive(Clone, Copy, Debug)]
pub struct Quad {
    pub shape: Shape,
    pub color: Rgba,
    /// Page texture to fill a `Rect` with (multiplied by `color`).
    pub texture: Option<u64>,
}

/// Shaping width for a single line: wide enough that nothing wraps.
const LINE: f32 = 100_000.0;

#[derive(Clone, Debug)]
pub struct Text {
    pub text: String,
    /// Top-left, screen px.
    pub pos: [f32; 2],
    pub size: f32,
    pub color: Rgba,
    /// Clip rectangle, screen px.
    pub clip: Rect,
    pub bold: bool,
}

#[derive(Default, Clone)]
pub struct Layer {
    pub quads: Vec<Quad>,
    pub texts: Vec<Text>,
}

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct Instance {
    a: [f32; 4],
    color: [f32; 4],
    params: [f32; 4],
    /// xy: texture extent (see `Shape::Page`); 1, 1 for everything else.
    /// z: corners left square (`corners` mask), 0 for everything else.
    uv: [f32; 4],
}

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct Globals {
    viewport: [f32; 2],
    grid_origin: [f32; 2],
    grid_spacing: f32,
    dot_radius: f32,
    fine: f32,
    grain: f32,
    bg: [f32; 4],
    dot: [f32; 4],
    bg2: [f32; 4],
}

struct PageTexture {
    texture: wgpu::Texture,
    bind_group: wgpu::BindGroup,
    size: (u32, u32),
    serial: u64,
}

type TextKey = (String, u32, u32, bool);

pub struct Renderer {
    window: Arc<Window>,
    surface: wgpu::Surface<'static>,
    device: wgpu::Device,
    queue: wgpu::Queue,
    config: wgpu::SurfaceConfiguration,
    bg_pipeline: wgpu::RenderPipeline,
    quad_pipeline: wgpu::RenderPipeline,
    globals_buf: wgpu::Buffer,
    globals_bg: wgpu::BindGroup,
    tex_layout: wgpu::BindGroupLayout,
    sampler: wgpu::Sampler,
    white: wgpu::BindGroup,
    pages: HashMap<u64, PageTexture>,
    font_system: FontSystem,
    swash: SwashCache,
    text_viewport: Viewport,
    atlas: TextAtlas,
    text_layers: Vec<TextRenderer>,
    text_cache: HashMap<TextKey, Buffer>,
    /// Fingerprint of the last frame presented ([`frame_key`]); `None` forces
    /// the next one.
    last_frame: Option<u64>,
    /// Present every frame, changed or not (`TRAILBOARD_REDRAW=always`).
    pub always_present: bool,
    /// Frames presented and frames skipped as unchanged, since the start.
    pub presented: u64,
    pub skipped: u64,
}

impl Renderer {
    pub async fn new(
        window: Arc<Window>,
        display: Box<dyn wgpu::wgt::WgpuHasDisplayHandle>,
    ) -> Result<Self, String> {
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
            backends: wgpu::Backends::VULKAN | wgpu::Backends::GL,
            ..wgpu::InstanceDescriptor::new_with_display_handle(display)
        });
        let surface = instance.create_surface(window.clone()).map_err(|e| e.to_string())?;
        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                compatible_surface: Some(&surface),
                ..Default::default()
            })
            .await
            .map_err(|e| e.to_string())?;
        log::info!("GPU adapter: {:?}", adapter.get_info());
        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor::default())
            .await
            .map_err(|e| e.to_string())?;

        let size = window.inner_size();
        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format: FORMAT,
            width: size.width.max(1),
            height: size.height.max(1),
            present_mode: wgpu::PresentMode::AutoVsync,
            alpha_mode: wgpu::CompositeAlphaMode::Auto,
            view_formats: vec![],
            desired_maximum_frame_latency: 2,
            color_space: wgpu::SurfaceColorSpace::Auto,
        };
        surface.configure(&device, &config);

        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("board"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shader.wgsl").into()),
        });

        let globals_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("globals"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
        });
        let tex_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("page texture"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        multisampled: false,
                        view_dimension: wgpu::TextureViewDimension::D2,
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });
        let globals_buf = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("globals"),
            size: std::mem::size_of::<Globals>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let globals_bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("globals"),
            layout: &globals_layout,
            entries: &[wgpu::BindGroupEntry { binding: 0, resource: globals_buf.as_entire_binding() }],
        });
        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("board"),
            bind_group_layouts: &[Some(&globals_layout), Some(&tex_layout)],
            immediate_size: 0,
        });
        let target = [Some(wgpu::ColorTargetState {
            format: FORMAT,
            blend: Some(wgpu::BlendState::PREMULTIPLIED_ALPHA_BLENDING),
            write_mask: wgpu::ColorWrites::ALL,
        })];
        let bg_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("background"),
            layout: Some(&layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_bg"),
                buffers: &[],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_bg"),
                targets: &target,
                compilation_options: Default::default(),
            }),
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview_mask: None,
            cache: None,
        });
        const ATTRS: [wgpu::VertexAttribute; 4] =
            wgpu::vertex_attr_array![0 => Float32x4, 1 => Float32x4, 2 => Float32x4, 3 => Float32x4];
        let quad_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("quads"),
            layout: Some(&layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_quad"),
                buffers: &[Some(wgpu::VertexBufferLayout {
                    array_stride: std::mem::size_of::<Instance>() as u64,
                    step_mode: wgpu::VertexStepMode::Instance,
                    attributes: &ATTRS,
                })],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_quad"),
                targets: &target,
                compilation_options: Default::default(),
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleStrip,
                ..Default::default()
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview_mask: None,
            cache: None,
        });

        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("page"),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });
        let white_tex = device.create_texture_with_data(
            &queue,
            &wgpu::TextureDescriptor {
                label: Some("white"),
                size: wgpu::Extent3d { width: 1, height: 1, depth_or_array_layers: 1 },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: FORMAT,
                usage: wgpu::TextureUsages::TEXTURE_BINDING,
                view_formats: &[],
            },
            wgpu::util::TextureDataOrder::LayerMajor,
            &[255, 255, 255, 255],
        );
        let white = make_tex_bind_group(&device, &tex_layout, &white_tex, &sampler);

        // The interface is set in Inter (bundled, SIL OFL), so it looks the
        // same on every desktop; other scripts fall back to the system's fonts.
        let mut font_system = FontSystem::new();
        for face in [&include_bytes!("../fonts/Inter-Regular.ttf")[..], &include_bytes!("../fonts/Inter-SemiBold.ttf")[..]] {
            font_system.db_mut().load_font_data(face.to_vec());
        }
        let swash = SwashCache::new();
        let cache = Cache::new(&device);
        let text_viewport = Viewport::new(&device, &cache);
        // The surface is a non-sRGB format holding sRGB values (like our quads), so text
        // colours must not be converted to linear first (`Accurate` darkens mid-tones).
        let atlas = TextAtlas::with_color_mode(&device, &queue, &cache, FORMAT, ColorMode::Web);

        Ok(Self {
            window,
            surface,
            device,
            queue,
            config,
            bg_pipeline,
            quad_pipeline,
            globals_buf,
            globals_bg,
            tex_layout,
            sampler,
            white,
            pages: HashMap::new(),
            font_system,
            swash,
            text_viewport,
            atlas,
            text_layers: Vec::new(),
            text_cache: HashMap::new(),
            last_frame: None,
            always_present: false,
            presented: 0,
            skipped: 0,
        })
    }

    pub fn window(&self) -> &Arc<Window> {
        &self.window
    }

    pub fn resize(&mut self, width: u32, height: u32) {
        if width > 0 && height > 0 {
            self.config.width = width;
            self.config.height = height;
            self.surface.configure(&self.device, &self.config);
        }
        self.last_frame = None;
    }

    /// Present the next frame even if it looks the same (the window was
    /// exposed, say).
    pub fn invalidate(&mut self) {
        self.last_frame = None;
    }

    pub fn viewport(&self) -> (f32, f32) {
        (self.config.width as f32, self.config.height as f32)
    }

    /// Upload a page frame (BGRA8) if it changed since the last upload.
    pub fn update_page(&mut self, key: u64, width: u32, height: u32, pixels: &[u8], serial: u64) {
        if width == 0 || height == 0 || pixels.len() < (width * height * 4) as usize {
            return;
        }
        let needs_new = self.pages.get(&key).is_none_or(|p| p.size != (width, height));
        if needs_new {
            let texture = self.device.create_texture(&wgpu::TextureDescriptor {
                label: Some("page"),
                size: wgpu::Extent3d { width, height, depth_or_array_layers: 1 },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: FORMAT,
                usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
                view_formats: &[],
            });
            let bind_group = make_tex_bind_group(&self.device, &self.tex_layout, &texture, &self.sampler);
            self.pages.insert(key, PageTexture { texture, bind_group, size: (width, height), serial: u64::MAX });
        }
        let page = self.pages.get_mut(&key).expect("inserted above");
        if page.serial == serial {
            return;
        }
        page.serial = serial;
        self.queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &page.texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            pixels,
            wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(4 * width), rows_per_image: Some(height) },
            wgpu::Extent3d { width, height, depth_or_array_layers: 1 },
        );
    }

    pub fn has_page(&self, key: u64) -> bool {
        self.pages.contains_key(&key)
    }

    /// Textures held by key (pages, and thumbnails and icons drawn as pages) and
    /// their size in bytes (RGBA, 4 bytes a pixel).
    pub fn page_texture_bytes(&self) -> (usize, u64) {
        let bytes = self.pages.values().map(|p| p.size.0 as u64 * p.size.1 as u64 * 4).sum();
        (self.pages.len(), bytes)
    }

    pub fn remove_page(&mut self, key: u64) {
        self.pages.remove(&key);
    }

    /// Width in px of `text` at `size` (single line), for layout.
    pub fn measure(&mut self, text: &str, size: f32, bold: bool) -> f32 {
        let key = self.shape(text, size, LINE, bold);
        self.text_cache[&key].layout_runs().map(|r| r.line_w).fold(0.0, f32::max)
    }

    /// `text` cut with "…" to fit `max_w` px at `size`, or as is if it fits.
    /// Every text is drawn this way, so a label never wraps or stops mid-letter.
    pub fn ellipsize(&mut self, text: &str, size: f32, bold: bool, max_w: f32) -> String {
        // Half a pixel of slack: rounding mustn't cut a label that fits.
        let max_w = max_w + 0.5;
        if self.measure(text, size, bold) <= max_w {
            return text.to_string();
        }
        let chars: Vec<char> = text.chars().collect();
        let cut = |n: usize| format!("{}…", chars[..n].iter().collect::<String>().trim_end());
        let (mut lo, mut hi) = (0, chars.len());
        while lo < hi {
            let mid = (lo + hi).div_ceil(2);
            if self.measure(&cut(mid), size, bold) <= max_w { lo = mid } else { hi = mid - 1 }
        }
        if lo == 0 { String::new() } else { cut(lo) }
    }

    /// Shapes `text` into the cache (if not already there) and returns its key.
    fn shape(&mut self, text: &str, size: f32, width: f32, bold: bool) -> TextKey {
        let key: TextKey = (text.to_string(), size.to_bits(), width.to_bits(), bold);
        if !self.text_cache.contains_key(&key) {
            let fs = &mut self.font_system;
            let mut b = Buffer::new(fs, Metrics::new(size, size * 1.3));
            b.set_size(Some(width), Some(size * 1.6));
            // Emphasis is Inter's semibold: clear at small sizes without the
            // heaviness of a bold.
            let weight = if bold { Weight::SEMIBOLD } else { Weight::NORMAL };
            b.set_text(text, &Attrs::new().family(Family::Name("Inter")).weight(weight), Shaping::Advanced, None);
            b.shape_until_scroll(fs, false);
            self.text_cache.insert(key.clone(), b);
        }
        key
    }

    /// Draw and present a frame. A frame the same as the last one presented
    /// (same layers, background, size and page pixels) is skipped; returns
    /// whether it was presented.
    pub fn render(&mut self, bg: Background, layers: &[Layer]) -> bool {
        let key = frame_key(self.viewport(), &bg, layers, |k| self.pages.get(&k).map(|p| p.serial));
        if !self.always_present && self.last_frame == Some(key) {
            self.skipped += 1;
            return false;
        }
        // Trim the shaped-text cache between frames only: clearing it while a
        // frame's texts are being shaped dropped buffers that frame still needed.
        if self.text_cache.len() > 1024 {
            self.text_cache.clear();
        }
        let (w, h) = self.viewport();
        let globals = Globals {
            viewport: [w, h],
            grid_origin: bg.grid_origin,
            grid_spacing: bg.grid_spacing,
            dot_radius: bg.dot_radius,
            fine: bg.fine,
            grain: bg.grain,
            bg: bg.color,
            dot: bg.dot,
            bg2: bg.color2,
        };
        self.queue.write_buffer(&self.globals_buf, 0, bytemuck::bytes_of(&globals));

        const FULL: [f32; 4] = [1.0, 1.0, 0.0, 0.0];
        // All instances in one buffer; per-layer batches split by texture.
        let mut instances: Vec<Instance> = Vec::new();
        let mut batches: Vec<Vec<(std::ops::Range<u32>, Option<u64>)>> = Vec::with_capacity(layers.len());
        for layer in layers {
            let mut lb: Vec<(std::ops::Range<u32>, Option<u64>)> = Vec::new();
            for q in &layer.quads {
                let tex = q.texture.filter(|k| self.pages.contains_key(k));
                let inst = match q.shape {
                    Shape::Rect { rect, radius } => Instance {
                        a: [rect.x, rect.y, rect.w, rect.h],
                        color: q.color,
                        params: [0.0, radius, 0.0, if tex.is_some() { 1.0 } else { 0.0 }],
                        uv: FULL,
                    },
                    Shape::RoundBottom { rect, radius } => Instance {
                        a: [rect.x, rect.y, rect.w, rect.h],
                        color: q.color,
                        params: [2.0, radius, 0.0, if tex.is_some() { 1.0 } else { 0.0 }],
                        uv: FULL,
                    },
                    Shape::Page { rect, radius, square, extent } => Instance {
                        a: [rect.x, rect.y, rect.w, rect.h],
                        color: q.color,
                        params: [0.0, radius, 0.0, if tex.is_some() { 1.0 } else { 0.0 }],
                        uv: [extent[0].max(1e-3), extent[1].max(1e-3), square as f32, 0.0],
                    },
                    Shape::Shadow { rect, radius, blur } => Instance {
                        a: [rect.x, rect.y, rect.w, rect.h],
                        color: q.color,
                        params: [0.0, radius, blur.max(0.5), 0.0],
                        uv: FULL,
                    },
                    Shape::Ring { rect, radius, width } => Instance {
                        a: [rect.x, rect.y, rect.w, rect.h],
                        color: q.color,
                        params: [3.0, radius, width.max(0.5), 0.0],
                        uv: FULL,
                    },
                    Shape::Segment { from, to, width } => Instance {
                        a: [from[0], from[1], to[0], to[1]],
                        color: q.color,
                        params: [1.0, width, 0.0, 0.0],
                        uv: FULL,
                    },
                };
                let idx = instances.len() as u32;
                instances.push(inst);
                match lb.last_mut() {
                    Some((range, t)) if *t == tex && range.end == idx => range.end += 1,
                    _ => lb.push((idx..idx + 1, tex)),
                }
            }
            batches.push(lb);
        }
        let instance_buf = (!instances.is_empty()).then(|| {
            self.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("instances"),
                contents: bytemuck::cast_slice(&instances),
                usage: wgpu::BufferUsages::VERTEX,
            })
        });

        // Text: one renderer per layer so text respects layer order.
        self.text_viewport.update(&self.queue, Resolution { width: self.config.width, height: self.config.height });
        while self.text_layers.len() < layers.len() {
            let r = TextRenderer::new(&mut self.atlas, &self.device, wgpu::MultisampleState::default(), None);
            self.text_layers.push(r);
        }
        for (i, layer) in layers.iter().enumerate() {
            // One line each, cut with "…" at the clip's right edge (an input
            // scrolled left has its start before the clip, so it is measured
            // from where the text starts, not from the clip).
            let keys: Vec<TextKey> = (0..layer.texts.len())
                .map(|j| {
                    let t = &layer.texts[j];
                    let room = t.clip.right() - t.pos[0];
                    let text = if room > 0.0 { self.ellipsize(&t.text, t.size, t.bold, room) } else { t.text.clone() };
                    self.shape(&text, t.size, LINE, t.bold)
                })
                .collect();
            let areas: Vec<TextArea> = layer
                .texts
                .iter()
                .zip(&keys)
                .map(|(t, k)| {
                    let c = t.color;
                    TextArea {
                        buffer: &self.text_cache[k],
                        left: t.pos[0],
                        top: t.pos[1],
                        scale: 1.0,
                        bounds: TextBounds {
                            left: t.clip.x.floor() as i32,
                            top: t.clip.y.floor() as i32,
                            right: t.clip.right().ceil() as i32,
                            bottom: t.clip.bottom().ceil() as i32,
                        },
                        default_color: TextColor::rgba(
                            (c[0] * 255.0) as u8,
                            (c[1] * 255.0) as u8,
                            (c[2] * 255.0) as u8,
                            (c[3] * 255.0) as u8,
                        ),
                        custom_glyphs: &[],
                    }
                })
                .collect();
            if let Err(e) = self.text_layers[i].prepare(
                &self.device,
                &self.queue,
                &mut self.font_system,
                &mut self.atlas,
                &self.text_viewport,
                areas,
                &mut self.swash,
            ) {
                log::warn!("text prepare failed: {e}");
            }
        }

        let frame = match self.surface.get_current_texture() {
            wgpu::CurrentSurfaceTexture::Success(f) => f,
            wgpu::CurrentSurfaceTexture::Suboptimal(f) => {
                self.surface.configure(&self.device, &self.config);
                f
            }
            wgpu::CurrentSurfaceTexture::Outdated | wgpu::CurrentSurfaceTexture::Lost => {
                self.surface.configure(&self.device, &self.config);
                self.last_frame = None;
                return false;
            }
            _ => {
                self.last_frame = None;
                return false;
            }
        };
        let view = frame.texture.create_view(&wgpu::TextureViewDescriptor::default());
        let mut enc = self.device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("board") });
        {
            let mut pass = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("board"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations { load: wgpu::LoadOp::Clear(wgpu::Color::BLACK), store: wgpu::StoreOp::Store },
                    depth_slice: None,
                })],
                ..Default::default()
            });
            pass.set_bind_group(0, &self.globals_bg, &[]);
            pass.set_bind_group(1, &self.white, &[]);
            pass.set_pipeline(&self.bg_pipeline);
            pass.draw(0..3, 0..1);

            for (i, lb) in batches.iter().enumerate() {
                if let (Some(buf), false) = (&instance_buf, lb.is_empty()) {
                    pass.set_pipeline(&self.quad_pipeline);
                    pass.set_bind_group(0, &self.globals_bg, &[]);
                    pass.set_vertex_buffer(0, buf.slice(..));
                    for (range, tex) in lb {
                        let group = tex.and_then(|k| self.pages.get(&k)).map_or(&self.white, |p| &p.bind_group);
                        pass.set_bind_group(1, group, &[]);
                        pass.draw(0..4, range.clone());
                    }
                }
                if !layers[i].texts.is_empty()
                    && let Err(e) = self.text_layers[i].render(&self.atlas, &self.text_viewport, &mut pass) {
                        log::warn!("text render failed: {e}");
                    }
            }
        }
        self.queue.submit(Some(enc.finish()));
        self.window.pre_present_notify();
        self.queue.present(frame);
        self.last_frame = Some(key);
        self.presented += 1;
        self.atlas.trim();
        true
    }
}

/// A fingerprint of everything a frame shows: the viewport, the background,
/// every quad and text in order, and the pixels' serial of each page drawn.
fn frame_key(viewport: (f32, f32), bg: &Background, layers: &[Layer], serial: impl Fn(u64) -> Option<u64>) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    let f = |h: &mut std::collections::hash_map::DefaultHasher, xs: &[f32]| {
        for x in xs {
            x.to_bits().hash(h);
        }
    };
    f(&mut h, &[viewport.0, viewport.1, bg.grain, bg.grid_spacing, bg.dot_radius, bg.fine]);
    f(&mut h, &bg.color);
    f(&mut h, &bg.color2);
    f(&mut h, &bg.dot);
    f(&mut h, &bg.grid_origin);
    for layer in layers {
        layer.quads.len().hash(&mut h);
        for q in &layer.quads {
            match q.shape {
                Shape::Rect { rect, radius } => {
                    0u8.hash(&mut h);
                    f(&mut h, &[rect.x, rect.y, rect.w, rect.h, radius]);
                }
                Shape::RoundBottom { rect, radius } => {
                    1u8.hash(&mut h);
                    f(&mut h, &[rect.x, rect.y, rect.w, rect.h, radius]);
                }
                Shape::Shadow { rect, radius, blur } => {
                    2u8.hash(&mut h);
                    f(&mut h, &[rect.x, rect.y, rect.w, rect.h, radius, blur]);
                }
                Shape::Ring { rect, radius, width } => {
                    3u8.hash(&mut h);
                    f(&mut h, &[rect.x, rect.y, rect.w, rect.h, radius, width]);
                }
                Shape::Segment { from, to, width } => {
                    4u8.hash(&mut h);
                    f(&mut h, &[from[0], from[1], to[0], to[1], width]);
                }
                Shape::Page { rect, radius, square, extent } => {
                    5u8.hash(&mut h);
                    square.hash(&mut h);
                    f(&mut h, &[rect.x, rect.y, rect.w, rect.h, radius, extent[0], extent[1]]);
                }
            }
            f(&mut h, &q.color);
            q.texture.hash(&mut h);
            q.texture.map(&serial).hash(&mut h);
        }
        layer.texts.len().hash(&mut h);
        for t in &layer.texts {
            t.text.hash(&mut h);
            t.bold.hash(&mut h);
            f(&mut h, &[t.pos[0], t.pos[1], t.size, t.clip.x, t.clip.y, t.clip.w, t.clip.h]);
            f(&mut h, &t.color);
        }
    }
    h.finish()
}

fn make_tex_bind_group(
    device: &wgpu::Device,
    layout: &wgpu::BindGroupLayout,
    texture: &wgpu::Texture,
    sampler: &wgpu::Sampler,
) -> wgpu::BindGroup {
    let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
    device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("page texture"),
        layout,
        entries: &[
            wgpu::BindGroupEntry { binding: 0, resource: wgpu::BindingResource::TextureView(&view) },
            wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::Sampler(sampler) },
        ],
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bg() -> Background {
        Background {
            color: [0.1, 0.1, 0.1, 1.0],
            color2: [0.1, 0.1, 0.1, 1.0],
            grain: 0.0,
            dot: [0.5, 0.5, 0.5, 1.0],
            grid_origin: [0.0, 0.0],
            grid_spacing: 24.0,
            dot_radius: 1.0,
            fine: 0.0,
        }
    }

    fn layer(x: f32, texture: Option<u64>, text: &str) -> Layer {
        let rect = Rect { x, y: 10.0, w: 100.0, h: 50.0 };
        Layer {
            quads: vec![Quad { shape: Shape::Page { rect, radius: 8.0, square: 0, extent: [1.0, 1.0] }, color: [1.0; 4], texture }],
            texts: vec![Text { text: text.into(), pos: [x, 0.0], size: 13.0, color: [1.0; 4], clip: rect, bold: false }],
        }
    }

    #[test]
    fn frame_key_changes_with_what_is_shown() {
        let serial = |s: u64| move |_k: u64| Some(s);
        let key = |x: f32, text: &str, s: u64| frame_key((800.0, 600.0), &bg(), &[layer(x, Some(7), text)], serial(s));
        let same = key(10.0, "a", 1);
        assert_eq!(same, key(10.0, "a", 1), "the same frame twice");
        assert_ne!(same, key(10.5, "a", 1), "a quad moved half a pixel");
        assert_ne!(same, key(10.0, "b", 1), "a label changed");
        assert_ne!(same, key(10.0, "a", 2), "the page painted");
        assert_ne!(same, frame_key((801.0, 600.0), &bg(), &[layer(10.0, Some(7), "a")], serial(1)), "the window resized");
        let mut dark = bg();
        dark.color = [0.0, 0.0, 0.0, 1.0];
        assert_ne!(same, frame_key((800.0, 600.0), &dark, &[layer(10.0, Some(7), "a")], serial(1)), "the theme changed");
        let two = frame_key((800.0, 600.0), &bg(), &[layer(10.0, Some(7), "a"), Layer::default()], serial(1));
        assert_ne!(same, two, "an empty layer counts");
    }
}
