Texture2D<float4> image : register(t0);

float4 vs_main(uint vertex : SV_VertexID) : SV_Position {
    float2 position = float2((vertex << 1) & 2, vertex & 2);
    return float4(position * float2(2, -2) + float2(-1, 1), 0, 1);
}

uint4 ps_main(float4 position : SV_Position) : SV_Target {
    uint left[6];
    uint right[6];
    MarkerRead(image, float2(0, 0), float2(0.5, 1), left);
    MarkerRead(image, float2(0.5, 0), float2(0.5, 1), right);
    uint column = (uint)position.x % 3;
    if (column == 0) return uint4(left[0], left[1], left[2], left[3]);
    if (column == 1) return uint4(left[4], left[5], right[0], right[1]);
    return uint4(right[2], right[3], right[4], right[5]);
}
