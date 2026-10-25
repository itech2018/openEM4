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
fn shade(normal: vec3<f32>) -> f32 {
 let n = normalize(normal + vec3<f32>(0.00001));
 let key = max(dot(n,normalize(vec3<f32>(0.45,0.82,0.35))),0.0);
 let fill = max(dot(n,normalize(vec3<f32>(-0.65,0.3,-0.45))),0.0);
 let hemi = 0.5 + 0.5*n.y;
 return clamp(0.36+0.22*hemi+0.34*key+0.08*fill,0.0,1.0);
}
@fragment fn fs_main(v: VOut) -> @location(0) vec4<f32> {
 if (camera.debug.x != 0u) {
  return vec4<f32>(normalize(v.normal) * 0.5 + vec3<f32>(0.5), 1.0);
 }
 let texel = textureSample(color_texture, color_sampler, v.uv);
 return vec4<f32>(texel.rgb * shade(v.normal), texel.a);
}

@fragment fn fs_cutout(v: VOut) -> @location(0) vec4<f32> {
 if (camera.debug.x != 0u) {
  return vec4<f32>(normalize(v.normal) * 0.5 + vec3<f32>(0.5), 1.0);
 }
 let texel=textureSample(color_texture,color_sampler,v.uv);
 if (texel.a < 0.5) { discard; }
 return vec4<f32>(texel.rgb * shade(v.normal),1.0);
}

@fragment fn fs_emissive(v: VOut) -> @location(0) vec4<f32> {
 let category=u32(floor(v.uv.x/2.0));
 let u=fract(v.uv.x/2.0)*2.0;
 let corona=textureSample(color_texture,color_sampler,vec2<f32>(u,v.uv.y));
 // Legacy corona TGA may already be coloured. Tint primarily its luminance/alpha.
 let tint=select(vec3<f32>(1.0,0.09,0.08),vec3<f32>(0.1,0.35,1.0),category==1u);
 let tint2=select(tint,vec3<f32>(1.0,1.0,0.9),category==2u);
 let tint3=select(tint2,vec3<f32>(1.0,0.62,0.1),category==3u);
 // Preserve the original artwork, but keep the tint visible for pale coronas.
 let rgb=mix(tint3,corona.rgb,0.45);
 let radial=1.0-smoothstep(0.28,0.71,length(vec2<f32>(u,v.uv.y)-vec2<f32>(0.5)));
 let opacity=max(corona.a*radial,0.0);
 return vec4<f32>(rgb,opacity);
}
