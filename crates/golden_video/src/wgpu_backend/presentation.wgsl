struct Vertex {
    @builtin(position) position: vec4f,
    @location(0) uv: vec2f,
}

@group(0) @binding(0) var source: texture_2d<f32>;
@group(0) @binding(1) var source_sampler: sampler;

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
fn capture(vertex: Vertex) -> @location(0) vec4f {
    let color = textureSample(source, source_sampler, vertex.uv);
    // The input is premultiplied, so its RGB is already composited on black.
    return vec4f(color.rgb, 1.0);
}
