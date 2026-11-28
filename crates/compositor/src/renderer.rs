//! The wgpu renderer: pipelines, page textures, text, and drawing a frame
//! (partial where it can).

use std::collections::HashMap;
use std::sync::Arc;

use glyphon::{
    Attrs, Buffer, Cache, Color as TextColor, ColorMode, Family, FontSystem, Metrics, Resolution, Shaping,
    SwashCache, TextArea, TextAtlas, TextBounds, TextRenderer, Viewport, Weight,
};
use wgpu::util::DeviceExt;
use winit::window::Window;

use crate::{clamp_px, cover, cut_px, frame_key, page_copy_from, page_plain, scissor_px, screen_damage, union_px, Background, Layer, Shape, LINE};

const FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Bgra8Unorm;

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub(crate) struct Instance {
    a: [f32; 4],
    color: [f32; 4],
    params: [f32; 4],
    /// xy: texture extent (see `Shape::Page`); 1, 1 for everything else.
    /// z: corners left square (`corners` mask), 0 for everything else.
    uv: [f32; 4],
}

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub(crate) struct Globals {
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

pub(crate) struct PageTexture {
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
    /// `fs_page`, unblended: the inside of an opaque page ([`page_plain`]).
    page_pipeline: wgpu::RenderPipeline,
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
    /// Screen pixels filled by copying a page in rather than shading it
    /// ([`page_copy_from`]), since the start.
    pub copied_px: u64,
    /// Screen pixels of page insides drawn plainly ([`page_plain`]), since
    /// the start.
    pub plain_px: u64,
    /// The last frame presented was laid out differently from the one shown
    /// before it (not only page pixels changed): something may be moving.
    pub relaid: bool,
    /// Of `presented`, the frames redrawn only where page pixels changed.
    pub partial: u64,
    /// Page pixel bytes handed to the GPU, and the time spent in
    /// [`Renderer::render`] for presented frames (waiting for the surface
    /// included), since the start.
    pub uploaded: u64,
    pub render_time: std::time::Duration,
    /// The surface takes copies, so frames are drawn on [`Renderer::canvas`]
    /// and copied over; without it every frame is drawn whole, as before.
    can_copy: bool,
    /// The last frame drawn, kept so a frame where only page pixels changed
    /// (a caret blinking, a spinner) redraws just those pixels.
    canvas: Option<wgpu::Texture>,
    /// [`frame_key`] of the last frame without the pages' serials: the same
    /// value means only page pixels can differ. `None` while the canvas
    /// doesn't hold the last frame.
    last_layout: Option<u64>,
    /// The same, of the last frame presented, on the canvas or not: when a
    /// frame keeps it (only page pixels changed, a video playing), the
    /// canvas is worth keeping even in motion, so later frames can be partial.
    shown_layout: Option<u64>,
    /// When the last frame was presented: frames in quick succession (video,
    /// panning) are drawn straight to the screen, skipping the canvas copy.
    last_present: Option<std::time::Instant>,
    /// Pixels of each page changed since the last frame presented (texture
    /// px: x, y, w, h); `None` for the whole page.
    page_damage: HashMap<u64, Option<[u32; 4]>>,
}

/// Damaged area above which a frame is drawn whole. A scissored redraw costs
/// a copy of the whole canvas, cheap next to drawing the board around a
/// large playing video (M16 step 6e: a card over half the window drew it all
/// every frame).
pub(crate) const PARTIAL_MAX: f32 = 0.9;
/// Smallest part of a frame (px²) worth drawing from an opaque page up
/// rather than from the background (see [`cover`]).
pub(crate) const COVER_MIN: u32 = 64 * 64;
/// Frames closer together than this are motion: drawn straight to the screen
/// (the canvas copy would cost every frame), so the canvas waits for a pause.
pub(crate) const MOTION_GAP: std::time::Duration = std::time::Duration::from_millis(100);

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
        let can_copy = surface.get_capabilities(&adapter).usages.contains(wgpu::TextureUsages::COPY_DST);
        log::info!("partial redraw: {}", if can_copy { "on" } else { "off (the surface takes no copies)" });
        let config = wgpu::SurfaceConfiguration {
            usage: if can_copy {
                wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_DST
            } else {
                wgpu::TextureUsages::RENDER_ATTACHMENT
            },
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
        // The inside of an opaque page: written as is, no blending.
        let opaque = [Some(wgpu::ColorTargetState { format: FORMAT, blend: None, write_mask: wgpu::ColorWrites::ALL })];
        let page_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("page insides"),
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
                entry_point: Some("fs_page"),
                targets: &opaque,
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
            page_pipeline,
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
            copied_px: 0,
            plain_px: 0,
            relaid: true,
            partial: 0,
            uploaded: 0,
            render_time: std::time::Duration::ZERO,
            can_copy,
            canvas: None,
            last_layout: None,
            shown_layout: None,
            last_present: None,
            page_damage: HashMap::new(),
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
        self.last_layout = None;
    }

    /// Present the next frame even if it looks the same (the window was
    /// exposed, say).
    pub fn invalidate(&mut self) {
        self.last_frame = None;
        self.last_layout = None;
    }

    pub fn viewport(&self) -> (f32, f32) {
        (self.config.width as f32, self.config.height as f32)
    }

    /// Upload a page frame (BGRA8) if it changed since the last upload.
    pub fn update_page(&mut self, key: u64, width: u32, height: u32, pixels: &[u8], serial: u64) {
        self.update_page_part(key, width, height, pixels, serial, None);
    }

    /// The serial of the pixels uploaded for `key`, if any.
    pub fn page_serial(&self, key: u64) -> Option<u64> {
        self.pages.get(&key).map(|p| p.serial)
    }

    /// [`Renderer::update_page`], told which pixels changed since the serial
    /// last uploaded ([`Renderer::page_serial`]): `changed` (x, y, w, h in
    /// `pixels`), or `None` for all of them. Only those are uploaded and
    /// redrawn.
    pub fn update_page_part(&mut self, key: u64, width: u32, height: u32, pixels: &[u8], serial: u64, changed: Option<[u32; 4]>) {
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
                usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST | wgpu::TextureUsages::COPY_SRC,
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
        // A new texture holds nothing yet: all of it.
        let [x, y, w, h] = match changed.filter(|_| !needs_new).map(|r| clamp_px(r, width, height)) {
            Some([_, _, 0, _] | [_, _, _, 0]) => return, // a new serial, the same pixels
            Some(r) => r,
            None => [0, 0, width, height],
        };
        self.queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &page.texture,
                mip_level: 0,
                origin: wgpu::Origin3d { x, y, z: 0 },
                aspect: wgpu::TextureAspect::All,
            },
            pixels,
            wgpu::TexelCopyBufferLayout {
                offset: (y as u64 * width as u64 + x as u64) * 4,
                bytes_per_row: Some(4 * width),
                rows_per_image: Some(height),
            },
            wgpu::Extent3d { width: w, height: h, depth_or_array_layers: 1 },
        );
        self.uploaded += w as u64 * h as u64 * 4;
        // A new texture (a resize) may show anywhere: the whole frame. All of
        // an old one changing (a video filling its page) is still only where
        // the page shows.
        let entry = self.page_damage.entry(key).or_insert(Some([x, y, 0, 0]));
        *entry = match (*entry, needs_new) {
            (Some(old), false) => Some(union_px(old, [x, y, w, h])),
            _ => None,
        };
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
        let started = std::time::Instant::now();
        let key = frame_key(self.viewport(), &bg, layers, |k| self.pages.get(&k).map(|p| p.serial));
        if !self.always_present && self.last_frame == Some(key) {
            self.skipped += 1;
            return false;
        }
        let layout = frame_key(self.viewport(), &bg, layers, |k| self.pages.get(&k).map(|_| 0));
        let damage = std::mem::take(&mut self.page_damage);
        // Only page pixels changed since the frame shown: where they show
        // (`None`: too much to tell, a whole page).
        let steady = self.shown_layout == Some(layout);
        let changed = steady.then(|| screen_damage(layers, &damage, |k| self.pages.get(&k).map(|p| p.size), self.viewport()));
        let max_area = PARTIAL_MAX * self.viewport().0 * self.viewport().1;
        // Redraw just there, on the canvas, when it is small enough.
        let scissor = match changed {
            Some(Some(r)) if self.can_copy && !self.always_present && self.last_layout == Some(layout) => {
                if r.w <= 0.0 || r.h <= 0.0 {
                    // What changed isn't on screen.
                    self.last_frame = Some(key);
                    self.skipped += 1;
                    return false;
                }
                (r.w * r.h <= max_area).then_some(r)
            }
            _ => None,
        };
        // Only pages changed, but too much of the frame to draw partly (a
        // video filling the window): the next frame won't be partial
        // either, so the canvas isn't kept for it.
        let big = matches!(changed, Some(None)) || matches!(changed, Some(Some(r)) if r.w * r.h > max_area);
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
                self.last_layout = None;
                return false;
            }
            _ => {
                self.last_frame = None;
                self.last_layout = None;
                return false;
            }
        };
        let size = wgpu::Extent3d { width: self.config.width, height: self.config.height, depth_or_array_layers: 1 };
        let now = std::time::Instant::now();
        let moving = self.last_present.is_some_and(|t| now - t < MOTION_GAP);
        // The canvas is kept up to date at rest, when drawing on it partly,
        // or when only page pixels changed (so the next frame can be partial).
        let use_canvas = self.can_copy && (scissor.is_some() || !moving || (steady && !big));
        let fresh = use_canvas && self.canvas.as_ref().is_none_or(|c| c.size() != size);
        if fresh {
            self.canvas = Some(self.device.create_texture(&wgpu::TextureDescriptor {
                label: Some("canvas"),
                size,
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: FORMAT,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC | wgpu::TextureUsages::COPY_DST,
                view_formats: &[],
            }));
        }
        // A new canvas holds nothing: draw it whole.
        let scissor = scissor.filter(|_| !fresh);
        let target = match (&self.canvas, use_canvas) {
            (Some(c), true) => c,
            _ => &frame.texture,
        };
        let view = target.create_view(&wgpu::TextureViewDescriptor::default());
        let mut enc = self.device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("board") });
        let area = match scissor {
            Some(r) => scissor_px(r, size.width, size.height),
            None => [0, 0, size.width, size.height],
        };
        // Under an opaque page nothing drawn before it shows: there the
        // frame starts at that page, and only the rest of `area` gets the
        // background, shadows and cards beneath.
        let covered = cover(layers, area);
        // That page shown 1:1 is copied there rather than shaded (the
        // target takes copies: the canvas always, the screen when
        // `can_copy`); the pass then starts above it.
        let copied = covered.and_then(|(_, inst, hole)| {
            let q = layers.iter().flat_map(|l| &l.quads).nth(inst as usize)?;
            let page = self.pages.get(&q.texture?)?;
            let from = page_copy_from(&q.shape, q.color, page.size, hole)?;
            let to_target = use_canvas || self.can_copy;
            to_target.then(|| {
                enc.copy_texture_to_texture(
                    wgpu::TexelCopyTextureInfo { texture: &page.texture, mip_level: 0, origin: wgpu::Origin3d { x: from[0], y: from[1], z: 0 }, aspect: wgpu::TextureAspect::All },
                    wgpu::TexelCopyTextureInfo { texture: target, mip_level: 0, origin: wgpu::Origin3d { x: hole[0], y: hole[1], z: 0 }, aspect: wgpu::TextureAspect::All },
                    wgpu::Extent3d { width: hole[2], height: hole[3], depth_or_array_layers: 1 },
                );
            })
        });
        let hole_px = covered.map_or(0, |(_, _, h)| h[2] as u64 * h[3] as u64);
        self.copied_px += copied.map_or(0, |()| hole_px);
        // Else that page's inside, if plain, is drawn by `fs_page`.
        let plain = covered.filter(|_| copied.is_none()).and_then(|(_, inst, _)| {
            let q = layers.iter().flat_map(|l| &l.quads).nth(inst as usize)?;
            let page = self.pages.get(&q.texture?)?;
            page_plain(&q.shape, q.color).then_some((&page.bind_group, inst))
        });
        self.plain_px += plain.map_or(0, |_| hole_px);
        {
            // Partly: the last frame stays and everything is drawn again
            // inside the scissor, the background first (opaque), as a whole
            // frame would be. With a page copied in, the frame keeps it: the
            // rest of it is drawn over opaquely all the same.
            let load = if scissor.is_some() || copied.is_some() { wgpu::LoadOp::Load } else { wgpu::LoadOp::Clear(wgpu::Color::BLACK) };
            let mut pass = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("board"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations { load, store: wgpu::StoreOp::Store },
                    depth_slice: None,
                })],
                ..Default::default()
            });
            let pieces = match covered {
                Some((layer, inst, hole)) => {
                    let mut v: Vec<_> = cut_px(area, hole).into_iter().map(|r| (r, None)).collect();
                    // A copied page is there already: draw from above it.
                    v.push((hole, Some((layer, inst + u32::from(copied.is_some() || plain.is_some())))));
                    v
                }
                None => vec![(area, None)],
            };
            for ([x, y, w, h], from) in pieces {
                if w == 0 || h == 0 {
                    continue;
                }
                pass.set_scissor_rect(x, y, w, h);
                let (first_layer, first_inst) = from.unwrap_or((0, 0));
                // The inside of a page that wasn't copied, drawn plainly.
                if let (Some(_), Some((group, inst)), Some(buf)) = (from, plain, &instance_buf) {
                    pass.set_pipeline(&self.page_pipeline);
                    pass.set_bind_group(0, &self.globals_bg, &[]);
                    pass.set_bind_group(1, group, &[]);
                    pass.set_vertex_buffer(0, buf.slice(..));
                    pass.draw(0..4, inst..inst + 1);
                }
                if from.is_none() {
                    pass.set_bind_group(0, &self.globals_bg, &[]);
                    pass.set_bind_group(1, &self.white, &[]);
                    pass.set_pipeline(&self.bg_pipeline);
                    pass.draw(0..3, 0..1);
                }
                for (i, lb) in batches.iter().enumerate().skip(first_layer) {
                    if let (Some(buf), false) = (&instance_buf, lb.is_empty()) {
                        pass.set_pipeline(&self.quad_pipeline);
                        pass.set_bind_group(0, &self.globals_bg, &[]);
                        pass.set_vertex_buffer(0, buf.slice(..));
                        for (range, tex) in lb {
                            let range = range.start.max(first_inst)..range.end;
                            if range.is_empty() {
                                continue;
                            }
                            let group = tex.and_then(|k| self.pages.get(&k)).map_or(&self.white, |p| &p.bind_group);
                            pass.set_bind_group(1, group, &[]);
                            pass.draw(0..4, range);
                        }
                    }
                    if !layers[i].texts.is_empty()
                        && let Err(e) = self.text_layers[i].render(&self.atlas, &self.text_viewport, &mut pass) {
                            log::warn!("text render failed: {e}");
                        }
                }
            }
        }
        if let (Some(c), true) = (&self.canvas, use_canvas) {
            enc.copy_texture_to_texture(c.as_image_copy(), frame.texture.as_image_copy(), size);
        }
        self.queue.submit(Some(enc.finish()));
        self.window.pre_present_notify();
        self.queue.present(frame);
        self.last_frame = Some(key);
        self.last_layout = use_canvas.then_some(layout);
        self.relaid = self.shown_layout != Some(layout);
        self.shown_layout = Some(layout);
        self.last_present = Some(now);
        self.presented += 1;
        self.partial += u64::from(scissor.is_some());
        self.atlas.trim();
        self.render_time += started.elapsed();
        true
    }
}


pub(crate) fn make_tex_bind_group(
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
    /// wgpu only checks the shader when the window opens: check it here.
    #[test]
    fn the_shader_is_valid_wgsl() {
        let module = naga::front::wgsl::parse_str(include_str!("shader.wgsl")).unwrap_or_else(|e| panic!("{}", e.emit_to_string(include_str!("shader.wgsl"))));
        naga::valid::Validator::new(naga::valid::ValidationFlags::all(), naga::valid::Capabilities::empty())
            .validate(&module)
            .unwrap_or_else(|e| panic!("{e:?}"));
        for entry in ["vs_bg", "fs_bg", "vs_quad", "fs_quad", "fs_page"] {
            assert!(module.entry_points.iter().any(|e| e.name == entry), "{entry}");
        }
    }
}
