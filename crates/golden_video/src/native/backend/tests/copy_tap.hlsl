Texture2D<float4> input_image : register(t0);
Texture2D<float4> converted_image : register(t1);

float4 vs_main(uint vertex : SV_VertexID) : SV_Position {
    float2 position = float2((vertex << 1) & 2, vertex & 2);
    return float4(position * float2(2, -2) + float2(-1, 1), 0, 1);
}

uint4 ps_main(float4 position : SV_Position) : SV_Target {
    uint input_words[6];
    uint converted_words[6];
    MarkerRead(input_image, float2(0, 0), float2(1, 1), input_words);
    MarkerRead(converted_image, float2(0, 0), float2(1, 1), converted_words);
    uint column = (uint)position.x % 3;
    if (column == 0) return uint4(input_words[0], input_words[1], input_words[2], input_words[3]);
    if (column == 1) return uint4(input_words[4], input_words[5], converted_words[0], converted_words[1]);
    return uint4(converted_words[2], converted_words[3], converted_words[4], converted_words[5]);
}
