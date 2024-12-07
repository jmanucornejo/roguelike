@vertex
fn vertex(input: VertexInput) -> VertexOutput {
    var output: VertexOutput;
    output.position = input.position;
    
    // Add sine wave for wave animation
    output.position.y += sin(input.position.x * 5.0 + time) * 0.1;
    return output;
}

@fragment
fn fragment(input: FragmentInput) -> FragmentOutput {
    var output: FragmentOutput;
    output.color = vec4(0.0, 0.5, 1.0, 0.5); // Water-like blue
    return output;
}