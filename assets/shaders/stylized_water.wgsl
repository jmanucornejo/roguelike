#import bevy_pbr::{
    mesh_view_bindings::globals,
    forward_io::VertexOutput,
}

@group(#{MATERIAL_BIND_GROUP}) @binding(0) var<uniform> shallow_color: vec4<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(1) var<uniform> deep_color: vec4<f32>;
// x: world-space UV scale, y: wave strength, z/w: layer speeds.
@group(#{MATERIAL_BIND_GROUP}) @binding(2) var<uniform> water_settings: vec4<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(3) var water_normals_texture: texture_2d<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(4) var water_normals_sampler: sampler;

fn sample_water_normal(uv: vec2<f32>) -> vec3<f32> {
    // The source normal texture stores Y in blue, matching Bevy's official
    // water sample. Swizzling RBG converts it to tangent-space XYZ.
    return textureSample(water_normals_texture, water_normals_sampler, uv).rbg * 2.0 - 1.0;
}

@fragment
fn fragment(in: VertexOutput) -> @location(0) vec4<f32> {
    let base_uv = in.world_position.xz * water_settings.x;
    let time = globals.time;

    let first_uv = base_uv + vec2<f32>(water_settings.z, water_settings.w) * time;
    let second_uv =
        base_uv * 1.73 + vec2<f32>(-water_settings.w, water_settings.z * 0.7) * time;

    let first_wave = sample_water_normal(first_uv);
    let second_wave = sample_water_normal(second_uv);
    let waves = normalize(first_wave + second_wave);

    let wave_light = clamp(0.5 + waves.x * water_settings.y, 0.0, 1.0);
    let broad_ripple = sin((in.world_position.x + in.world_position.z) * 0.075 + time * 0.45);
    let color_mix = clamp(wave_light * 0.72 + broad_ripple * 0.08, 0.08, 0.92);
    let color = mix(deep_color.rgb, shallow_color.rgb, color_mix);

    // A small crest highlight keeps the surface readable from the game's
    // elevated camera without requiring expensive reflections.
    let crest = pow(clamp(waves.z * 0.5 + 0.5, 0.0, 1.0), 6.0) * 0.12;
    let opacity = mix(deep_color.a, shallow_color.a, color_mix);
    return vec4<f32>(color + vec3<f32>(crest), opacity);
}
