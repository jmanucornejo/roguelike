// Water Vertex Shader
@group(0) @binding(0) var<uniform> time: f32;

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) world_pos: vec3<f32>,
    @location(1) uv: vec2<f32>,
};

@vertex
fn vertex(@location(0) position: vec3<f32>, @location(1) uv: vec2<f32>) -> VertexOutput {
    var out: VertexOutput;

    let wave_frequency: f32 = 2.0;
    let wave_amplitude: f32 = 0.1;

    let wave = sin(position.x * wave_frequency + time) * wave_amplitude;
    let displaced_position = vec3<f32>(position.x, position.y + wave, position.z);

    out.position = vec4<f32>(displaced_position, 1.0);
    out.world_pos = displaced_position;
    out.uv = uv;
    
    return out;
}

// Water Fragment Shader
@fragment
fn fragment(in: VertexOutput) -> @location(0) vec4<f32> {
    let shallow_color = vec3<f32>(0.4, 0.7, 0.9); // Light blue for shallow areas
    let deep_color = vec3<f32>(0.0, 0.0, 0.5);    // Dark blue for deep water

    let depth_factor = clamp(in.world_pos.y * 0.1, 0.0, 1.0);
    let color = mix(shallow_color, deep_color, depth_factor);

    // Simple fresnel-like effect to brighten the edges
    let fresnel = pow(1.0 - dot(normalize(in.world_pos), vec3<f32>(0.0, 1.0, 0.0)), 2.0);

    return vec4<f32>(color + fresnel, 1.0);
}