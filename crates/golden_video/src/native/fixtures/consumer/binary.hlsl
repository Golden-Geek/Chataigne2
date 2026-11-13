Texture2D<float4> image : register(t0);

float4 vs_main(uint vertex : SV_VertexID) : SV_Position {
    float2 position = float2((vertex << 1) & 2, vertex & 2);
    return float4(position * float2(2, -2) + float2(-1, 1), 0, 1);
}

uint4 ps_main(float4 position : SV_Position) : SV_Target {
    uint words[12];
    [unroll] for (uint word = 0; word < 12; ++word) words[word] = 0;
    words[0] = 0x56464331u;
    words[3] = CODE_BITS;
    uint4 sentinel = (uint4)(saturate(image.Load(int3(SENTINEL_X, SENTINEL_Y, 0))) * 255.0 + 0.5);
    words[2] = sentinel.r | (sentinel.g << 8) | (sentinel.b << 16) | (sentinel.a << 24);
    [unroll] for (uint bit = 0; bit < CODE_BITS; ++bit) {
        uint value = (uint)(saturate(image.Load(int3(CODE_X + bit * CODE_STEP, CODE_Y, 0)).r) * 255.0 + 0.5);
        if (value >= CODE_THRESHOLD) words[1] |= 1u << bit;
        words[4 + bit / 4] |= value << ((bit % 4) * 8);
    }
    uint first = ((uint)position.x % 3) * 4;
    return uint4(words[first], words[first + 1], words[first + 2], words[first + 3]);
}
