@group(0) @binding(0) var<uniform> aoe_center: vec3<f32>;
@group(0) @binding(1) var<uniform> aoe_radius: f32;
@group(0) @binding(2) var aoe_texture: texture_2d<f32>;
@group(0) @binding(3) var aoe_sampler: sampler;

@fragment
fn main(input: FragmentInput) -> @location(0) vec4<f32> {
    let distance = length(input.world_pos - aoe_center);
    if (distance <= aoe_radius) {
        let uv = vec2<f32>(
            0.5 + (input.world_pos.x - aoe_center.x) / aoe_radius,
            0.5 + (input.world_pos.z - aoe_center.z) / aoe_radius
        );
        let sampled_color = textureSample(aoe_texture, aoe_sampler, uv);
        return vec4<f32>(sampled_color.rgb, sampled_color.a * smoothstep(aoe_radius, 0.0, distance));
    }
    return vec4<f32>(0.0, 0.0, 0.0, 0.0); // Fully transparent outside the AoE radius
}