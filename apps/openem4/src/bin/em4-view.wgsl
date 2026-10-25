struct Camera { matrix: mat4x4<f32>, debug: vec4<u32> };
@group(0) @binding(0) var<uniform> camera: Camera;
@group(1) @binding(0) var color_texture: texture_2d<f32>;
@group(1) @binding(1) var color_sampler: sampler;
struct VIn { @location(0) pos: vec3<f32>, @location(1) normal: vec3<f32>, @location(2) uv: vec2<f32> };
struct VOut { @builtin(position) position: vec4<f32>, @location(0) normal: vec3<f32>, @location(1) uv: vec2<f32> };
@vertex fn vs_main(v: VIn) -> VOut {
 var out: VOut;
 out.position = camera.matrix * vec4<f32>(v.pos, 1.0);
 out.normal = v.normal;
 out.uv = v.uv;
 return out;
}
@fragment fn fs_main(v: VOut) -> @location(0) vec4<f32> {
 if (camera.debug.x != 0u) {
  return vec4<f32>(normalize(v.normal) * 0.5 + vec3<f32>(0.5), 1.0);
 }
 let texel = textureSample(color_texture, color_sampler, v.uv);
 let n = normalize(v.normal + vec3<f32>(0.00001, 0.00001, 0.00001));
 let light_dir = normalize(vec3<f32>(0.4,0.85,0.45));
 let diffuse = max(dot(n, light_dir), 0.0);
 let lighting = 0.58 + 0.42 * diffuse;
 return vec4<f32>(texel.rgb * lighting, texel.a);
}
