struct VSInput
{
    float4 pos : POSITION0;
    float4 color : COLOR0;
};

struct VSOutput
{
    float4 pos : SV_Position;
    float4 color : COLOR0;
};

VSOutput main(VSInput input) 
{
    VSOutput output;
    output.pos = input.pos;
    output.color = input.color;
    return output;
}
