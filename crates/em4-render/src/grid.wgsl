struct In { @location(0) position: vec3<f32>, @location(1) color: vec3<f32> };
struct Out { @builtin(position) position: vec4<f32>, @location(0) color: vec3<f32> };
@vertex fn vs_main(input: In) -> Out {
    var out: Out;
    out.position = vec4<f32>(input.position, 1.0);
    out.color = input.color;
    return out;
}
@fragment fn fs_main(input: Out) -> @location(0) vec4<f32> {
    return vec4<f32>(input.color, 1.0);
}
