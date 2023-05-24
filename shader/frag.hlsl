struct PSInput
{
    float4 o_color : COLOR0;
};

struct PSOutput
{
    float4 uFragColor : SV_Target;
};

PSOutput main(PSInput input) 
{
    PSOutput output;
    output.uFragColor = input.o_color;
    return output;
}