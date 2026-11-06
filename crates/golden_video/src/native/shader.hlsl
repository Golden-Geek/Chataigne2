Texture2D<float4> source_image : register(t0);
Texture2D<float4> previous_image : register(t1);
ByteAddressBuffer pattern_data : register(t2);
Texture2D<uint4> source_uint_image : register(t3);
Texture2D<int4> source_sint_image : register(t4);

cbuffer VideoParameters : register(b3) {
    uint video_format;
    uint video_target_encoding;
    uint video_target_alpha;
    uint video_reserved;
};

cbuffer Parameters : register(b0) {
    uint2 source_extent;
    uint source_encoding;
    uint source_alpha;
    uint target_encoding;
    uint target_alpha;
    uint filter_linear;
    uint blend_mode;
    float opacity;
    uint has_previous;
    uint source_storage;
    uint source_has_alpha;
    float4 source_rect;
    float4 inverse_row0;
    float4 inverse_row1;
    float4 inverse_row2;
};

cbuffer OverlayParameters : register(b2) {
    float4 overlay_points[4];
    float4 overlay_fill;
    float4 overlay_stroke;
    float4 overlay_style;
    uint4 overlay_kind;
};

static const uint ENCODING_SRGB = 1;
static const uint ALPHA_STRAIGHT = 0;
static const uint ALPHA_PREMULTIPLIED = 1;
static const uint ALPHA_OPAQUE = 2;

struct Vertex {
    float4 position : SV_Position;
    float2 uv : TEXCOORD;
};

Vertex vs_main(uint id : SV_VertexID) {
    // One oversized triangle covers the target without a diagonal interpolation seam.
    float2 uv = float2((id << 1) & 2, id & 2);
    Vertex result;
    result.position = float4(uv * float2(2, -2) + float2(-1, 1), 0, 1);
    result.uv = uv;
    return result;
}

// Preserve stored component values across a device-sharing bridge. Color/alpha policy
// belongs to the eventual renderer sampling; no filtering or transfer occurs here.
float4 ps_import(Vertex input) : SV_Target {
    return source_image.Load(int3(uint2(input.position.xy), 0));
}

uint4 ps_import_uint(Vertex input) : SV_Target {
    return source_uint_image.Load(int3(uint2(input.position.xy), 0));
}

int4 ps_import_sint(Vertex input) : SV_Target {
    return source_sint_image.Load(int3(uint2(input.position.xy), 0));
}

float4 load_stored(int2 position) {
    // Uniform branches keep float draws on their existing view and integer transport typed.
    [branch] if ((source_storage >= 4 && source_storage <= 6) || source_storage == 10) {
        uint4 codes = source_uint_image.Load(int3(position, 0));
        if (source_storage == 10) return float4(codes) / float4(1023, 1023, 1023, 3);
        float maximum = source_storage == 4 ? 255.0 : source_storage == 5 ? 65535.0 : 4294967295.0;
        return float4(codes) / maximum;
    }
    [branch] if (source_storage >= 7 && source_storage <= 9) {
        int4 codes = source_sint_image.Load(int3(position, 0));
        float maximum = source_storage == 7 ? 127.0 : source_storage == 8 ? 32767.0 : 2147483647.0;
        float4 value = max(float4(codes) / maximum, -1.0);
        value.a = saturate(value.a);
        return value;
    }
    return source_image.Load(int3(position, 0));
}

float decode_srgb(float encoded) {
    if (encoded <= 0.04045) {
        return encoded / 12.92;
    }
    return pow((encoded + 0.055) / 1.055, 2.4);
}

float encode_srgb(float value) {
    if (value <= 0.0031308) {
        return value * 12.92;
    }
    return 1.055 * pow(value, 1.0 / 2.4) - 0.055;
}

float3 decode_rgb(float3 encoded) {
    return float3(decode_srgb(encoded.r), decode_srgb(encoded.g), decode_srgb(encoded.b));
}

float3 encode_rgb(float3 value) {
    return float3(encode_srgb(value.r), encode_srgb(value.g), encode_srgb(value.b));
}

float decode_bt709(float value) {
    if (value < 0.081) return value / 4.5;
    precise float x = (value + 0.099) / 1.099;
    // Refine the ninth root before forming x^(20/9). Native pow accuracy on bright
    // NVIDIA samples exceeded the absolute 1e-6 decoder target without this correction.
    precise float root = pow(x, 1.0 / 9.0);
    precise float r2 = root * root;
    precise float r4 = r2 * r2;
    precise float r8 = r4 * r4;
    root -= (root - x / r8) / 9.0;
    precise float power = x * root;
    return power * power;
}

// Packed 4:4:4 defaults: studio-range BT.709. Native metadata contains neither
// colorimetry nor range; alternate assumptions remain an explicit API limitation.
float4 ps_video(Vertex input) : SV_Target {
    uint2 pixel = uint2(input.position.xy);
    bool paired = video_format >= 107;
    uint2 address = uint2(paired ? pixel.x / 2 : pixel.x, pixel.y);
    uint4 code = source_uint_image.Load(int3(address, 0));
    // Y210 stores ten significant bits in the high bits of each sixteen-bit word.
    if (video_format == 108) code >>= 6;
    float scale = video_format == 100 || video_format == 107 ? 1.0 :
        video_format == 101 || video_format == 108 ? 4.0 : 256.0;
    float y = paired ? ((pixel.x & 1) == 0 ? code.r : code.b) : video_format == 100 ? code.b : code.g;
    float u = paired || video_format == 100 ? code.g : code.r;
    float v = paired ? code.a : video_format == 100 ? code.r : code.b;
    y = (y - 16.0 * scale) / (219.0 * scale);
    u = (u - 128.0 * scale) / (224.0 * scale);
    v = (v - 128.0 * scale) / (224.0 * scale);
    float3 encoded = float3(y + 1.5748 * v,
        y - (0.0722 * 1.8556 / 0.7152) * u - (0.2126 * 1.5748 / 0.7152) * v,
        y + 1.8556 * u);
    float3 color = float3(decode_bt709(encoded.r), decode_bt709(encoded.g), decode_bt709(encoded.b));
    float maximum_alpha = video_format == 100 ? 255.0 : video_format == 101 ? 3.0 : 65535.0;
    float coverage = paired || video_target_alpha == ALPHA_OPAQUE ? 1.0 : code.a / maximum_alpha;
    [branch] if (video_target_encoding == ENCODING_SRGB) color = encode_rgb(color);
    if (video_target_alpha == ALPHA_PREMULTIPLIED) color *= coverage;
    return float4(color, coverage);
}

float4 load_source(int2 position) {
    position = clamp(position, int2(0, 0), int2(source_extent) - 1);
    float4 stored = load_stored(position);
    // Packed BGR views may expand channels through an eight-bit representation. Recover
    // the original quantized component before linear decoding/premultiplication/filtering.
    if (source_storage == 1) {
        float3 maximum = float3(31, 63, 31);
        stored.rgb = round(stored.rgb * maximum) / maximum;
    } else if (source_storage == 2) {
        stored.rgb = round(stored.rgb * 31) / 31;
    } else if (source_storage == 3) {
        // XR_BIAS is transported through its copy-compatible raw R10 UNORM member.
        // Recover each stored code before applying the documented scale/offset.
        stored.rgb = (round(stored.rgb * 1023) - 384) / 510;
    }
    float coverage = source_has_alpha == 0 || source_alpha == ALPHA_OPAQUE ? 1 : stored.a;
    if (coverage == 0) {
        return 0;
    }
    float3 straight = source_alpha == ALPHA_PREMULTIPLIED ? stored.rgb / coverage : stored.rgb;
    if (source_encoding == ENCODING_SRGB) {
        straight = decode_rgb(straight);
    }
    return float4(straight * coverage, coverage);
}

float4 sample_source(float2 uv) {
    if (filter_linear == 0) {
        return load_source(int2(floor(uv * source_extent)));
    }
    float2 texel = uv * source_extent - 0.5;
    int2 first = int2(floor(texel));
    float2 weight = frac(texel);
    // Decode and premultiply each texel before interpolation, including encoded-premultiplied
    // sources. Sampling UNORM first would interpolate sRGB/straight alpha in the wrong space.
    float4 top = lerp(load_source(first), load_source(first + int2(1, 0)), weight.x);
    float4 bottom = lerp(load_source(first + int2(0, 1)), load_source(first + int2(1, 1)), weight.x);
    return lerp(top, bottom, weight.y);
}

float4 composite(float4 source, float4 destination) {
    if (blend_mode == 1) {
        // Porter-Duff Plus in the bounded SDR representation, including coverage.
        return saturate(source + destination);
    }
    float coverage = source.a + destination.a * (1 - source.a);
    float3 color;
    if (blend_mode == 2) {
        color = source.rgb * (1 - destination.a) + destination.rgb * (1 - source.a)
            + source.rgb * destination.rgb;
    } else if (blend_mode == 3) {
        color = source.rgb + destination.rgb - source.rgb * destination.rgb;
    } else {
        color = source.rgb + destination.rgb * (1 - source.a);
    }
    return float4(color, coverage);
}

float4 encode_target(float4 value) {
    if (target_alpha == ALPHA_OPAQUE) {
        // Opaque presentation composites coverage over black before transfer encoding.
        float3 color = value.rgb;
        // The transfer is uniform for this draw. A select can still evaluate all pow calls
        // when an SRGB target view does the encoding; retain an actual coherent branch.
        [branch]
        if (target_encoding == ENCODING_SRGB) {
            color = encode_rgb(color);
        }
        return float4(color, 1);
    }
    if (target_encoding != ENCODING_SRGB && target_alpha == ALPHA_PREMULTIPLIED) {
        return value;
    }
    float3 color = value.a > 0 ? value.rgb / value.a : 0;
    if (target_encoding == ENCODING_SRGB) {
        color = encode_rgb(color);
    }
    if (target_alpha == ALPHA_PREMULTIPLIED) {
        color *= value.a;
    }
    return float4(color, value.a);
}

float4 finish_pixel(Vertex input, float4 result) {
    if (has_previous != 0) {
        float4 previous = previous_image.Load(int3(int2(input.position.xy), 0));
        result = composite(result, previous);
    }
    return encode_target(result);
}

float4 ps_main(Vertex input) : SV_Target {
    return finish_pixel(input, sample_source(source_rect.xy + input.uv * source_rect.zw) * opacity);
}

float4 ps_mapped(Vertex input) : SV_Target {
    float3 position = float3(input.uv, 1);
    float3 projected = float3(
        dot(inverse_row0.xyz, position),
        dot(inverse_row1.xyz, position),
        dot(inverse_row2.xyz, position));
    float4 source = 0;
    if (projected.z > 0) {
        float2 uv = projected.xy / projected.z;
        if (all(uv >= 0) && all(uv <= 1)) {
            source = sample_source(source_rect.xy + uv * source_rect.zw) * opacity;
        }
    }
    // Transparent outside the quad preserves the accumulated backdrop, including mirrored
    // or partially off-target mappings. Crop is applied in source space after inverse warp.
    return finish_pixel(input, source);
}

float4 canvas_row(uint index, uint row) {
    // CPU header/palette: 160 bytes; each unchanged primitive occupies thirteen rows.
    return asfloat(pattern_data.Load4(160 + (index * 13 + row) * 16));
}

float4 canvas_color(uint index, uint color) {
    return canvas_row(index, 5 + color);
}

float4 canvas_glyph(uint index, float2 p) {
    uint4 metadata = pattern_data.Load4(160 + index * 208);
    float4 geometry0 = canvas_row(index, 2);
    float4 geometry1 = canvas_row(index, 3);
    float2 tile = floor(p * geometry0.xy);
    if (any(tile >= ceil(geometry0.xy))) return 0;
    float2 local = p - tile / geometry0.xy - geometry1.xy;
    if (any(local < 0) || local.y >= geometry0.w) return 0;
    uint character = (uint)floor(local.x / geometry0.z);
    if (character >= metadata.w) return 0;
    uint columns = (uint)ceil(geometry0.x);
    uint offset = metadata.z + ((uint)tile.y * columns + (uint)tile.x) * metadata.w + character;
    if (offset >= 8192) return 0;
    uint word = pattern_data.Load(6816 + (offset / 4) * 4);
    uint code = (word >> ((offset % 4) * 8)) & 127;
    if (metadata.y != 0) {
        if (code < 32 || code > 126) return 0;
        uint slot = code - 32;
        float4 atlas = canvas_color(index, 1);
        float2 within = float2(frac(local.x / geometry0.z), local.y / geometry0.w);
        float2 uv = atlas.xy + (float2(slot % 16, slot / 16) + within) / float2(16, 6) * atlas.zw;
        float2 texel = uv * source_extent - 0.5;
        int2 first = (int2)floor(texel);
        float2 weight = frac(texel);
        int2 limit = int2(source_extent) - 1;
        float top = lerp(source_image.Load(int3(clamp(first, 0, limit), 0)).a,
            source_image.Load(int3(clamp(first + int2(1, 0), 0, limit), 0)).a, weight.x);
        float bottom = lerp(source_image.Load(int3(clamp(first + int2(0, 1), 0, limit), 0)).a,
            source_image.Load(int3(clamp(first + int2(1, 1), 0, limit), 0)).a, weight.x);
        return canvas_color(index, 0) * lerp(top, bottom, weight.y);
    }
    uint2 pixel = (uint2)floor(float2(frac(local.x / geometry0.z) * 9, local.y / geometry0.w * 8));
    if (pixel.x >= 8 || pixel.y >= 8) return 0;
    uint2 glyph = canvas_font[code];
    uint row = ((pixel.y < 4 ? glyph.x : glyph.y) >> ((pixel.y % 4) * 8)) & 255;
    return ((row >> pixel.x) & 1) != 0 ? canvas_color(index, 0) : 0;
}

float4 canvas_primitive(uint index, float2 uv) {
    float4 bounds = canvas_row(index, 1);
    float2 p = (uv - bounds.xy) / bounds.zw;
    if (any(p < 0) || any(p > 1)) return 0;
    uint4 metadata = pattern_data.Load4(160 + index * 208);
    uint kind = metadata.x;
    if (kind == 6) {
        float4 image = canvas_row(index, 2);
        return sample_source(image.xy + p * image.zw) * canvas_color(index, 0).a;
    }
    if (kind == 7) return canvas_glyph(index, p);
    float4 geometry0 = canvas_row(index, 2);
    if (kind == 1) {
        uint2 cell = (uint2)floor(p * geometry0.xy);
        return canvas_color(index, (cell.x + cell.y) & 1);
    }
    if (kind == 2) {
        float2 phase = frac(p * geometry0.xy);
        float2 distance = min(phase, 1-phase) / geometry0.xy;
        if (all(distance > geometry0.zw)) return 0;
    }
    if (kind == 3 && abs(length((p - 0.5) * 2) - 1) > geometry0.x) return 0;
    if (kind == 4) {
        float4 geometry1 = canvas_row(index, 3);
        float2 aspect = float2(geometry1.y, 1);
        float2 start = geometry0.xy * aspect;
        float2 delta = (geometry0.zw - geometry0.xy) * aspect;
        float along = saturate(dot(p * aspect - start, delta) / dot(delta, delta));
        if (length(p * aspect - start - delta * along) > geometry1.x) return 0;
    }
    if (kind == 5) return canvas_color(index, min((uint)floor(p.x * 8), 7));
    if (metadata.y != 0) {
        float4 paint = canvas_row(index, 4);
        float hue = dot(p, paint.xy) - paint.z;
        float3 rgb = saturate(abs(frac(hue + float3(0, 2.0/3.0, 1.0/3.0)) * 6 - 3) - 1);
        return float4(rgb * paint.w, paint.w);
    }
    return canvas_color(index, 0);
}

float4 ps_pattern(Vertex input) : SV_Target {
    uint4 header = pattern_data.Load4(0);
    uint pattern_kind = header.x;
    uint pattern_mapped = header.y;
    uint2 pattern_cells = header.zw;
    float3 position = float3(input.uv, 1);
    float3 projected = pattern_mapped != 0 ? float3(
        dot(inverse_row0.xyz, position), dot(inverse_row1.xyz, position),
        dot(inverse_row2.xyz, position)) : position;
    float4 color = 0;
    if (projected.z > 0) {
        float2 uv = projected.xy / projected.z;
        if (all(uv >= 0) && all(uv <= 1)) {
            uv = source_rect.xy + uv * source_rect.zw;
            if (pattern_kind == 0) {
                float2 phase = frac(uv * pattern_cells);
                bool is_grid_line = any(min(phase, 1-phase) < asfloat(pattern_data.Load(16)));
                color = asfloat(pattern_data.Load4(is_grid_line ? 32 : 48));
            } else if (pattern_kind == 1) {
                uint stripe = min((uint)floor(uv.x * 8), 7);
                color = asfloat(pattern_data.Load4(32 + stripe * 16));
            } else {
                uint2 cell = min((uint2)floor(uv * 16), uint2(15, 15));
                uint candidates = pattern_data.Load(15008 + (cell.y * 16 + cell.x) * 4);
                [loop]
                while (candidates != 0) {
                    uint index = firstbitlow(candidates);
                    candidates &= candidates - 1;
                    float4 top = canvas_primitive(index, uv);
                    color = top + color * (1 - top.a);
                }
            }
        }
    }
    return finish_pixel(input, color * opacity);
}

float4 ps_overlay(Vertex input) : SV_Target {
    float2 p = input.position.xy;
    float distance_to_edge = 1e20;
    float path_position = 0;
    bool inside = false;
    if (overlay_kind.x == 1) {
        float length_from_center = length(p - overlay_points[0].xy);
        distance_to_edge = abs(length_from_center - overlay_style.y);
        inside = length_from_center <= overlay_style.y;
    } else {
        float path_length = 0;
        [unroll] for (uint i = 0; i < 4; i++) {
            float2 a = overlay_points[i].xy;
            float2 b = overlay_points[(i + 1) % 4].xy;
            float2 edge = b - a;
            float edge_length = length(edge);
            float t = saturate(dot(p - a, edge) / max(dot(edge, edge), 1e-12));
            float edge_distance = length(p - a - edge * t);
            if (edge_distance < distance_to_edge) {
                distance_to_edge = edge_distance;
                path_position = path_length + t * edge_length;
            }
            path_length += edge_length;
            if ((a.y > p.y) != (b.y > p.y)) {
                if (p.x < a.x + (p.y - a.y) * edge.x / edge.y) inside = !inside;
            }
        }
    }
    float fill_coverage = saturate((inside ? distance_to_edge : -distance_to_edge) + 0.5);
    float stroke_coverage = overlay_style.x > 0 ? saturate(overlay_style.x * 0.5 + 0.5 - distance_to_edge) : 0;
    if (overlay_style.z > 0 && fmod(path_position, overlay_style.z + overlay_style.w) > overlay_style.z) stroke_coverage = 0;
    float4 fill = overlay_fill * fill_coverage;
    float4 stroke = overlay_stroke * stroke_coverage;
    return stroke + fill * (1 - stroke.a);
}
