struct Vertex {
    @builtin(position) position: vec4f,
    @location(0) uv: vec2f,
}

@group(0) @binding(0) var source: texture_2d<f32>;
@group(0) @binding(1) var source_sampler: sampler;

struct Layer {
    inverse_x: vec4f,
    inverse_y: vec4f,
    inverse_w: vec4f,
    crop: vec4f,
    opacity: f32,
}

@group(1) @binding(0) var<uniform> layer: Layer;

@vertex
fn fullscreen(@builtin(vertex_index) index: u32) -> Vertex {
    let uv = array(vec2f(0.0, 0.0), vec2f(2.0, 0.0), vec2f(0.0, 2.0))[index];
    return Vertex(vec4f(uv * vec2f(2.0, -2.0) + vec2f(-1.0, 1.0), 0.0, 1.0), uv);
}

@fragment
fn sample(vertex: Vertex) -> @location(0) vec4f {
    return textureSample(source, source_sampler, vertex.uv);
}

@fragment
fn premultiply(vertex: Vertex) -> @location(0) vec4f {
    let color = textureSample(source, source_sampler, vertex.uv);
    return vec4f(color.rgb * color.a, color.a);
}

@fragment
fn composite(vertex: Vertex) -> @location(0) vec4f {
    let point = vec3f(vertex.uv, 1.0);
    let w = dot(layer.inverse_w.xyz, point);
    if w <= 0.0 {
        return vec4f(0.0);
    }
    let local = vec2f(dot(layer.inverse_x.xyz, point), dot(layer.inverse_y.xyz, point)) / w;
    if any(local < vec2f(0.0)) || any(local > vec2f(1.0)) {
        return vec4f(0.0);
    }
    return textureSampleLevel(source, source_sampler, layer.crop.xy + local * layer.crop.zw, 0.0) * layer.opacity;
}

@fragment
fn capture(vertex: Vertex) -> @location(0) vec4f {
    let color = textureSample(source, source_sampler, vertex.uv);
    // The input is premultiplied, so its RGB is already composited on black.
    return vec4f(color.rgb, 1.0);
}
