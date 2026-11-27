// Board compositor: dot-grid background, then layers of instanced quads.
// A quad instance is either a rounded rectangle (optionally textured with a
// page, optionally a soft shadow) or a thick line segment (trail edges).
// Output is premultiplied alpha.

struct Globals {
    viewport: vec2f,
    grid_origin: vec2f,
    grid_spacing: f32,
    dot_radius: f32,
    fine: f32,
    grain: f32,
    bg: vec4f,
    dot: vec4f,
    bg2: vec4f,
};

@group(0) @binding(0) var<uniform> g: Globals;
@group(1) @binding(0) var tex: texture_2d<f32>;
@group(1) @binding(1) var samp: sampler;

// ---- background -----------------------------------------------------------

@vertex
fn vs_bg(@builtin(vertex_index) vi: u32) -> @builtin(position) vec4f {
    let uv = vec2f(f32((vi << 1u) & 2u), f32(vi & 2u));
    return vec4f(uv * 2.0 - 1.0, 0.0, 1.0);
}

// A cheap per-pixel hash in 0..1, for grain.
fn hash2(p: vec2f) -> f32 {
    let q = fract(p * vec2f(0.1031, 0.1030));
    let r = q + dot(q, q.yx + 33.33);
    return fract((r.x + r.y) * r.x);
}

@fragment
fn fs_bg(@builtin(position) pos: vec4f) -> @location(0) vec4f {
    // `bg` at the top left easing into `bg2` at the bottom right (the
    // window's own diagonal, so the mood fills any shape the same way).
    let uv = pos.xy / max(g.viewport, vec2f(1.0));
    let t = smoothstep(0.0, 1.0, clamp(uv.x * 0.45 + uv.y * 0.55, 0.0, 1.0));
    var c = vec4f(mix(g.bg.rgb, g.bg2.rgb, t), 1.0);
    if (g.grain > 0.0) {
        let n = hash2(floor(pos.xy)) + hash2(floor(pos.xy) + 17.0) - 1.0;
        c = vec4f(clamp(c.rgb + vec3f(n * g.grain / 255.0), vec3f(0.0), vec3f(1.0)), 1.0);
    }
    if (g.grid_spacing > 4.0) {
        // Main dots every `grid_spacing`; between them, dots that fade in
        // with `fine` as the board zooms in.
        let p = pos.xy - g.grid_origin;
        let m = p - g.grid_spacing * round(p / g.grid_spacing);
        var a = clamp(g.dot_radius + 0.5 - length(m), 0.0, 1.0);
        if (g.fine > 0.0) {
            let h = g.grid_spacing * 0.5;
            let mf = p - h * round(p / h);
            a = max(a, clamp(g.dot_radius + 0.5 - length(mf), 0.0, 1.0) * g.fine);
        }
        c = vec4f(mix(c.rgb, g.dot.rgb, a * g.dot.a), 1.0);
    }
    return c;
}

// ---- quads ----------------------------------------------------------------

struct Inst {
    // rect: x, y, w, h (px)   segment: x0, y0, x1, y1 (px)
    @location(0) a: vec4f,
    @location(1) color: vec4f,
    // x: kind (0 rect, 1 segment, 2 rect rounded at the bottom only, 3 outline)  y: corner radius / thickness
    // z: softness (shadow blur px; an outline's width)  w: textured (0/1)
    @location(2) params: vec4f,
    // xy: how many texture widths/heights the rect spans (1, 1: exactly fills it)
    // z: corners left square (1 top-left, 2 top-right, 4 bottom-right, 8 bottom-left)
    @location(3) uv: vec4f,
};

struct VOut {
    @builtin(position) pos: vec4f,
    @location(0) local: vec2f,
    @location(1) half: vec2f,
    @location(2) color: vec4f,
    @location(3) params: vec4f,
    @location(4) uv: vec2f,
    @location(5) square: f32,
};

@vertex
fn vs_quad(@builtin(vertex_index) vi: u32, i: Inst) -> VOut {
    let corner = vec2f(f32(vi & 1u), f32((vi >> 1u) & 1u));
    var o: VOut;
    var p: vec2f;
    if (i.params.x < 0.5 || i.params.x > 1.5) {
        let size = i.a.zw;
        let half = size * 0.5;
        let center = i.a.xy + half;
        let ext = half + vec2f(max(i.params.z, 0.0) + 1.0);
        let local = (corner * 2.0 - 1.0) * ext;
        p = center + local;
        o.local = local;
        o.half = half;
        o.uv = (local + half) / max(size, vec2f(1e-3)) * i.uv.xy;
    } else {
        let p0 = i.a.xy;
        let p1 = i.a.zw;
        let d = p1 - p0;
        let len = max(length(d), 1e-3);
        let dir = d / len;
        let n = vec2f(-dir.y, dir.x);
        let t = i.params.y * 0.5 + 1.0;
        let along = corner.x * len;
        let across = (corner.y * 2.0 - 1.0) * t;
        p = p0 + dir * along + n * across;
        o.local = vec2f(along - len * 0.5, across);
        o.half = vec2f(len * 0.5, i.params.y * 0.5);
        o.uv = vec2f(0.0);
    }
    o.pos = vec4f(p.x / g.viewport.x * 2.0 - 1.0, 1.0 - p.y / g.viewport.y * 2.0, 0.0, 1.0);
    o.color = i.color;
    o.params = i.params;
    o.square = i.uv.z;
    return o;
}

// The inside of an opaque white page, clear of its corners and edge
// (`damage::page_plain`): its texels as they are, drawn without blending.
// What `fs_quad` gives there, for less work a pixel on a software renderer.
@fragment
fn fs_page(v: VOut) -> @location(0) vec4f {
    return textureSampleLevel(tex, samp, v.uv, 0.0);
}

fn sd_round_rect(p: vec2f, half: vec2f, r: f32) -> f32 {
    let q = abs(p) - half + vec2f(r);
    return length(max(q, vec2f(0.0))) + min(max(q.x, q.y), 0.0) - r;
}

@fragment
fn fs_quad(v: VOut) -> @location(0) vec4f {
    // Sample unconditionally (uniform control flow); untextured quads bind a 1x1 white texture.
    // Past the texture (a page painted smaller than its rect), the row's
    // left-edge colour fills in, usually the page's background.
    var st = clamp(v.uv, vec2f(0.0), vec2f(1.0));
    if (v.uv.x > 1.0 || v.uv.y > 1.0) {
        st = vec2f(0.0, st.y);
    }
    let t = textureSampleLevel(tex, samp, st, 0.0);
    var c = v.color;
    var a = 1.0;
    if (v.params.x < 0.5 || v.params.x > 1.5) {
        var r = min(v.params.y, min(v.half.x, v.half.y));
        // Kind 2: square top corners (local.y < 0 is the top half).
        if (v.params.x > 1.5 && v.params.x < 2.5 && v.local.y < 0.0) {
            r = 0.0;
        }
        // Corners left square, by which quarter this is.
        let left = v.local.x < 0.0;
        let corner = select(select(4u, 8u, left), select(2u, 1u, left), v.local.y < 0.0);
        if ((u32(v.square + 0.5) & corner) != 0u) {
            r = 0.0;
        }
        let d = sd_round_rect(v.local, v.half, r);
        let soft = v.params.z;
        if (v.params.x > 2.5) {
            // An outline: inside the shape, within `soft` px of its edge.
            a = clamp(0.5 - d, 0.0, 1.0) * clamp(d + soft + 0.5, 0.0, 1.0);
        } else if (soft > 0.0) {
            a = 1.0 - smoothstep(-soft, soft, d);
        } else {
            a = clamp(0.5 - d, 0.0, 1.0);
        }
        if (v.params.w > 0.5) {
            // Straight alpha: pages are opaque, icons are not.
            c = vec4f(t.rgb * c.rgb, c.a * t.a);
        }
    } else {
        a = clamp(0.5 - (abs(v.local.y) - v.half.y), 0.0, 1.0);
    }
    let alpha = c.a * a;
    return vec4f(c.rgb * alpha, alpha);
}
