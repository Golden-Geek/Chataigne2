cbuffer CardData : register(b0) { uint4 words0; uint4 words1; };
struct Vertex { float4 position : SV_Position; float2 uv : TEXCOORD; };
Vertex vs_main(uint id : SV_VertexID) {
    float2 p = float2((id << 1) & 2, id & 2);
    Vertex value;
    value.position = float4(p * float2(2, -2) + float2(-1, 1), 0, 1);
    value.uv = p;
    return value;
}
bool Rect(float2 p, float2 a, float2 b) { return all(p >= a) && all(p <= b); }
float HexDigit(float2 p, uint value) {
    uint masks[16] = {63,6,91,79,102,109,125,7,127,111,119,124,57,94,121,113};
    uint mask = masks[value & 15];
    bool segments[7] = {
        Rect(p,float2(.15,.02),float2(.85,.14)), Rect(p,float2(.80,.10),float2(.94,.48)),
        Rect(p,float2(.80,.52),float2(.94,.90)), Rect(p,float2(.15,.86),float2(.85,.98)),
        Rect(p,float2(.06,.52),float2(.20,.90)), Rect(p,float2(.06,.10),float2(.20,.48)),
        Rect(p,float2(.15,.44),float2(.85,.56))
    };
    [unroll] for (uint bit = 0; bit < 7; bit++) if (segments[bit] && (mask & (1 << bit))) return 1;
    return 0;
}
float4 ps_main(Vertex value) : SV_Target {
    float2 uv = value.uv;
    float2 marker = (uv - MARKER_ORIGIN) / MARKER_EXTENT;
    if (all(marker >= 0) && all(marker < 1)) {
        uint2 cell = (uint2)floor(marker * float2(MARKER_COLUMNS, MARKER_ROWS));
        return float4(MarkerRgb(cell.y * MARKER_COLUMNS + cell.x, words0, words1), 1);
    }
    // First eight hexadecimal digits show source ID, followed by sixteen frame-ID digits.
    if (Rect(uv, float2(.03,.24), float2(.97,.39))) {
        float2 digit = float2((uv.x - .03) / .94 * 24, (uv.y - .24) / .15);
        uint column = min((uint)digit.x, 23);
        uint word = column < 8 ? words0.x : (column < 16 ? words0.z : words0.y);
        uint nibble = (word >> ((7 - column % 8) * 4)) & 15;
        return float4(lerp(float3(.02,.02,.02), float3(1,1,1), HexDigit(frac(digit), nibble)), 1);
    }
    float3 color = .25 + .75 * frac(float3(.37,.61,.83) * float(words0.x + 1));
    uint2 cell = (uint2)floor(uv * float2(16, 9));
    float grid = frac(uv.x * 16) < .025 || frac(uv.y * 9) < .025;
    float checker = ((cell.x + cell.y) & 1) ? .23 : .06;
    float3 result = lerp(checker.xxx, color, .48);
    float2 local = frac(uv * float2(16, 9));
    if (local.y > .2 && local.y < .8 && local.x > .2 && local.x < .8) {
        uint number = cell.y * 16 + cell.x;
        float2 digit = float2((local.x - .2) / .6 * 2, (local.y - .2) / .6);
        uint nibble = (number >> ((1 - min((uint)digit.x, 1)) * 4)) & 15;
        result = lerp(result, float3(1,1,1), HexDigit(frac(digit), nibble));
    }
    float alpha = uv.y > .76 ? (floor(min(uv.x, .999) * 4) + 1) * .25 : 1;
    return float4(lerp(result, float3(1,1,1), grid), alpha);
}
