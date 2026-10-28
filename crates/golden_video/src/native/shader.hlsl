Texture2D<float4> source_image : register(t0);
Texture2D<float4> previous_image : register(t1);

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
    uint2 reserved;
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

float4 load_source(int2 position) {
    position = clamp(position, int2(0, 0), int2(source_extent) - 1);
    float4 stored = source_image.Load(int3(position, 0));
    float coverage = source_alpha == ALPHA_OPAQUE ? 1 : stored.a;
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
        float3 color = target_encoding == ENCODING_SRGB ? encode_rgb(value.rgb) : value.rgb;
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

float4 ps_main(Vertex input) : SV_Target {
    float4 result = sample_source(input.uv) * opacity;
    if (has_previous != 0) {
        float4 previous = previous_image.Load(int3(int2(input.position.xy), 0));
        result = composite(result, previous);
    }
    return encode_target(result);
}
