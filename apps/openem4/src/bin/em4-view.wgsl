struct Camera { matrix: mat4x4<f32> };
@group(0) @binding(0) var<uniform> camera: Camera;
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
  let lighting = 0.42 + 0.58 * abs(dot(normalize(v.normal + vec3<f32>(0.001)), normalize(vec3<f32>(0.3,0.8,0.6))));
  return vec4<f32>(vec3<f32>(0.72,0.8,0.9) * lighting, 1.0);
}
