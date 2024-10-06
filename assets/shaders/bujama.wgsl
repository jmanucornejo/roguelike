@group(1) @binding(0) var red_texture: texture_2d<f32>;
@group(1) @binding(1) var green_texture: texture_2d<f32>;
@group(1) @binding(2) var blue_texture: texture_2d<f32>;
@group(1) @binding(3) var mask_texture: texture_2d<f32>;
@group(1) @binding(4) var sampler: sampler;

@fragment
fn fragment_main(in: VertexOutput) -> @location(0) vec4<f32> {
    // Sample the mask texture
    let mask = textureSample(mask_texture, sampler, in.uv);

    // Sample the other textures
    let red_color = textureSample(red_texture, sampler, in.uv);
    let green_color = textureSample(green_texture, sampler, in.uv);
    let blue_color = textureSample(blue_texture, sampler, in.uv);

    // Mix the textures based on the RGB values of the mask
    let final_color = red_color * mask.r + green_color * mask.g + blue_color * mask.b;

    return final_color;
}