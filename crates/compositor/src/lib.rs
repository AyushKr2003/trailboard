//! GPU compositor for the board: a dot grid, then z-ordered layers. Each layer
//! draws its quads (cards, shadows, page textures, edges) and then its text,
//! so a card's title is covered by cards stacked above it.

use std::collections::HashMap;
use std::sync::Arc;

use glyphon::{
    Attrs, Buffer, Cache, Color as TextColor, Family, FontSystem, Metrics, Resolution, Shaping,
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
    pub color: Rgba,
    pub dot: Rgba,
    /// Screen position of world origin, px.
    pub grid_origin: [f32; 2],
    /// Distance between dots, px (dots hidden below ~4px).
    pub grid_spacing: f32,
    pub dot_radius: f32,
}

#[derive(Clone, Copy, Debug)]
pub enum Shape {
    /// Rounded rectangle in screen px.
    Rect { rect: Rect, radius: f32 },
    /// Rectangle with only its bottom corners rounded (page content under a title bar).
    RoundBottom { rect: Rect, radius: f32 },
    /// Soft shadow under a rounded rectangle.
    Shadow { rect: Rect, radius: f32, blur: f32 },
    /// Thick line segment in screen px.
    Segment { from: [f32; 2], to: [f32; 2], width: f32 },
}

#[derive(Clone, Copy, Debug)]
pub struct Quad {
    pub shape: Shape,
    pub color: Rgba,
    /// Page texture to fill a `Rect` with (multiplied by `color`).
    pub texture: Option<u64>,
}

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

#[derive(Default)]
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
}

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct Globals {
    viewport: [f32; 2],
    grid_origin: [f32; 2],
    grid_spacing: f32,
    dot_radius: f32,
    _pad: [f32; 2],
    bg: [f32; 4],
    dot: [f32; 4],
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
        const ATTRS: [wgpu::VertexAttribute; 3] =
            wgpu::vertex_attr_array![0 => Float32x4, 1 => Float32x4, 2 => Float32x4];
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

        let font_system = FontSystem::new();
        let swash = SwashCache::new();
        let cache = Cache::new(&device);
        let text_viewport = Viewport::new(&device, &cache);
        let atlas = TextAtlas::new(&device, &queue, &cache, FORMAT);

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

    pub fn remove_page(&mut self, key: u64) {
        self.pages.remove(&key);
    }

    /// Width in px of `text` at `size` (single line), for layout.
    pub fn measure(&mut self, text: &str, size: f32, bold: bool) -> f32 {
        let key = self.shape(text, size, 100_000.0, bold);
        self.text_cache[&key].layout_runs().map(|r| r.line_w).fold(0.0, f32::max)
    }

    /// Shapes `text` into the cache (if not already there) and returns its key.
    fn shape(&mut self, text: &str, size: f32, width: f32, bold: bool) -> TextKey {
        let key: TextKey = (text.to_string(), size.to_bits(), width.to_bits(), bold);
        if !self.text_cache.contains_key(&key) {
            if self.text_cache.len() > 512 {
                self.text_cache.clear();
            }
            let fs = &mut self.font_system;
            let mut b = Buffer::new(fs, Metrics::new(size, size * 1.3));
            b.set_size(Some(width), Some(size * 1.6));
            // BOLD, not SEMIBOLD: common sans fonts (e.g. Liberation Sans) ship no semibold
            // face, and the fallback picked a serif face with broken digits and wide spaces.
            let weight = if bold { Weight::BOLD } else { Weight::NORMAL };
            b.set_text(text, &Attrs::new().family(Family::SansSerif).weight(weight), Shaping::Advanced, None);
            b.shape_until_scroll(fs, false);
            self.text_cache.insert(key.clone(), b);
        }
        key
    }

    pub fn render(&mut self, bg: Background, layers: &[Layer]) {
        let (w, h) = self.viewport();
        let globals = Globals {
            viewport: [w, h],
            grid_origin: bg.grid_origin,
            grid_spacing: bg.grid_spacing,
            dot_radius: bg.dot_radius,
            _pad: [0.0; 2],
            bg: bg.color,
            dot: bg.dot,
        };
        self.queue.write_buffer(&self.globals_buf, 0, bytemuck::bytes_of(&globals));

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
                    },
                    Shape::RoundBottom { rect, radius } => Instance {
                        a: [rect.x, rect.y, rect.w, rect.h],
                        color: q.color,
                        params: [2.0, radius, 0.0, if tex.is_some() { 1.0 } else { 0.0 }],
                    },
                    Shape::Shadow { rect, radius, blur } => Instance {
                        a: [rect.x, rect.y, rect.w, rect.h],
                        color: q.color,
                        params: [0.0, radius, blur.max(0.5), 0.0],
                    },
                    Shape::Segment { from, to, width } => Instance {
                        a: [from[0], from[1], to[0], to[1]],
                        color: q.color,
                        params: [1.0, width, 0.0, 0.0],
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
            let keys: Vec<TextKey> = layer
                .texts
                .iter()
                .map(|t| self.shape(&t.text, t.size, t.clip.w.max(1.0).ceil(), t.bold))
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
                return;
            }
            _ => return,
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
        self.atlas.trim();
    }
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
