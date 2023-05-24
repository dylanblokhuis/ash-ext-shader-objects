RWTexture2D<float4> outputTexture : register(u0);

[numthreads(8, 8, 1)]
void main(uint3 DTid : SV_DispatchThreadID)
{
    outputTexture[DTid.xy] = float4(0.0f, 0.0f, 1.0f, 1.0f); // RGBA = blue
}