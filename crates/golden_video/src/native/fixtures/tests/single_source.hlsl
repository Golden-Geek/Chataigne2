Texture2D<float4> image : register(t0);

float4 vs_main(uint vertex : SV_VertexID) : SV_Position {
    float2 position = float2((vertex << 1) & 2, vertex & 2);
    return float4(position * float2(2, -2) + float2(-1, 1), 0, 1);
}

uint4 ps_main(float4 position : SV_Position) : SV_Target {
    uint words[6];
    MarkerRead(image, float2(0, 0), float2(1, 1), words);
    uint column = (uint)position.x % 3;
    if (column == 0) return uint4(words[0], words[1], words[2], words[3]);
    if (column == 1) return uint4(words[4], words[5], words[0], words[1]);
    return uint4(words[2], words[3], words[4], words[5]);
}
