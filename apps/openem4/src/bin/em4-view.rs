//! Standalone prototype 3D V3O viewer. Geometry is shaded without textures.
//! Left mouse: orbit; right mouse: pan; wheel: zoom; R: reset.
#[path = "../texture_upscale.rs"]
mod texture_upscale;
#[path = "../material_registry.rs"]
mod material_registry;
#[path = "../scene_mesh.rs"]
mod scene_mesh;
#[path = "../scene_entities.rs"]
mod scene_entities;
use std::{env, fs, mem, path::Path, sync::Arc, time::Instant};
use wgpu::util::DeviceExt;
use winit::{
 event::{ElementState, Event, MouseButton, MouseScrollDelta, WindowEvent},
 event_loop::{ControlFlow, EventLoop},
 keyboard::{KeyCode, PhysicalKey},
 window::WindowBuilder,
};
#[repr(C)]
#[derive(Clone,Copy,bytemuck::Pod,bytemuck::Zeroable)]
struct Vertex { pos:[f32;3], normal:[f32;3], uv:[f32;2] }
impl Vertex {
 const ATTRS:[wgpu::VertexAttribute;3]=wgpu::vertex_attr_array![0=>Float32x3,1=>Float32x3,2=>Float32x2];
 fn desc()->wgpu::VertexBufferLayout<'static> {
  wgpu::VertexBufferLayout{array_stride:mem::size_of::<Vertex>() as u64,step_mode:wgpu::VertexStepMode::Vertex,attributes:&Self::ATTRS}
 }
}
#[repr(C)]
#[derive(Clone,Copy,bytemuck::Pod,bytemuck::Zeroable)]
struct Uniform { matrix:[[f32;4];4], debug:[u32;4] }
#[repr(C)]
#[derive(Clone,Copy,bytemuck::Pod,bytemuck::Zeroable)]
struct ObjectUniform { model:[[f32;4];4] }
fn model_matrix(offset:[f32;3],yaw:f32,pivot:[f32;3])->[[f32;4];4]{
 let (sn,cs)=yaw.to_radians().sin_cos();
 let (cx,cz)=(pivot[0],pivot[2]);
 [ [cs,0.0,-sn,0.0],[0.0,1.0,0.0,0.0],[sn,0.0,cs,0.0],
   [cx-cs*cx-sn*cz+offset[0],offset[1],cz+sn*cx-cs*cz+offset[2],1.0] ]
}
fn identity_matrix()->[[f32;4];4]{model_matrix([0.0;3],0.0,[0.0;3])}
fn invert_rigid_y(m:[[f32;4];4])->[[f32;4];4]{
 let mut inverse=identity_matrix();
 for col in 0..3{for row in 0..3{inverse[col][row]=m[row][col];}}
 for row in 0..3{inverse[3][row]=-(0..3).map(|i|inverse[i][row]*m[3][i]).sum::<f32>();}
 inverse
}
#[derive(Clone,Copy)]
struct Camera {yaw:f32,pitch:f32,distance:f32,target:[f32;3]}
impl Camera {
 fn reset(target:[f32;3],distance:f32)->Self {Self{yaw:0.65,pitch:0.4,distance,target}}
 fn matrix(&self,aspect:f32)->[[f32;4];4] {
  let (sy,cy)=self.yaw.sin_cos();let (sp,cp)=self.pitch.sin_cos();
  let eye=[self.target[0]+self.distance*cp*sy,self.target[1]+self.distance*sp,self.target[2]+self.distance*cp*cy];
  let forward=normalize(sub(self.target,eye));
  let right=normalize(cross(forward,[0.0,1.0,0.0]));
  let up=cross(right,forward);
  let view=[
   [right[0],up[0],-forward[0],0.0],
   [right[1],up[1],-forward[1],0.0],
   [right[2],up[2],-forward[2],0.0],
   [-dot(right,eye),-dot(up,eye),dot(forward,eye),1.0],
  ];
  let f=1.0/(45f32.to_radians()/2.0).tan();
  // A mixed scene can span far more world units than a single vehicle.
  // Keep depth valid for the camera distance instead of clipping the scene
  // against the previous fixed ~1000-unit far plane.
  let near=0.05f32;
  let far=(self.distance*8.0).max(2000.0).min(10_000_000.0);
  let projection=[
   [f/aspect.max(0.01),0.0,0.0,0.0],
   [0.0,f,0.0,0.0],
   [0.0,0.0,far/(near-far),-1.0],
   [0.0,0.0,far*near/(near-far),0.0],
  ];
  multiply(projection,view)
 }
}
fn dot(a:[f32;3],b:[f32;3])->f32 {a[0]*b[0]+a[1]*b[1]+a[2]*b[2]}
fn sub(a:[f32;3],b:[f32;3])->[f32;3]{[a[0]-b[0],a[1]-b[1],a[2]-b[2]]}
fn cross(a:[f32;3],b:[f32;3])->[f32;3]{[a[1]*b[2]-a[2]*b[1],a[2]*b[0]-a[0]*b[2],a[0]*b[1]-a[1]*b[0]]}
fn normalize(a:[f32;3])->[f32;3]{let m=dot(a,a).sqrt().max(1e-7);[a[0]/m,a[1]/m,a[2]/m]}
fn multiply(a:[[f32;4];4],b:[[f32;4];4])->[[f32;4];4]{
 let mut out=[[0f32;4];4];
 for col in 0..4 {for row in 0..4 {for k in 0..4 {out[col][row]+=a[k][row]*b[col][k];}}}
 out
}
struct Depth { _texture:wgpu::Texture,view:wgpu::TextureView }
fn depth(device:&wgpu::Device,w:u32,h:u32)->Depth {
 let texture=device.create_texture(&wgpu::TextureDescriptor {
  label:Some("V3O depth"),size:wgpu::Extent3d{width:w.max(1),height:h.max(1),depth_or_array_layers:1},
  mip_level_count:1,sample_count:1,dimension:wgpu::TextureDimension::D2,format:wgpu::TextureFormat::Depth24Plus,
  usage:wgpu::TextureUsages::RENDER_ATTACHMENT,view_formats:&[],
 });
 let view=texture.create_view(&Default::default());Depth{_texture:texture,view}
}
#[derive(Default)]
#[derive(Clone,Copy,Default)]
struct LightControls { headlights:bool, brake:bool, left:bool, right:bool, beacon:bool }
impl LightControls {
 fn phase(&self,seconds:f32)->[u32;4] {
  let indicator=(seconds*1.5).fract()<0.5;
  let emergency=(seconds*3.0).fract()<0.5;
  [u32::from(self.headlights),u32::from(self.brake),
   u32::from(indicator&&(self.left||self.right)),u32::from(self.beacon&&emergency)]
 }
}
#[derive(Clone)]
struct DoorPreview { range:std::ops::Range<usize>,pivot:[f32;3],direction:f32,prototype:String,axis:[f32;3],angle:f32,native:bool }
fn estimated_mip_bytes(w:u32,h:u32)->u64{
 let (mut w,mut h)=(w.max(1) as u64,h.max(1) as u64);let mut bytes=0u64;
 loop {bytes=bytes.saturating_add(w.saturating_mul(h).saturating_mul(4));if w==1&&h==1{break;}w=(w/2).max(1);h=(h/2).max(1);}
 bytes
}
struct ResidentEnhancement {indices:Vec<usize>,bytes:u64,job_id:usize}
struct EnhancedTexture { job_id:usize,indices:std::sync::Arc<std::sync::Mutex<Vec<usize>>>,w:u32,h:u32,pixels:Vec<u8> }
#[derive(Clone)]
struct EnhancementJob {job_id:usize,indices:std::sync::Arc<std::sync::Mutex<Vec<usize>>>,name:String,w:u32,h:u32,pixels:Vec<u8>,scale:u32}
fn material_group(device:&wgpu::Device,queue:&wgpu::Queue,layout:&wgpu::BindGroupLayout,sampler:&wgpu::Sampler,w:u32,h:u32,pixels:Vec<u8>)->Result<wgpu::BindGroup,String>{
 let levels=32-w.max(h).leading_zeros();
 let texture=device.create_texture(&wgpu::TextureDescriptor{label:Some("V3O texture"),
  size:wgpu::Extent3d{width:w,height:h,depth_or_array_layers:1},mip_level_count:levels,sample_count:1,
  dimension:wgpu::TextureDimension::D2,format:wgpu::TextureFormat::Rgba8UnormSrgb,
  usage:wgpu::TextureUsages::TEXTURE_BINDING|wgpu::TextureUsages::COPY_DST,view_formats:&[]});
 let mut img=image::RgbaImage::from_raw(w,h,pixels).ok_or("invalid RGBA material buffer")?;
 for mip in 0..levels{
  let (mw,mh)=img.dimensions();
  queue.write_texture(wgpu::ImageCopyTexture{texture:&texture,mip_level:mip,origin:wgpu::Origin3d::ZERO,aspect:wgpu::TextureAspect::All},
   img.as_raw(),wgpu::ImageDataLayout{offset:0,bytes_per_row:Some(mw*4),rows_per_image:Some(mh)},
   wgpu::Extent3d{width:mw,height:mh,depth_or_array_layers:1});
  if mip+1<levels{img=image::imageops::resize(&img,(mw/2).max(1),(mh/2).max(1),image::imageops::FilterType::Triangle);}
 }
 let view=texture.create_view(&Default::default());
 Ok(device.create_bind_group(&wgpu::BindGroupDescriptor{label:Some("V3O material bind"),layout,entries:&[
  wgpu::BindGroupEntry{binding:0,resource:wgpu::BindingResource::TextureView(&view)},
  wgpu::BindGroupEntry{binding:1,resource:wgpu::BindingResource::Sampler(sampler)}
 ]}))
}

#[derive(Clone,Copy,Debug)]
enum SceneParent {Source(usize),Instance(u64)}
struct RuntimeInstance {id:u64,source:usize,offset:[f32;3],yaw:f32,lights:LightControls,parent:Option<SceneParent>,uniform:wgpu::Buffer,bind:wgpu::BindGroup}
struct Viewer {
 surface:wgpu::Surface<'static>,device:wgpu::Device,queue:wgpu::Queue,config:wgpu::SurfaceConfiguration,
 pipeline:wgpu::RenderPipeline,cutout_pipeline:wgpu::RenderPipeline,transparent_pipeline:wgpu::RenderPipeline,vertices:wgpu::Buffer,indices:wgpu::Buffer,draws:Vec<(std::ops::Range<u32>,Arc<wgpu::BindGroup>,u8,[f32;3])>,
 object_vertices:Vec<wgpu::Buffer>,object_indices:Vec<wgpu::Buffer>,object_draw_ranges:Vec<std::ops::Range<u32>>,uniform:wgpu::Buffer,bind:wgpu::BindGroup,depth:Depth,camera:Camera,
 initial_camera:Camera,drag:Option<MouseButton>,cursor:Option<(f64,f64)>,debug_normals:bool,
 ground_vertex:wgpu::Buffer,ground_index:wgpu::Buffer,ground_material:wgpu::BindGroup,show_ground:bool,
 marker_pipeline:wgpu::RenderPipeline,occluded_marker_pipeline:wgpu::RenderPipeline,corona_occlusion:bool,marker_vertex:wgpu::Buffer,marker_index:wgpu::Buffer,
 light_markers:Vec<(std::ops::Range<u32>,u8,f32,f32,usize)>,corona_anchors:Vec<([f32;3],f32,u8)>,corona_materials:Vec<wgpu::BindGroup>,
 material_layout:wgpu::BindGroupLayout,material_sampler:wgpu::Sampler,enhancements:std::sync::mpsc::Receiver<EnhancedTexture>,enhancement_done:usize,enhancement_total:usize,enhancement_gpu_budget:u64,enhancement_gpu_used:u64,baseline_texture_bytes:u64,pending_enhancements:Vec<(usize,EnhancementJob)>,enhancement_sender:std::sync::mpsc::SyncSender<EnhancementJob>,enhancement_inflight:usize,restore_jobs:Vec<(usize,EnhancementJob)>,restore_eligible_at:Vec<Instant>,restore_pending:Vec<bool>,original_materials:Vec<Arc<wgpu::BindGroup>>,resident_enhancements:Vec<ResidentEnhancement>,
 scene_ranges:Vec<std::ops::Range<usize>>,scene_offsets:Vec<[f32;3]>,scene_yaws:Vec<f32>,scene_pivots:Vec<[f32;3]>,object_layout:wgpu::BindGroupLayout,scene_uniforms:Vec<wgpu::Buffer>,scene_binds:Vec<wgpu::BindGroup>,draw_scene_ids:Vec<usize>,scene_instances:Vec<RuntimeInstance>,next_instance_id:u64,selected_scene:usize,selected_instance:Option<u64>,
 lights:LightControls,source_lights:Vec<LightControls>,started:Instant,doors:Vec<DoorPreview>,closed_vertices:Vec<Vertex>,door_progress:Vec<f32>,door_targets:Vec<bool>,last_door_tick:Instant,selected_door:Option<usize>,
}
impl Viewer {
 async fn new(window:Arc<winit::window::Window>,mesh:&em4_formats::v3o_mesh::Mesh,model_path:&Path,light_defs:&[em4_formats::e4p_lights::Light],doors:Vec<DoorPreview>,upscale_factor:u32,scene_ranges:Vec<std::ops::Range<usize>>,light_sources:Vec<usize>)->Result<Self,String>{
  if mesh.vertices.is_empty()||mesh.triangles.is_empty(){return Err("V3O model contains no renderable triangles".into());}
  let mut min=[f32::INFINITY;3];let mut max=[f32::NEG_INFINITY;3];
  // EM4 is Z-up. Convert positions AND normals into renderer Y-up.
  let vertices:Vec<Vertex>=mesh.vertices.iter().map(|v|{
   let position=[v.position[0],v.position[2],-v.position[1]];
   let normal=[v.normal[0],v.normal[2],-v.normal[1]];
   for i in 0..3 {min[i]=min[i].min(position[i]);max[i]=max[i].max(position[i]);}
   Vertex{pos:position,normal,uv:v.uv}
  }).collect();
  let target=[(min[0]+max[0])*0.5,(min[1]+max[1])*0.5,(min[2]+max[2])*0.5];
  let diagonal=((max[0]-min[0]).powi(2)+(max[1]-min[1]).powi(2)+(max[2]-min[2]).powi(2)).sqrt();
  let camera=Camera::reset(target,(diagonal*1.5).max(2.0));
  println!("Scene bounds (renderer Y-up): min={min:?} max={max:?}; camera distance={:.2}; adaptive far clip={:.2}",camera.distance,(camera.distance*8.0).max(2000.0));
  let mut indices:Vec<u32>=Vec::with_capacity(mesh.triangles.len()*3);
  let mut ranges=Vec::new();
  for surface in 0..mesh.surfaces.len(){
   let start=indices.len() as u32;
   for t in mesh.triangles.iter().filter(|t|t.surface_index==surface){indices.extend_from_slice(&t.indices);}
   ranges.push(start..indices.len() as u32);
  }
  // Mixed scenes get independently addressable per-object index streams.
  // Geometry is still stored in a shared vertex allocation, but each object's
  // draw references its own index buffer instead of the monolithic index list.
  let mut object_index_data=Vec::<Vec<u32>>::new();
  let mut object_draw_ranges=vec![0..0;mesh.surfaces.len()];
  if !scene_ranges.is_empty(){
   object_index_data=vec![Vec::new();scene_ranges.len()];
   for (surface_id,vertex_range) in scene_ranges.iter().enumerate(){
    let mut local=Vec::new();
    for material in 0..mesh.surfaces.len(){
     let start=local.len() as u32;
     for triangle in mesh.triangles.iter().filter(|t|t.surface_index==material
       &&t.indices.iter().all(|&index|vertex_range.contains(&(index as usize)))){
      local.extend(triangle.indices.iter().map(|&index|index-vertex_range.start as u32));
     }
     // Each surface belongs to exactly one of the separate V3O scene objects.
     if local.len() as u32>start{object_draw_ranges[material]=start..local.len() as u32;}
    }
    object_index_data[surface_id]=local;
   }
  }
  let instance=wgpu::Instance::default();
  let surface=instance.create_surface(window.clone()).map_err(|e|e.to_string())?;
  let adapter=instance.request_adapter(&wgpu::RequestAdapterOptions{
   power_preference:wgpu::PowerPreference::HighPerformance,compatible_surface:Some(&surface),force_fallback_adapter:false
  }).await.ok_or("No compatible GPU adapter")?;
  // Adapter class is only a conservative memory proxy, not VRAM detection.
  let enhancement_budget_mib=match adapter.get_info().device_type{
   wgpu::DeviceType::DiscreteGpu=>256u64,
   wgpu::DeviceType::IntegratedGpu=>96,
   wgpu::DeviceType::VirtualGpu|wgpu::DeviceType::Cpu=>64,
   _=>128,
  };
  println!("Automatic enhanced-texture budget: {} MiB (adapter class heuristic)",enhancement_budget_mib);
  let (device,queue)=adapter.request_device(&wgpu::DeviceDescriptor{
   label:Some("V3O viewer"),required_features:wgpu::Features::empty(),required_limits:wgpu::Limits::default()
  },None).await.map_err(|e|e.to_string())?;
  let sz=window.inner_size();let caps=surface.get_capabilities(&adapter);
  let format=caps.formats.iter().copied().find(|f|f.is_srgb()).unwrap_or(caps.formats[0]);
  let config=wgpu::SurfaceConfiguration{usage:wgpu::TextureUsages::RENDER_ATTACHMENT,format,
   width:sz.width.max(1),height:sz.height.max(1),present_mode:wgpu::PresentMode::Fifo,
   alpha_mode:caps.alpha_modes[0],view_formats:vec![],desired_maximum_frame_latency:2};
  surface.configure(&device,&config);
  let uniform=device.create_buffer_init(&wgpu::util::BufferInitDescriptor{label:Some("camera"),
   contents:bytemuck::bytes_of(&Uniform{matrix:camera.matrix(config.width as f32/config.height as f32),debug:[0;4]}),
   usage:wgpu::BufferUsages::UNIFORM|wgpu::BufferUsages::COPY_DST});
  let layout=device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor{
   label:Some("camera bind layout"),entries:&[wgpu::BindGroupLayoutEntry{
    binding:0,visibility:wgpu::ShaderStages::VERTEX_FRAGMENT,ty:wgpu::BindingType::Buffer{
     ty:wgpu::BufferBindingType::Uniform,has_dynamic_offset:false,min_binding_size:None},count:None
   }]});
  let bind=device.create_bind_group(&wgpu::BindGroupDescriptor{label:Some("camera bind"),
   layout:&layout,entries:&[wgpu::BindGroupEntry{binding:0,resource:uniform.as_entire_binding()}]});
  let material_layout=device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor{
   label:Some("V3O material layout"),entries:&[
    wgpu::BindGroupLayoutEntry{binding:0,visibility:wgpu::ShaderStages::FRAGMENT,
     ty:wgpu::BindingType::Texture{sample_type:wgpu::TextureSampleType::Float{filterable:true},view_dimension:wgpu::TextureViewDimension::D2,multisampled:false},count:None},
    wgpu::BindGroupLayoutEntry{binding:1,visibility:wgpu::ShaderStages::FRAGMENT,
     ty:wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),count:None},
   ]});
  let object_layout=device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor{
   label:Some("object transform bind layout"),entries:&[wgpu::BindGroupLayoutEntry{
    binding:0,visibility:wgpu::ShaderStages::VERTEX,
    ty:wgpu::BindingType::Buffer{ty:wgpu::BufferBindingType::Uniform,has_dynamic_offset:false,min_binding_size:None},count:None
   }]
  });
  let mut scene_pivots=Vec::new();
  for range in &scene_ranges {
   let points=&vertices[range.clone()];
   let (mut min_x,mut max_x,mut min_z,mut max_z)=(f32::INFINITY,f32::NEG_INFINITY,f32::INFINITY,f32::NEG_INFINITY);
   for v in points{min_x=min_x.min(v.pos[0]);max_x=max_x.max(v.pos[0]);min_z=min_z.min(v.pos[2]);max_z=max_z.max(v.pos[2]);}
   scene_pivots.push([(min_x+max_x)*0.5,0.0,(min_z+max_z)*0.5]);
  }
  let mut scene_uniforms=Vec::new();let mut scene_binds=Vec::new();
  // Index zero is the identity transform for ground, markers and E4P.
  for _ in 0..=scene_ranges.len() {
   let buffer=device.create_buffer_init(&wgpu::util::BufferInitDescriptor{
    label:Some("object model uniform"),contents:bytemuck::bytes_of(&ObjectUniform{model:identity_matrix()}),
    usage:wgpu::BufferUsages::UNIFORM|wgpu::BufferUsages::COPY_DST
   });
   let group=device.create_bind_group(&wgpu::BindGroupDescriptor{label:Some("object model bind"),layout:&object_layout,
    entries:&[wgpu::BindGroupEntry{binding:0,resource:buffer.as_entire_binding()}]});
   scene_uniforms.push(buffer);scene_binds.push(group);
  }
  let sampler=device.create_sampler(&wgpu::SamplerDescriptor{address_mode_u:wgpu::AddressMode::Repeat,address_mode_v:wgpu::AddressMode::Repeat,
   mag_filter:wgpu::FilterMode::Linear,min_filter:wgpu::FilterMode::Linear,mipmap_filter:wgpu::FilterMode::Linear,..Default::default()});
  let (enhance_tx,enhance_rx)=std::sync::mpsc::channel::<EnhancedTexture>();
  let (job_tx,job_rx)=std::sync::mpsc::sync_channel::<EnhancementJob>(16);
  let shared_jobs=std::sync::Arc::new(std::sync::Mutex::new(job_rx));
  for _ in 0..2 {
   let jobs=shared_jobs.clone();let complete=enhance_tx.clone();
   std::thread::spawn(move||loop {
    let job={let Ok(receiver)=jobs.lock()else{return};receiver.recv()};
    let Ok(job)=job else{return};
    let (w,h,pixels)=texture_upscale::upscale(job.w,job.h,job.pixels,job.scale);
    let indices=job.indices;
    if w!=job.w||h!=job.h{let _=complete.send(EnhancedTexture{job_id:job.job_id,indices,w,h,pixels});}
    else{let _=complete.send(EnhancedTexture{job_id:job.job_id,indices,w:0,h:0,pixels:Vec::new()});eprintln!("Enhancement skipped for {}",job.name);}
   });
  }
  let mut staged_jobs=Vec::<(usize,EnhancementJob,blake3::Hash)>::new();
  let enhancement_total;
  let mut duplicate_count=0usize;
  let mut pending_keys=std::collections::HashMap::<blake3::Hash,std::sync::Arc<std::sync::Mutex<Vec<usize>>>>::new();
  let mut draws=Vec::new();
  let mut material_registry=material_registry::MaterialRegistry::new();
  for (i,surface) in mesh.surfaces.iter().enumerate(){
   let (w,h,pixels)=load_surface_image(model_path,&surface.texture);
   if upscale_factor!=0{
    let scale=if upscale_factor==4{4}else{texture_upscale::automatic_factor(&surface.texture,w,h,&pixels)};
    if scale>1 {
     let mut hasher=blake3::Hasher::new();
     hasher.update(b"openem4-enhancement-job-v1");
     hasher.update(&w.to_le_bytes());hasher.update(&h.to_le_bytes());
     hasher.update(&scale.to_le_bytes());hasher.update(&pixels);
     let key=hasher.finalize();
     if let Some(indices)=pending_keys.get(&key){
      if let Ok(mut group)=indices.lock(){group.push(i);duplicate_count+=1;}
     }else{
      let indices=std::sync::Arc::new(std::sync::Mutex::new(vec![i]));
      let job=EnhancementJob{job_id:staged_jobs.len(),indices:indices.clone(),name:surface.texture.clone(),w,h,pixels:pixels.clone(),scale};
      // Stage jobs so large visible mesh surfaces get first chance at the
      // bounded queue, rather than ordering by unrelated surface-file order.
      let triangles=mesh.triangles.iter().filter(|t|t.surface_index==i).count();
      staged_jobs.push((triangles,job,key));
      pending_keys.insert(key,indices);
     }

    }
   }
   let alpha:Vec<u8>=pixels.chunks_exact(4).map(|pixel|pixel[3]).collect();
   let (min_alpha,max_alpha)=alpha.iter().fold((255u8,0u8),|(lo,hi),&a|(lo.min(a),hi.max(a)));
   let has_alpha=alpha.iter().any(|&a|a<250);
   let binary_alpha=alpha.iter().all(|&a|a<24||a>231);
   let material_kind=if !has_alpha{0u8}else if binary_alpha{1u8}else{2u8};
   println!("Material {} {} texture={} mode={} size={}x{} alpha={}..{}",i,surface.name,surface.texture,match material_kind{0=>"opaque",1=>"cutout",_=>"blended"},w,h,min_alpha,max_alpha);
   let group=material_registry.get_or_upload(w,h,pixels,|w,h,pixels|
    material_group(&device,&queue,&material_layout,&sampler,w,h,pixels))?;
   let mut center=[0.0f32;3];let mut count=0f32;
   for t in mesh.triangles.iter().filter(|t|t.surface_index==i){
    for &v in &t.indices {let p=vertices[v as usize].pos;for axis in 0..3{center[axis]+=p[axis];}count+=1.0;}
   }
   if count>0.0 {for x in &mut center{*x/=count;}}
   draws.push((ranges[i].clone(),group,material_kind,center));
  }
  let pending_enhancements=staged_jobs.into_iter().map(|(triangles,job,_)|(triangles,job)).collect::<Vec<_>>();
  enhancement_total=pending_enhancements.len();
  let restore_jobs=pending_enhancements.clone();
  let restore_eligible_at=vec![Instant::now();restore_jobs.len()];
  let restore_pending=vec![false;restore_jobs.len()];
  drop(enhance_tx);
  println!("Enhancement candidates: {} unique jobs ({} duplicate surface requests joined); priority adapts to camera",enhancement_total,duplicate_count);
    let shader=device.create_shader_module(wgpu::ShaderModuleDescriptor{
   label:Some("V3O shader"),source:wgpu::ShaderSource::Wgsl(include_str!("em4-view.wgsl").into())});
  let pipeline_layout=device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor{
   label:None,bind_group_layouts:&[&layout,&material_layout,&object_layout],push_constant_ranges:&[]});
  let make_pipeline=|mode:u8|device.create_render_pipeline(&wgpu::RenderPipelineDescriptor{
   label:Some(if mode==2{"V3O transparent pipeline"}else if mode==1{"V3O cutout pipeline"}else{"V3O opaque pipeline"}),
   layout:Some(&pipeline_layout),
   vertex:wgpu::VertexState{module:&shader,entry_point:"vs_main",buffers:&[Vertex::desc()],compilation_options:Default::default()},
   fragment:Some(wgpu::FragmentState{module:&shader,entry_point:if mode==1{"fs_cutout"}else{"fs_main"},
    targets:&[Some(wgpu::ColorTargetState{format,blend:if mode==2{Some(wgpu::BlendState::ALPHA_BLENDING)}else{None},
     write_mask:wgpu::ColorWrites::ALL})],
    compilation_options:Default::default()}),
   primitive:wgpu::PrimitiveState{cull_mode:None,..Default::default()},
   depth_stencil:Some(wgpu::DepthStencilState{format:wgpu::TextureFormat::Depth24Plus,
    depth_write_enabled:mode!=2,depth_compare:wgpu::CompareFunction::Less,
    stencil:Default::default(),bias:Default::default()}),
   multisample:Default::default(),multiview:None
  });
  let pipeline=make_pipeline(0);
  let cutout_pipeline=make_pipeline(1);
  let transparent_pipeline=make_pipeline(2);
  // Original E4P point records; no guessed bounds-based positions.
  let mut mv:Vec<Vertex>=Vec::new();let mut mi:Vec<u32>=Vec::new();
  let mut light_markers=Vec::new();
  let mut corona_anchors=Vec::new();
  let mut corona_materials=Vec::new();
  let mut corona_cache=std::collections::HashMap::<String,Option<std::path::PathBuf>>::new();
  for (light_index,light) in light_defs.iter().enumerate() {
   let tex=light.corona_texture.to_ascii_lowercase();
   let kind:u8=if tex.contains("blue"){1}else if tex.contains("red"){0}
    else if tex.contains("yellow"){3}else{2};
   let p=[light.position[0],light.position[2],-light.position[1]];
   // The E4P corona size is used as a relative size. Clamp pathological values.
   let size=if light.corona_size.is_finite()&&light.corona_size>0.0{
    (light.corona_size*0.5).clamp(0.08,10.0)
   }else{0.35};
   let first=mi.len() as u32;
   let base=mv.len() as u32;
   for (u,v) in [(0.0,0.0),(1.0,0.0),(1.0,1.0),(0.0,1.0)]{
    mv.push(Vertex{pos:p,normal:[0.0,0.0,1.0],uv:[kind as f32*2.0+u,v]});
   }
   mi.extend_from_slice(&[base,base+1,base+2,base,base+2,base+3]);
   light_markers.push((first..first+6,kind,light.flash_frequency,light.flash_offset,*light_sources.get(light_index).unwrap_or(&0)));
   // Resolve legacy corona TGA by basename under the source game's Data folder.
   // Fallback to an in-memory radial alpha mask if the original cannot be read.
   let cache_key=light.corona_texture.to_ascii_lowercase();
   let corona_path=corona_cache.entry(cache_key).or_insert_with(||find_corona_file(model_path,&light.corona_texture));
   let loaded=corona_path.as_ref().and_then(|file|image::open(file).ok().map(|im|(file.clone(),im.to_rgba8())));
   let rgba=if let Some((file,img))=loaded {
    println!("Loaded original E4P corona: {}",file.display());img
   } else {
    eprintln!("Corona texture missing/unsupported: {} (procedural fallback)",light.corona_texture);
    image::RgbaImage::from_fn(64,64,|x,y|{
     let dx=(x as f32-31.5)/31.5;let dy=(y as f32-31.5)/31.5;
     let a=(1.0-(dx*dx+dy*dy).sqrt()).max(0.0).powf(1.6);
     image::Rgba([255,255,255,(a*255.0) as u8])
    })
   };
   // Original corona TGAs can have unusable all-zero or all-opaque alpha.
   // Recover opacity from RGB intensity only for these degenerate cases.
   let mut rgba=rgba;
   let (lo,hi)=rgba.pixels().fold((255u8,0u8),|(lo,hi),p|(lo.min(p[3]),hi.max(p[3])));
   if lo==hi {
    for p in rgba.pixels_mut(){p[3]=p[0].max(p[1]).max(p[2]);}
   }
   let (tw,th)=rgba.dimensions();
   let tex=device.create_texture(&wgpu::TextureDescriptor{
    label:Some("original E4P corona"),size:wgpu::Extent3d{width:tw,height:th,depth_or_array_layers:1},
    mip_level_count:1,sample_count:1,dimension:wgpu::TextureDimension::D2,
    format:wgpu::TextureFormat::Rgba8UnormSrgb,
    usage:wgpu::TextureUsages::TEXTURE_BINDING|wgpu::TextureUsages::COPY_DST,view_formats:&[]
   });
   queue.write_texture(wgpu::ImageCopyTexture{texture:&tex,mip_level:0,origin:wgpu::Origin3d::ZERO,aspect:wgpu::TextureAspect::All},
    rgba.as_raw(),wgpu::ImageDataLayout{offset:0,bytes_per_row:Some(tw*4),rows_per_image:Some(th)},
    wgpu::Extent3d{width:tw,height:th,depth_or_array_layers:1});
   let tex_view=tex.create_view(&Default::default());
   corona_materials.push(device.create_bind_group(&wgpu::BindGroupDescriptor{
    label:Some("E4P corona material"),layout:&material_layout,entries:&[
     wgpu::BindGroupEntry{binding:0,resource:wgpu::BindingResource::TextureView(&tex_view)},
     wgpu::BindGroupEntry{binding:1,resource:wgpu::BindingResource::Sampler(&sampler)}]
   }));
   corona_anchors.push((p,size,kind));
  }
  println!("Loaded {} original E4P light definitions",light_markers.len());
  // GPU buffers require non-empty data even when the prototype has no lights.
  if mv.is_empty() {mv.push(Vertex{pos:[0.;3],normal:[0.,1.,0.],uv:[0.,0.]});mi.push(0);}
  let marker_vertex=device.create_buffer_init(&wgpu::util::BufferInitDescriptor{
   label:Some("preview light verts"),contents:bytemuck::cast_slice(&mv),usage:wgpu::BufferUsages::VERTEX|wgpu::BufferUsages::COPY_DST});
  let marker_index=device.create_buffer_init(&wgpu::util::BufferInitDescriptor{
   label:Some("preview light indices"),contents:bytemuck::cast_slice(&mi),usage:wgpu::BufferUsages::INDEX});
  let occluded_marker_pipeline=device.create_render_pipeline(&wgpu::RenderPipelineDescriptor{
   label:Some("depth tested corona pipeline"),layout:Some(&pipeline_layout),
   vertex:wgpu::VertexState{module:&shader,entry_point:"vs_main",buffers:&[Vertex::desc()],compilation_options:Default::default()},
   fragment:Some(wgpu::FragmentState{module:&shader,entry_point:"fs_emissive",
    targets:&[Some(wgpu::ColorTargetState{format,blend:Some(wgpu::BlendState::ALPHA_BLENDING),write_mask:wgpu::ColorWrites::ALL})],compilation_options:Default::default()}),
   primitive:wgpu::PrimitiveState{cull_mode:None,..Default::default()},
   depth_stencil:Some(wgpu::DepthStencilState{format:wgpu::TextureFormat::Depth24Plus,
    depth_write_enabled:false,depth_compare:wgpu::CompareFunction::LessEqual,stencil:Default::default(),bias:Default::default()}),
   multisample:Default::default(),multiview:None
  });
  let marker_pipeline=device.create_render_pipeline(&wgpu::RenderPipelineDescriptor{
   label:Some("preview light pipeline"),layout:Some(&pipeline_layout),
   vertex:wgpu::VertexState{module:&shader,entry_point:"vs_main",buffers:&[Vertex::desc()],compilation_options:Default::default()},
   fragment:Some(wgpu::FragmentState{module:&shader,entry_point:"fs_emissive",
    targets:&[Some(wgpu::ColorTargetState{format,blend:Some(wgpu::BlendState::ALPHA_BLENDING),write_mask:wgpu::ColorWrites::ALL})],compilation_options:Default::default()}),
   primitive:wgpu::PrimitiveState{cull_mode:None,..Default::default()},
   depth_stencil:Some(wgpu::DepthStencilState{format:wgpu::TextureFormat::Depth24Plus,
    depth_write_enabled:false,depth_compare:wgpu::CompareFunction::Always,stencil:Default::default(),bias:Default::default()}),
   multisample:Default::default(),multiview:None
  });
  let vertex_buffer=device.create_buffer_init(&wgpu::util::BufferInitDescriptor{
   label:Some("V3O vertices"),contents:bytemuck::cast_slice(&vertices),usage:wgpu::BufferUsages::VERTEX|wgpu::BufferUsages::COPY_DST});
  let index_buffer=device.create_buffer_init(&wgpu::util::BufferInitDescriptor{
   label:Some("V3O triangles"),contents:bytemuck::cast_slice(&indices),usage:wgpu::BufferUsages::INDEX});
  let object_vertices=scene_ranges.iter().map(|range|device.create_buffer_init(&wgpu::util::BufferInitDescriptor{
   label:Some("scene object vertices"),contents:bytemuck::cast_slice(&vertices[range.clone()]),
   usage:wgpu::BufferUsages::VERTEX
  })).collect::<Vec<_>>();
  let object_indices=object_index_data.iter().map(|data|device.create_buffer_init(&wgpu::util::BufferInitDescriptor{
   label:Some("scene object indices"),contents:bytemuck::cast_slice(data),usage:wgpu::BufferUsages::INDEX
  })).collect::<Vec<_>>();
  // Flat reference plane positioned just below the loaded geometry.
  let radius=(diagonal*0.95).max(3.0);
  let floor=min[1]-0.03;
  let ground_verts=[
   Vertex{pos:[target[0]-radius,floor,target[2]-radius],normal:[0.0,1.0,0.0],uv:[0.0,0.0]},
   Vertex{pos:[target[0]+radius,floor,target[2]-radius],normal:[0.0,1.0,0.0],uv:[1.0,0.0]},
   Vertex{pos:[target[0]+radius,floor,target[2]+radius],normal:[0.0,1.0,0.0],uv:[1.0,1.0]},
   Vertex{pos:[target[0]-radius,floor,target[2]+radius],normal:[0.0,1.0,0.0],uv:[0.0,1.0]},
  ];
  let ground_vertex=device.create_buffer_init(&wgpu::util::BufferInitDescriptor{
   label:Some("ground positions"),contents:bytemuck::cast_slice(&ground_verts),usage:wgpu::BufferUsages::VERTEX});
  let ground_index=device.create_buffer_init(&wgpu::util::BufferInitDescriptor{
   label:Some("ground triangles"),contents:bytemuck::cast_slice(&[0u32,2,1,0,3,2]),usage:wgpu::BufferUsages::INDEX});
  let ground_tex=device.create_texture(&wgpu::TextureDescriptor{label:Some("ground reference texture"),
   size:wgpu::Extent3d{width:1,height:1,depth_or_array_layers:1},mip_level_count:1,sample_count:1,
   dimension:wgpu::TextureDimension::D2,format:wgpu::TextureFormat::Rgba8UnormSrgb,
   usage:wgpu::TextureUsages::COPY_DST|wgpu::TextureUsages::TEXTURE_BINDING,view_formats:&[]});
  queue.write_texture(wgpu::ImageCopyTexture{texture:&ground_tex,mip_level:0,origin:wgpu::Origin3d::ZERO,aspect:wgpu::TextureAspect::All},
   &[85,94,108,255],wgpu::ImageDataLayout{offset:0,bytes_per_row:Some(4),rows_per_image:Some(1)},
   wgpu::Extent3d{width:1,height:1,depth_or_array_layers:1});
  let ground_view=ground_tex.create_view(&Default::default());
  let ground_material=device.create_bind_group(&wgpu::BindGroupDescriptor{label:Some("ground material"),
   layout:&material_layout,entries:&[
    wgpu::BindGroupEntry{binding:0,resource:wgpu::BindingResource::TextureView(&ground_view)},
    wgpu::BindGroupEntry{binding:1,resource:wgpu::BindingResource::Sampler(&sampler)}]});
  let depth=depth(&device,config.width,config.height);
  let baseline_texture_bytes=material_registry.estimated_bytes();
  println!("Base unique textures: {} GPU uploads, {} surfaces reused; ~{} MiB estimate; enhancement allowance {} MiB",material_registry.unique_count(),material_registry.reused_count(),baseline_texture_bytes/1048576,enhancement_budget_mib);
  let draw_scene_ids=(0..mesh.surfaces.len()).map(|surface|{
   let first_vertex=mesh.triangles.iter().find(|t|t.surface_index==surface).map(|t|t.indices[0] as usize);
   first_vertex.and_then(|v|scene_ranges.iter().position(|r|r.contains(&v))).map(|id|id+1).unwrap_or(0)
  }).collect::<Vec<_>>();
  let original_materials=draws.iter().map(|d|d.1.clone()).collect();
  Ok(Self{surface,device,queue,config,pipeline,cutout_pipeline,transparent_pipeline,vertices:vertex_buffer,indices:index_buffer,
   draws,object_vertices,object_indices,object_draw_ranges,uniform,bind,depth,camera,initial_camera:camera,drag:None,cursor:None,debug_normals:false,ground_vertex,ground_index,ground_material,show_ground:true,marker_pipeline,occluded_marker_pipeline,corona_occlusion:false,marker_vertex,marker_index,light_markers,corona_anchors,corona_materials,material_layout,material_sampler:sampler,enhancements:enhance_rx,enhancement_done:0,enhancement_total,enhancement_gpu_budget:enhancement_budget_mib*1024*1024,enhancement_gpu_used:0,baseline_texture_bytes,pending_enhancements,enhancement_sender:job_tx,enhancement_inflight:0,restore_jobs,restore_eligible_at,restore_pending,original_materials,resident_enhancements:Vec::new(),scene_offsets:vec![[0.0;3];scene_ranges.len()],scene_yaws:vec![0.0;scene_ranges.len()],scene_pivots,object_layout,scene_uniforms,scene_binds,draw_scene_ids,scene_instances:Vec::new(),next_instance_id:scene_ranges.len() as u64+1,scene_ranges,selected_scene:0,selected_instance:None,lights:LightControls::default(),source_lights:vec![LightControls::default();scene_ranges.len()],started:Instant::now(),door_progress:vec![0.0;doors.len()],door_targets:vec![false;doors.len()],doors,closed_vertices:vertices,last_door_tick:Instant::now(),selected_door:None})
 }
 fn debug_hinges(&self){
  println!("=== E4P door hinge preview (heuristic, Y-up) ===");
  for (i,d) in self.doors.iter().enumerate(){
   println!("Door {} {}: pivot {:?}, swing {}, animation {:.0}%",
    i+1,d.prototype,d.pivot,d.angle.to_degrees(),self.door_progress[i]*100.0);
  }
 }
 fn queue_evicted_restorations(&mut self){
  let now=Instant::now();
  for id in 0..self.restore_jobs.len(){
   if !self.restore_pending[id]||now<self.restore_eligible_at[id]{continue;}
   // Do not duplicate already queued work, and never requeue while a job is running.
   let job=&self.restore_jobs[id].1;
   if self.pending_enhancements.iter().any(|(_,j)|j.job_id==id){continue;}
   // Only requeue when some camera motion has occurred: a lower-priority evicted
   // texture otherwise displaces a useful resident repeatedly.
   let (sy,cy)=self.camera.yaw.sin_cos();let (sp,cp)=self.camera.pitch.sin_cos();
   let eye=[self.camera.target[0]+self.camera.distance*cp*sy,self.camera.target[1]+self.camera.distance*sp,self.camera.target[2]+self.camera.distance*cp*cy];
   let forward=normalize(sub(self.camera.target,eye));
   let relevance=|indices:&[usize]|->f32{indices.iter().filter(|&&i|i<self.draws.len()).map(|&i|{
     let v=sub(self.scene_draw_center(i),eye);(0.1+0.9*dot(normalize(v),forward).max(0.0))/dot(v,v).sqrt().max(1.0)
   }).fold(0.0f32,f32::max)};
   let indices=job.indices.lock().map(|v|v.clone()).unwrap_or_default();
   let candidate=relevance(&indices);
   let weakest=self.resident_enhancements.iter().map(|r|relevance(&r.indices)).fold(f32::INFINITY,f32::min);
   if weakest.is_finite() && candidate<=weakest*1.25 {continue;}
   let (triangles,original_job)=&self.restore_jobs[id];
   self.pending_enhancements.push((*triangles,original_job.clone()));
   self.enhancement_total+=1;
   self.restore_pending[id]=false;
   println!("Texture restoration queued: {} (camera relevance increased)",original_job.name);
  }
 }
 fn scene_draw_center(&self,index:usize)->[f32;3]{
  let center=self.draws[index].3;
  let scene=self.draw_scene_ids.get(index).copied().unwrap_or(0);
  if scene==0{return center;}
  let id=scene-1;
  let matrix=model_matrix(self.scene_offsets[id],self.scene_yaws[id],self.scene_pivots[id]);
  [
   matrix[0][0]*center[0]+matrix[1][0]*center[1]+matrix[2][0]*center[2]+matrix[3][0],
   matrix[0][1]*center[0]+matrix[1][1]*center[1]+matrix[2][1]*center[2]+matrix[3][1],
   matrix[0][2]*center[0]+matrix[1][2]*center[1]+matrix[2][2]*center[2]+matrix[3][2]
  ]
 }
 fn dispatch_enhancements(&mut self){
  // Recompute priority before dispatch so orbit/pan/zoom affects remaining work.
  // Only two jobs in flight; other candidates stay eligible instead of being dropped.
  let (sy,cy)=self.camera.yaw.sin_cos();let (sp,cp)=self.camera.pitch.sin_cos();
  let eye=[self.camera.target[0]+self.camera.distance*cp*sy,
   self.camera.target[1]+self.camera.distance*sp,
   self.camera.target[2]+self.camera.distance*cp*cy];
  let forward=normalize(sub(self.camera.target,eye));
  // Precompute centres once so the scoring closure never captures `self`.
  // This avoids holding an immutable borrow across queue removal and dispatch.
  let centres=(0..self.draws.len()).map(|i|self.scene_draw_center(i)).collect::<Vec<_>>();
  let score=|triangles:usize,job:&EnhancementJob|->f32{
   let indices=job.indices.lock().map(|v|v.clone()).unwrap_or_default();
   let proximity=indices.iter().filter_map(|&index|centres.get(index)).map(|&center|{
    let toward=sub(center,eye);let distance=dot(toward,toward).sqrt().max(1.0);
    let facing=dot(normalize(toward),forward);
    (0.1+0.9*((facing-0.15)/0.85).clamp(0.0,1.0))/distance
   }).fold(0.0_f32,f32::max);
   triangles.max(1) as f32*proximity
  };
  while self.enhancement_inflight<2&&!self.pending_enhancements.is_empty(){
   let best=(0..self.pending_enhancements.len()).max_by(|&a,&b|{
    let (ta,ja)=&self.pending_enhancements[a];let (tb,jb)=&self.pending_enhancements[b];
    score(*ta,ja).total_cmp(&score(*tb,jb))
   }).unwrap();
   let (_,job)=self.pending_enhancements.swap_remove(best);
   match self.enhancement_sender.try_send(job){
    Ok(())=>self.enhancement_inflight+=1,
    Err(std::sync::mpsc::TrySendError::Full(job))=>{self.pending_enhancements.push((1,job));break;}
    Err(std::sync::mpsc::TrySendError::Disconnected(_))=>{self.pending_enhancements.clear();break;}
   }
  }
 }
 fn reclaim_enhancement_memory(&mut self,required:u64)->bool{
  if required>self.enhancement_gpu_budget{return false;}
  let (sy,cy)=self.camera.yaw.sin_cos();let (sp,cp)=self.camera.pitch.sin_cos();
  let eye=[self.camera.target[0]+self.camera.distance*cp*sy,
   self.camera.target[1]+self.camera.distance*sp,
   self.camera.target[2]+self.camera.distance*cp*cy];
  let forward=normalize(sub(self.camera.target,eye));
  while required>self.enhancement_gpu_budget.saturating_sub(self.enhancement_gpu_used){
   if self.resident_enhancements.is_empty(){return false;}
   let score=|item:&ResidentEnhancement|->f32{
    item.indices.iter().filter(|&&i|i<self.draws.len()).map(|&i|{
     let v=sub(self.scene_draw_center(i),eye);let dist=dot(v,v).sqrt().max(1.0);
     (0.1+0.9*dot(normalize(v),forward).max(0.0))/dist
    }).fold(0.0_f32,f32::max)
   };
   let victim=(0..self.resident_enhancements.len()).min_by(|&a,&b|
    score(&self.resident_enhancements[a]).total_cmp(&score(&self.resident_enhancements[b]))).unwrap();
   let old=self.resident_enhancements.swap_remove(victim);
   if let Some(pending)=self.restore_pending.get_mut(old.job_id){*pending=true;}
   if let Some(at)=self.restore_eligible_at.get_mut(old.job_id){*at=Instant::now()+std::time::Duration::from_secs(5);}
   for i in old.indices{
    if let (Some(draw),Some(original))=(self.draws.get_mut(i),self.original_materials.get(i)){
     draw.1=original.clone();
    }
   }
   self.enhancement_gpu_used=self.enhancement_gpu_used.saturating_sub(old.bytes);
   println!("Texture residency eviction: freed ~{} KiB (estimated active texture footprint ~{} MiB)",old.bytes/1024,(self.baseline_texture_bytes+self.enhancement_gpu_used)/1048576);
  }
  true
 }
 fn receive_enhancements(&mut self)->bool{
  let mut changed=false;
  // Budget uploads to one texture per event loop iteration.
  if let Ok(result)=self.enhancements.try_recv(){
   self.enhancement_done+=1;
   self.enhancement_inflight=self.enhancement_inflight.saturating_sub(1);
   if result.w==0||result.h==0 {return false;}
   if let Some(pending)=self.restore_pending.get_mut(result.job_id){*pending=false;}
   let required=estimated_mip_bytes(result.w,result.h);
   // Reject lower-value upgrades before displacing more visible resident textures.
   // This avoids budget thrashing when many textures finish concurrently.
   if required>self.enhancement_gpu_budget.saturating_sub(self.enhancement_gpu_used)
      && !self.resident_enhancements.is_empty() {
    let (sy,cy)=self.camera.yaw.sin_cos();let (sp,cp)=self.camera.pitch.sin_cos();
    let eye=[self.camera.target[0]+self.camera.distance*cp*sy,
     self.camera.target[1]+self.camera.distance*sp,self.camera.target[2]+self.camera.distance*cp*cy];
    let forward=normalize(sub(self.camera.target,eye));
    let importance=|indices:&[usize]|->f32{
     indices.iter().filter(|&&i|i<self.draws.len()).map(|&i|{
      let toward=sub(self.scene_draw_center(i),eye);
      (0.1+0.9*dot(normalize(toward),forward).max(0.0))/dot(toward,toward).sqrt().max(1.0)
     }).fold(0.0_f32,f32::max)
    };
    let request_indices=result.indices.lock().map(|v|v.clone()).unwrap_or_default();
    let candidate=importance(&request_indices);
    let weakest=self.resident_enhancements.iter().map(|r|importance(&r.indices)).fold(f32::INFINITY,f32::min);
    if candidate<=weakest*1.25 {
     println!("Enhanced texture deferred: existing GPU residents have higher camera priority");
     if let Some(p)=self.restore_pending.get_mut(result.job_id){*p=true;}
     if let Some(at)=self.restore_eligible_at.get_mut(result.job_id){*at=Instant::now()+std::time::Duration::from_secs(5);}
     return false;
    }
   }
   if !self.reclaim_enhancement_memory(required){
    eprintln!("Enhanced texture exceeds GPU budget ({} MiB), retaining original",required/1048576);
    return false;
   }
   if let Ok(new_group)=material_group(&self.device,&self.queue,&self.material_layout,&self.material_sampler,result.w,result.h,result.pixels){
    let new_group=Arc::new(new_group);
    let mut count=0;
    let indices=result.indices.lock().map(|v|v.clone()).unwrap_or_default();
    for &index in &indices {
     if let Some(draw)=self.draws.get_mut(index){
      draw.1=new_group.clone();changed=true;count+=1;
     }
    }
    if count>0{self.enhancement_gpu_used=self.enhancement_gpu_used.saturating_add(required);self.resident_enhancements.push(ResidentEnhancement{indices,bytes:required,job_id:result.job_id});}
    println!("Enhanced texture ready {}/{} ({} bindings; enhancements ~{} MiB; total texture estimate ~{} MiB)",self.enhancement_done,self.enhancement_total,count,self.enhancement_gpu_used/1048576,(self.baseline_texture_bytes+self.enhancement_gpu_used)/1048576);
   }
  }
  changed
 }
 fn toggle_doors(&mut self){
  if self.doors.is_empty(){println!("No E4P child doors found");return;}
  let indices:Vec<usize>=match self.selected_door{Some(i) if i<self.doors.len()=>vec![i],_=> (0..self.doors.len()).collect()};
  let opening=indices.iter().any(|&i|!self.door_targets[i]);
  for i in indices {self.door_targets[i]=opening;}
  println!("Door target: {} (selection {:?})",if opening{"OPEN"}else{"CLOSED"},self.selected_door);
 }
 fn select_door(&mut self,digit:usize){
  if digit==0 {self.selected_door=None;println!("Door selection: ALL");}
  else if digit<=self.doors.len(){self.selected_door=Some(digit-1);println!("Door selection: {}",digit);}
  else {println!("Door {} not present; {} door(s) decoded",digit,self.doors.len());}
 }
 fn tick_doors(&mut self)->bool {
  let now=Instant::now();
  let dt=now.duration_since(self.last_door_tick).as_secs_f32().min(0.1);
  self.last_door_tick=now;
  let mut moving=false;let mut changed=false;
  for (i,p) in self.door_progress.iter_mut().enumerate(){
   let goal=if self.door_targets[i]{1.0}else{0.0};
   let next=if *p<goal{(*p+dt*1.3).min(goal)}else{(*p-dt*1.3).max(goal)};
   if (next-*p).abs()>1e-6{*p=next;changed=true;}
   if (goal-*p).abs()>1e-5{moving=true;}
  }
  if changed {
   let mut verts=self.closed_vertices.clone();
   for (i,door) in self.doors.iter().enumerate(){
    let t=self.door_progress[i];let eased=t*t*(3.0-2.0*t);
    let (sn,cs)=(door.angle*eased).sin_cos();
    let axis=door.axis;
    for v in &mut verts[door.range.clone()]{
     let p=sub(v.pos,door.pivot);
     let rotated=[
      p[0]*cs+cross(axis,p)[0]*sn+axis[0]*dot(axis,p)*(1.0-cs),
      p[1]*cs+cross(axis,p)[1]*sn+axis[1]*dot(axis,p)*(1.0-cs),
      p[2]*cs+cross(axis,p)[2]*sn+axis[2]*dot(axis,p)*(1.0-cs)
     ];
     for k in 0..3 {v.pos[k]=door.pivot[k]+rotated[k];}
     let n=v.normal;let c=cross(axis,n);let d=dot(axis,n);
     for k in 0..3 {v.normal[k]=n[k]*cs+c[k]*sn+axis[k]*d*(1.0-cs);}
    }
   }
   self.queue.write_buffer(&self.vertices,0,bytemuck::cast_slice(&verts));
  }
  moving
 }
 fn resize(&mut self,w:u32,h:u32){
  if w==0||h==0{return;}self.config.width=w;self.config.height=h;
  self.surface.configure(&self.device,&self.config);self.depth=depth(&self.device,w,h);
 }
 fn render(&mut self)->Result<(),wgpu::SurfaceError>{
  let matrix=self.camera.matrix(self.config.width as f32/self.config.height as f32);
  self.queue.write_buffer(&self.uniform,0,bytemuck::bytes_of(&Uniform{matrix,debug:[u32::from(self.debug_normals),self.lights.phase(self.started.elapsed().as_secs_f32())[0],self.lights.phase(self.started.elapsed().as_secs_f32())[2],self.lights.phase(self.started.elapsed().as_secs_f32())[3]]}));
  // Rebuild camera-facing corona quads; the anchors remain the decoded E4P offsets.
  if !self.corona_anchors.is_empty(){
   let (sy,cy)=self.camera.yaw.sin_cos();let (sp,cp)=self.camera.pitch.sin_cos();
   let right=[cy,0.0,-sy];let up=[-sp*sy,cp,-sp*cy];
   let mut vertices=Vec::with_capacity(self.corona_anchors.len()*4);
   for (center,size,kind) in &self.corona_anchors {
    for (u,v) in [(0.0f32,0.0f32),(1.0,0.0),(1.0,1.0),(0.0,1.0)] {
     let x=(u-0.5)*2.0*(*size);let y=(v-0.5)*2.0*(*size);
     let pos=[center[0]+right[0]*x+up[0]*y,center[1]+right[1]*x+up[1]*y,center[2]+right[2]*x+up[2]*y];
     vertices.push(Vertex{pos,normal:[0.0,0.0,1.0],uv:[*kind as f32*2.0+u,v]});
    }
   }
   self.queue.write_buffer(&self.marker_vertex,0,bytemuck::cast_slice(&vertices));
  }
  let frame=self.surface.get_current_texture()?;
  let view=frame.texture.create_view(&Default::default());
  let mut encoder=self.device.create_command_encoder(&wgpu::CommandEncoderDescriptor{label:Some("V3O frame")});
  {
   let mut pass=encoder.begin_render_pass(&wgpu::RenderPassDescriptor{
    label:Some("V3O geometry"),color_attachments:&[Some(wgpu::RenderPassColorAttachment{
     view:&view,resolve_target:None,ops:wgpu::Operations{
      load:wgpu::LoadOp::Clear(wgpu::Color{r:0.06,g:0.08,b:0.12,a:1.0}),store:wgpu::StoreOp::Store}})],
    depth_stencil_attachment:Some(wgpu::RenderPassDepthStencilAttachment{
     view:&self.depth.view,depth_ops:Some(wgpu::Operations{load:wgpu::LoadOp::Clear(1.0),store:wgpu::StoreOp::Store}),
     stencil_ops:None}),timestamp_writes:None,occlusion_query_set:None});
   pass.set_pipeline(&self.pipeline);pass.set_bind_group(0,&self.bind,&[]);pass.set_bind_group(2,&self.scene_binds[0],&[]);
   pass.set_vertex_buffer(0,self.vertices.slice(..));pass.set_index_buffer(self.indices.slice(..),wgpu::IndexFormat::Uint32);
   if self.show_ground {
    pass.set_bind_group(1,&self.ground_material,&[]);
    pass.set_vertex_buffer(0,self.ground_vertex.slice(..));
    pass.set_index_buffer(self.ground_index.slice(..),wgpu::IndexFormat::Uint32);
    pass.draw_indexed(0..6,0,0..1);
   }
   pass.set_vertex_buffer(0,self.vertices.slice(..));pass.set_index_buffer(self.indices.slice(..),wgpu::IndexFormat::Uint32);
   // Stable opaque/cutout passes; blended batches sorted back-to-front each frame.
   pass.set_pipeline(&self.pipeline);
   for (i,(range,material,kind,_)) in self.draws.iter().enumerate() {
    if *kind!=0 {continue;}
    pass.set_bind_group(2,&self.scene_binds[self.draw_scene_ids[i]],&[]);
    if self.draw_scene_ids[i]>0 {
     pass.set_vertex_buffer(0,self.object_vertices[self.draw_scene_ids[i]-1].slice(..));
     pass.set_index_buffer(self.object_indices[self.draw_scene_ids[i]-1].slice(..),wgpu::IndexFormat::Uint32);
     pass.set_bind_group(1,material.as_ref(),&[]);pass.draw_indexed(self.object_draw_ranges[i].clone(),0,0..1);
    }else{
     pass.set_vertex_buffer(0,self.vertices.slice(..));
     pass.set_index_buffer(self.indices.slice(..),wgpu::IndexFormat::Uint32);
     pass.set_bind_group(1,material.as_ref(),&[]);pass.draw_indexed(range.clone(),0,0..1);
    }
   }
   pass.set_pipeline(&self.cutout_pipeline);
   for (i,(range,material,kind,_)) in self.draws.iter().enumerate() {
    if *kind!=1 {continue;}
    pass.set_bind_group(2,&self.scene_binds[self.draw_scene_ids[i]],&[]);
    if self.draw_scene_ids[i]>0 {
     pass.set_vertex_buffer(0,self.object_vertices[self.draw_scene_ids[i]-1].slice(..));
     pass.set_index_buffer(self.object_indices[self.draw_scene_ids[i]-1].slice(..),wgpu::IndexFormat::Uint32);
     pass.set_bind_group(1,material.as_ref(),&[]);pass.draw_indexed(self.object_draw_ranges[i].clone(),0,0..1);
    }else{
     pass.set_vertex_buffer(0,self.vertices.slice(..));
     pass.set_index_buffer(self.indices.slice(..),wgpu::IndexFormat::Uint32);
     pass.set_bind_group(1,material.as_ref(),&[]);pass.draw_indexed(range.clone(),0,0..1);
    }
   }
   let (sy,cy)=self.camera.yaw.sin_cos();let (sp,cp)=self.camera.pitch.sin_cos();
   let eye=[self.camera.target[0]+self.camera.distance*cp*sy,self.camera.target[1]+self.camera.distance*sp,self.camera.target[2]+self.camera.distance*cp*cy];
   let mut blended:Vec<usize>=self.draws.iter().enumerate().filter(|(_,d)|d.2==2).map(|(i,_)|i).collect();
   blended.sort_by(|&a,&b|{
    let pa=self.scene_draw_center(a);let pb=self.scene_draw_center(b);
    let da=dot(sub(pa,eye),sub(pa,eye));
    let db=dot(sub(pb,eye),sub(pb,eye));
    db.total_cmp(&da)
   });
   pass.set_pipeline(&self.transparent_pipeline);
   for i in blended {
    let (range,material,_,_)=&self.draws[i];
    pass.set_bind_group(2,&self.scene_binds[self.draw_scene_ids[i]],&[]);
    if self.draw_scene_ids[i]>0 {
     pass.set_vertex_buffer(0,self.object_vertices[self.draw_scene_ids[i]-1].slice(..));
     pass.set_index_buffer(self.object_indices[self.draw_scene_ids[i]-1].slice(..),wgpu::IndexFormat::Uint32);
     pass.set_bind_group(1,material.as_ref(),&[]);pass.draw_indexed(self.object_draw_ranges[i].clone(),0,0..1);
    }else{
     pass.set_vertex_buffer(0,self.vertices.slice(..));
     pass.set_index_buffer(self.indices.slice(..),wgpu::IndexFormat::Uint32);
     pass.set_bind_group(1,material.as_ref(),&[]);pass.draw_indexed(range.clone(),0,0..1);
    }
   }
   // Runtime instances reuse source mesh GPU buffers and material bind groups.
   // These are separate draw calls, with their own model transform uniforms.
   for kind in 0..=2u8 {
    let pipeline=match kind{0=>&self.pipeline,1=>&self.cutout_pipeline,_=>&self.transparent_pipeline};
    pass.set_pipeline(pipeline);
    for instance in &self.scene_instances {
     for (i,(range,material,material_kind,_)) in self.draws.iter().enumerate(){
      if *material_kind!=kind||self.draw_scene_ids[i]!=instance.source+1{continue;}
      pass.set_bind_group(2,&instance.bind,&[]);
      pass.set_bind_group(1,material.as_ref(),&[]);
      pass.set_vertex_buffer(0,self.object_vertices[instance.source].slice(..));
      pass.set_index_buffer(self.object_indices[instance.source].slice(..),wgpu::IndexFormat::Uint32);
      let _=range;
      pass.draw_indexed(self.object_draw_ranges[i].clone(),0,0..1);
     }
    }
   }
   pass.set_bind_group(2,&self.scene_binds[0],&[]);
   // Data-driven diagnostic markers: position, colour and pulse from source E4P.
   let t=self.started.elapsed().as_secs_f32();
   pass.set_pipeline(if self.corona_occlusion{&self.occluded_marker_pipeline}else{&self.marker_pipeline});
   pass.set_bind_group(1,&self.ground_material,&[]);
   pass.set_vertex_buffer(0,self.marker_vertex.slice(..));
   pass.set_index_buffer(self.marker_index.slice(..),wgpu::IndexFormat::Uint32);
   for (index,(range,kind,frequency,offset,source)) in self.light_markers.iter().enumerate() {
    // Per-light phase. The source frequency and phase are independent for
    // each E4P record. Their exact EM4 time units still require verification.
    let pulse=em4_formats::e4p_lights::preview_flash_on(t,*frequency,*offset);
    // Red/blue corona entries are emergency equipment by default; do not
    // activate every red emergency lamp when pressing the brake key.
    let enabled_for=|lights:&LightControls|match kind {
     0|1=>lights.beacon,
     2=>lights.headlights,
     _=>lights.left||lights.right
    };
    if pulse {
     let source_lights=if *source==0{&self.lights}else{&self.source_lights[*source-1]};
     pass.set_bind_group(1,&self.corona_materials[index],&[]);
     if enabled_for(source_lights){
      pass.set_bind_group(2,&self.scene_binds[*source],&[]);
      pass.draw_indexed(range.clone(),0,0..1);
     }
     for instance in self.scene_instances.iter().filter(|v|v.source+1==*source){
      if enabled_for(&instance.lights){
       pass.set_bind_group(2,&instance.bind,&[]);
       pass.draw_indexed(range.clone(),0,0..1);
      }
     }
    }
   }
  }
  self.queue.submit(Some(encoder.finish()));frame.present();Ok(())
 }
 fn source_world(&self,source:usize)->[[f32;4];4]{
  model_matrix(self.scene_offsets[source],self.scene_yaws[source],self.scene_pivots[source])
 }
 fn instance_world(&self,id:u64,visited:&mut Vec<u64>)->Option<[[f32;4];4]>{
  if visited.contains(&id)||visited.len()>64{return None;}
  let item=self.scene_instances.iter().find(|i|i.id==id)?;
  visited.push(id);
  let local=model_matrix(item.offset,item.yaw,self.scene_pivots[item.source]);
  let world=match item.parent{
   None=>Some(local),
   Some(SceneParent::Source(source))=>Some(multiply(self.source_world(source),local)),
   Some(SceneParent::Instance(parent))=>self.instance_world(parent,visited).map(|m|multiply(m,local))
  };
  visited.pop();world
 }
 fn parent_world(&self,parent:SceneParent)->Option<[[f32;4];4]>{
  match parent{
   SceneParent::Source(s)=>if s<self.scene_ranges.len(){Some(self.source_world(s))}else{None},
   SceneParent::Instance(id)=>self.instance_world(id,&mut Vec::new())
  }
 }
 fn refresh_instance_transforms(&self){
  for instance in &self.scene_instances{
   if let Some(model)=self.instance_world(instance.id,&mut Vec::new()){
    self.queue.write_buffer(&instance.uniform,0,bytemuck::bytes_of(&ObjectUniform{model}));
   }
  }
 }
 fn reparent_selected(&mut self,target:Option<SceneParent>){
  let Some(id)=self.selected_instance else {println!("Select a spawned instance first.");return;};
  if matches!(target,Some(SceneParent::Instance(parent)) if parent==id){println!("Cannot parent entity to itself.");return;}
  let Some(world)=self.instance_world(id,&mut Vec::new())else{return;};
  if let Some(SceneParent::Instance(parent))=target{
   if self.instance_world(parent,&mut vec![id]).is_none(){
    println!("Parenting rejected: missing parent or cycle.");return;
   }
  }
  let local=if let Some(parent)=target{
   let Some(parent_world)=self.parent_world(parent)else{return;};
   multiply(invert_rigid_y(parent_world),world)
  }else{world};
  if let Some(instance)=self.scene_instances.iter_mut().find(|v|v.id==id){
   let p=self.scene_pivots[instance.source];
   let yaw=(-local[0][2]).atan2(local[0][0]);
   let (sn,cs)=yaw.sin_cos();
   instance.offset=[
    local[3][0]-p[0]+cs*p[0]+sn*p[2],
    local[3][1],
    local[3][2]-p[2]-sn*p[0]+cs*p[2]
   ];
   instance.yaw=yaw.to_degrees().rem_euclid(360.0);
   instance.parent=target;
  }
  self.refresh_instance_transforms();
  println!("Updated parent for instance {id}; preserved world transform.");
 }
 fn toggle_parent_source(&mut self){
  let Some(id)=self.selected_instance else {return;};
  let attached=self.scene_instances.iter().find(|v|v.id==id).and_then(|v|v.parent).is_some();
  let target=if attached{None}else{
   let source=self.scene_instances.iter().find(|v|v.id==id).map(|v|v.source).unwrap();
   Some(SceneParent::Source(source))
  };
  self.reparent_selected(target);
 }
 fn parent_to_last_spawned(&mut self){
  let Some(selected)=self.selected_instance else{return;};
  let target=self.scene_instances.iter().rev().find(|v|v.id!=selected).map(|v|v.id);
  if let Some(id)=target{self.reparent_selected(Some(SceneParent::Instance(id)));}
  else{println!("No other spawned instance to use as parent.");}
 }
 fn update_scene_entity_vertices(&mut self){
  let id=self.selected_scene;
  if id>=self.scene_pivots.len(){return;}
  let model=model_matrix(self.scene_offsets[id],self.scene_yaws[id],self.scene_pivots[id]);
  self.queue.write_buffer(&self.scene_uniforms[id+1],0,bytemuck::bytes_of(&ObjectUniform{model}));
  self.refresh_instance_transforms();
 }
 fn move_scene_entity(&mut self,axis:usize,amount:f32){
  if axis>=3{return;}
  if let Some(id)=self.selected_instance{
   if let Some(instance)=self.scene_instances.iter_mut().find(|v|v.id==id){
    instance.offset[axis]+=amount;
    let model=model_matrix(instance.offset,instance.yaw,self.scene_pivots[instance.source]);
    self.queue.write_buffer(&instance.uniform,0,bytemuck::bytes_of(&ObjectUniform{model}));
    println!("Moved instance {} to {:?}",id,instance.offset);
    self.refresh_instance_transforms();
   }
   return;
  }
  if self.scene_ranges.get(self.selected_scene).is_none(){return;}
  self.scene_offsets[self.selected_scene][axis]+=amount;
  self.update_scene_entity_vertices();
  println!("Scene entity {} offset {:?}",self.selected_scene+1,self.scene_offsets[self.selected_scene]);
 }
 fn rotate_scene_entity(&mut self,degrees:f32){
  if let Some(id)=self.selected_instance{
   if let Some(instance)=self.scene_instances.iter_mut().find(|v|v.id==id){
    instance.yaw=(instance.yaw+degrees).rem_euclid(360.0);
    let model=model_matrix(instance.offset,instance.yaw,self.scene_pivots[instance.source]);
    self.queue.write_buffer(&instance.uniform,0,bytemuck::bytes_of(&ObjectUniform{model}));
    println!("Rotated instance {} to {:.1} degrees",id,instance.yaw);
    self.refresh_instance_transforms();
   }
   return;
  }
  if self.scene_ranges.get(self.selected_scene).is_none(){return;}
  self.scene_yaws[self.selected_scene]=(self.scene_yaws[self.selected_scene]+degrees).rem_euclid(360.0);
  self.update_scene_entity_vertices();
  println!("Scene entity {} yaw {:.1}",self.selected_scene+1,self.scene_yaws[self.selected_scene]);
 }
 fn selected_lights_mut(&mut self)->&mut LightControls{
  if let Some(id)=self.selected_instance {
   if let Some(index)=self.scene_instances.iter().position(|i|i.id==id){
    return &mut self.scene_instances[index].lights;
   }
  }
  if self.scene_ranges.is_empty(){&mut self.lights}
  else{&mut self.source_lights[self.selected_scene]}
 }
 fn spawn_selected_instance(&mut self){
  let source=self.selected_instance.and_then(|id|self.scene_instances.iter().find(|i|i.id==id).map(|i|i.source)).unwrap_or(self.selected_scene);
  if source>=self.scene_ranges.len()||self.scene_instances.len()>=64{return;}
  let id=self.next_instance_id;self.next_instance_id+=1;
  let offset=[self.scene_offsets[source][0]+3.0,self.scene_offsets[source][1],self.scene_offsets[source][2]];
  let yaw=self.scene_yaws[source];
  let model=model_matrix(offset,yaw,self.scene_pivots[source]);
  let uniform=self.device.create_buffer_init(&wgpu::util::BufferInitDescriptor{
   label:Some("runtime scene instance"),contents:bytemuck::bytes_of(&ObjectUniform{model}),
   usage:wgpu::BufferUsages::UNIFORM|wgpu::BufferUsages::COPY_DST
  });
  // Match the pre-existing per-object transform layout, recovered from its bind
  // by retaining the layout separately in the viewer.
  let bind=self.device.create_bind_group(&wgpu::BindGroupDescriptor{
   label:Some("runtime scene instance bind"),layout:&self.object_layout,
   entries:&[wgpu::BindGroupEntry{binding:0,resource:uniform.as_entire_binding()}]
  });
  self.scene_instances.push(RuntimeInstance{id,source,offset,yaw,lights:LightControls::default(),parent:None,uniform,bind});
  self.selected_instance=Some(id);
  println!("Spawned and selected instance {id} from scene object {}",source+1);
 }
 fn remove_selected_instance(&mut self){
  let Some(id)=self.selected_instance else {
   println!("Loaded source objects are protected; select a spawned instance to delete.");return;
  };
  if let Some(index)=self.scene_instances.iter().position(|item|item.id==id){
   // Detach children while preserving their current world-space transforms.
   let children=self.scene_instances.iter().filter(|v|matches!(v.parent,Some(SceneParent::Instance(parent)) if parent==id)).map(|v|v.id).collect::<Vec<_>>();
   for child in children {self.selected_instance=Some(child);self.reparent_selected(None);}
   self.scene_instances.remove(index);
   self.selected_instance=None;
   println!("Removed selected runtime instance {id}");
  }
 }
 fn select_next_scene_entity(&mut self){
  let count=self.scene_ranges.len()+self.scene_instances.len();
  if count==0{return;}
  let current=if let Some(id)=self.selected_instance{
   self.scene_instances.iter().position(|v|v.id==id).map(|i|self.scene_ranges.len()+i).unwrap_or(0)
  }else{self.selected_scene};
  let next=(current+1)%count;
  if next<self.scene_ranges.len(){
   self.selected_scene=next;self.selected_instance=None;
   println!("Selected source entity {}",next+1);
  }else{
   let item=&self.scene_instances[next-self.scene_ranges.len()];
   self.selected_scene=item.source;self.selected_instance=Some(item.id);
   println!("Selected spawned instance {} (source {})",item.id,item.source+1);
  }
 }
 fn mouse_motion(&mut self,x:f64,y:f64){
  if let (Some(button),Some((last_x,last_y)))=(self.drag,self.cursor) {
   let dx=(x-last_x) as f32;let dy=(y-last_y) as f32;
   if button==MouseButton::Left {
    self.camera.yaw+=dx*0.007;
    self.camera.pitch=(self.camera.pitch+dy*0.007).clamp(-1.5,1.5);
   } else if button==MouseButton::Right {
    let scale=self.camera.distance*0.0015;
    let right=[self.camera.yaw.cos(),0.0,-self.camera.yaw.sin()];
    for (i,component) in right.iter().enumerate(){self.camera.target[i]-=dx*scale*component;}
    self.camera.target[1]+=dy*scale;
   }
  }
  self.cursor=Some((x,y));
 }
}

// A bounded, case-insensitive search for original Emergency 4 corona textures.
fn find_corona_file(model_path:&Path,name:&str)->Option<std::path::PathBuf>{
 if name.is_empty()||name.contains(".."){return None}
 let file_name=Path::new(name.replace('\\',"/").as_str()).file_name()?.to_string_lossy().to_string();
 let data=data_root(model_path).ok()?;
 let mut stack=Vec::new();
 if data.parent().and_then(|p|p.file_name()).is_some_and(|n|n.to_string_lossy().eq_ignore_ascii_case("Mods")){stack.push((data.parent()?.parent()?.join("Data"),0));}
 stack.push((data.clone(),0usize));
 let mut visited=0usize;
 while let Some((dir,depth))=stack.pop(){
  visited+=1;if visited>12000 {break}
  let Ok(entries)=fs::read_dir(&dir)else{continue};
  for e in entries.flatten(){
   let Ok(ft)=e.file_type()else{continue};
   if ft.is_symlink(){continue;}
   if ft.is_file()&&e.file_name().to_string_lossy().eq_ignore_ascii_case(&file_name){return Some(e.path());}
   if depth<8&&ft.is_dir(){stack.push((e.path(),depth+1));}
  }
 }
 None
}

fn load_surface_image(model_path:&Path,filename:&str)->(u32,u32,Vec<u8>){
 let fallback=|| (2,2,vec![220,220,220,255,120,120,120,255,120,120,120,255,220,220,220,255]);
 if filename.trim().is_empty(){return fallback();}
 let requested=filename.replace('\\',"/");
 let path=Path::new(&requested);
 // Texture paths may be local V3O filenames, explicit mod: game references,
 // or an absolute child-model texture path produced by E4P assembly.
 let found=if path.is_absolute() {
  path.is_file().then(||path.to_path_buf())
 }else if let Some(game_ref)=requested.strip_prefix("mod:"){
  data_root(model_path).ok().and_then(|root|resolve_game_ref(&root,game_ref))
 }else {
  let local=model_path.parent().and_then(|folder|lookup_in_root(folder,&requested));
  local.or_else(||data_root(model_path).ok().and_then(|root|resolve_game_ref(&root,&requested)))
 };
 let Some(found)=found else{eprintln!("Missing texture: {filename}");return fallback()};
 let Ok(bytes)=fs::read(&found)else{eprintln!("Cannot read texture: {}",found.display());return fallback()};
 let decoded=if found.extension().is_some_and(|e|e.to_string_lossy().eq_ignore_ascii_case("dds")){
  em4_formats::dds::decode_dds(&bytes).map(|im|(im.width,im.height,im.pixels)).map_err(|e|e.to_string())
 }else{
  image::load_from_memory(&bytes).map(|im|{let rgba=im.to_rgba8();(rgba.width(),rgba.height(),rgba.into_raw())}).map_err(|e|e.to_string())
 };
 match decoded{Ok(result)=>{println!("Loaded texture: {}",found.display());result},
  Err(e)=>{eprintln!("Texture unsupported {}: {e}",found.display());fallback()}}
}

fn data_root(path:&Path)->Result<std::path::PathBuf,String>{
 // Standalone games use Data/, while mods live under Mods/<mod name>/.
 for ancestor in path.ancestors(){
  let Some(name)=ancestor.file_name()else{continue};
  if name.to_string_lossy().eq_ignore_ascii_case("Data"){return Ok(ancestor.to_path_buf());}
  if ancestor.parent().and_then(|p|p.file_name()).is_some_and(|n|n.to_string_lossy().eq_ignore_ascii_case("Mods")){
   return Ok(ancestor.to_path_buf());
  }
 }
 Err("E4P must be inside Data/ or Mods/<mod name>/".into())
}
fn lookup_in_root(root:&Path,relative:&str)->Option<std::path::PathBuf>{
 let mut current=root.to_path_buf();
 for component in relative.split('/'){
  if component.is_empty()||component=="."{continue;}
  if component==".."||component.contains(':'){return None;}
  current=std::fs::read_dir(&current).ok()?.flatten().find(|e|
   e.file_name().to_string_lossy().eq_ignore_ascii_case(component))?.path();
 }
 if current.is_file() && current.canonicalize().ok()?.starts_with(root.canonicalize().ok()?){Some(current)}else{None}
}
fn resolve_game_ref(root:&Path,reference:&str)->Option<std::path::PathBuf>{
 let reference=reference.strip_prefix("mod:").unwrap_or(reference).trim_start_matches('/');
 let relative=reference.replace('\\',"/");
 if relative.is_empty()||relative.contains(':'){return None;}
 // Prefer mod overrides, falling back to original Data when an asset is not bundled.
 if let Some(path)=lookup_in_root(root,&relative){return Some(path);}
 let is_mod=root.parent().and_then(|p|p.file_name()).is_some_and(|n|n.to_string_lossy().eq_ignore_ascii_case("Mods"));
 if is_mod {
  let base=root.parent()?.parent()?.join("Data");
  if base.is_dir(){return lookup_in_root(&base,&relative);}
 }
 None
}

fn append_model(target:&mut em4_formats::v3o_mesh::Mesh,source:em4_formats::v3o_mesh::Mesh,
 attachment:Option<&em4_formats::e4p_assembly::Attachment>,transpose_rotation:bool){
 let vertex_start=target.vertices.len() as u32;
 let surface_start=target.surfaces.len();
 let mut verts=source.vertices;
 if let Some(a)=attachment {
  for v in &mut verts {
   let p=v.position;let r=&a.rotation;
   let matrix=|row:usize,col:usize|if transpose_rotation{r[col*3+row]}else{r[row*3+col]};
   // V3O coordinates are already converted in v3o_mesh; E4P attachment
   // offsets are in the same working coordinate units and must not be divided again.
   v.position=[
    matrix(0,0)*p[0]+matrix(0,1)*p[1]+matrix(0,2)*p[2]+a.position[0],
    matrix(1,0)*p[0]+matrix(1,1)*p[1]+matrix(1,2)*p[2]+a.position[1],
    matrix(2,0)*p[0]+matrix(2,1)*p[1]+matrix(2,2)*p[2]+a.position[2],
   ];
   let n=v.normal;
   v.normal=[matrix(0,0)*n[0]+matrix(0,1)*n[1]+matrix(0,2)*n[2],
    matrix(1,0)*n[0]+matrix(1,1)*n[1]+matrix(1,2)*n[2],
    matrix(2,0)*n[0]+matrix(2,1)*n[1]+matrix(2,2)*n[2]];
  }
 }
 target.vertices.extend(verts);
 target.surfaces.extend(source.surfaces);
 for mut t in source.triangles{
  for i in &mut t.indices{*i+=vertex_start;}
  t.surface_index+=surface_start;
  target.triangles.push(t);
 }
}
fn load_vehicle(path:&Path,transpose_rotation:bool)->Result<(em4_formats::v3o_mesh::Mesh,std::path::PathBuf,Vec<DoorPreview>),Box<dyn std::error::Error>>{
 let root=data_root(path)?;
 let prototype=em4_formats::e4p_assembly::parse_file(path)?;
 let body=prototype.body_model.ok_or("E4P does not contain a readable body ModelName")?;
 let body_path=resolve_game_ref(&root,&body).ok_or("Main V3O model reference cannot be located in Data")?;
 let mut mesh=em4_formats::v3o_mesh::parse(&fs::read(&body_path)?)?;
 let body_side_min=mesh.vertices.iter().map(|v|v.position[1]).fold(f32::INFINITY,f32::min);
 let body_side_max=mesh.vertices.iter().map(|v|v.position[1]).fold(f32::NEG_INFINITY,f32::max);
 let _body_mid_side=(body_side_min+body_side_max)*0.5;
 let body_half_width=(body_side_max-body_side_min)*0.5;
 let mut loaded=0usize;
 let mut doors=Vec::new();
 for child in &prototype.attachments {
  println!("Attachment {}: ({:.2}, {:.2}, {:.2}) original units, rotation {:?}", child.prototype,child.position[0],child.position[1],child.position[2],child.rotation);
  let Some(p)=resolve_game_ref(&root,&child.prototype)else{
   eprintln!("Missing child prototype: {}",child.prototype);continue;
  };
  let Ok(desc)=em4_formats::e4p_assembly::parse_file(&p)else{
   eprintln!("Cannot read child prototype: {}",p.display());continue;
  };
  let Some(model_ref)=desc.body_model else{eprintln!("No child V3O for {}",p.display());continue};
  let Some(model_path)=resolve_game_ref(&root,&model_ref)else{
   eprintln!("Missing child model: {model_ref}");continue;
  };
  match em4_formats::v3o_mesh::parse(&fs::read(&model_path)?) {
   Ok(mut part)=>{
    // Resolve child textures relative to their OWN model directory rather than the body.
    if let Some(dir)=model_path.parent(){for surface in &mut part.surfaces {
      let tex=std::path::Path::new(&surface.texture);
      if !tex.is_absolute() && !surface.texture.contains("..") {surface.texture=dir.join(tex).to_string_lossy().into_owned();}
    }}
    let start=mesh.vertices.len();
    append_model(&mut mesh,part,Some(child),transpose_rotation);
    if child.category=="door" {
     // Infer a vertical hinge from the closed door's world-space bounds, NOT
     // from the child attachment origin (which is not necessarily the hinge).
     let verts=&mesh.vertices[start..];
     let mut min=[f32::INFINITY;3];let mut max=[f32::NEG_INFINITY;3];
     for v in verts {for axis in 0..3{
      min[axis]=min[axis].min(v.position[axis]);max[axis]=max[axis].max(v.position[axis]);
     }}
     if verts.is_empty(){continue;}
     let center_x=(min[0]+max[0])*0.5;
     let door_side=(min[1]+max[1])*0.5;
     // Emergency 4 vehicles conventionally face +X. Front doors hinge
     // toward +X, rear doors toward -X; this remains a heuristic.
     let vehicle_middle_x=0.0f32;
     let front_door=center_x>=vehicle_middle_x;
     // The native E4P axis is not yet reliable for vehicle side doors.
     // Preserve the verified geometry hinge for off-centre cab/side doors.
     // Only try native horizontal-axis equipment when attached near the
     // vehicle centreline (typical rear ramp or tailgate).
     let lateral_span=max[1]-min[1];
     let wide_equipment=lateral_span>body_half_width*1.20;
     let native_axis=child.door_axis.map(|a|[a[0],a[2],-a[1]]);
     let horizontal=native_axis.is_some_and(|a|a[1].abs()<0.5);
     let is_mod=root.parent().and_then(|p|p.file_name()).is_some_and(|n|n.to_string_lossy().eq_ignore_ascii_case("Mods"));
     // Mod door parameters differ across authors. Keep native equipment axes
     // opt-in for mods until their hinge semantics are individually verified.
     let allow_native=!is_mod || env::args_os().any(|arg|arg=="--native-mod-door-axes");
     let use_native=allow_native&&horizontal&&wide_equipment&&child.open_angle.is_some();
     let direction=if door_side>=0.0{1.0}else{-1.0};
     let pivot_x=if front_door{max[0]}else{min[0]};
     let fallback_pivot=[pivot_x,(min[2]+max[2])*0.5,-door_side];
     // A horizontal equipment hinge uses its actual E4P child origin; mesh
     // bounds are not the articulated joint and cause detached-looking ramps.
     let pivot=if use_native{[child.position[0],child.position[2],-child.position[1]]}
       else{fallback_pivot};
     let axis=if use_native{native_axis.unwrap()}else{[0.,1.,0.]};
     let angle=if use_native{child.open_angle.unwrap().to_radians()}
       else{-direction*65.0f32.to_radians()};
     let native=use_native;
     println!("Door {}: pivot {:?} axis {:?} angle {:.1} source {} lateral_span {:.2} body_half_width {:.2}",
       child.prototype,pivot,axis,angle.to_degrees(),
       if native{"native wide equipment"}else{"vertical side-door fallback"},
       lateral_span,body_half_width);
     doors.push(DoorPreview{range:start..mesh.vertices.len(),pivot,direction,prototype:child.prototype.clone(),axis,angle,native});
    }
    loaded+=1;
   },
   Err(err)=>eprintln!("Child model failed {}: {err}",model_path.display()),
  }
 }
 println!("Resolved {} of {} child attachment(s)",loaded,prototype.attachments.len());
 Ok((mesh,body_path,doors))
}

fn main()->Result<(),Box<dyn std::error::Error>>{
 env_logger::init();
 let path=std::path::PathBuf::from(env::args_os().nth(1).ok_or("Usage: cargo run -p openem4 --bin em4-view -- <model.V3O>")?);
 let ext=path.extension().and_then(|e|e.to_str()).unwrap_or("").to_ascii_lowercase();
 if ext!="v3o"&&ext!="e4p"{
  return Err(format!("Unsupported input format .{}: expected a .v3o 3D model or .e4p vehicle prototype. PNG/DDS/TGA images are textures, not models.",if ext.is_empty(){"(none)"}else{&ext}).into());
 }
 if !path.is_file(){
  let hint=if ext=="e4p" {
   let path_text=path.to_string_lossy();
   let alternate=path_text.replace("\\Models\\","\\Prototypes\\").replace("/Models/","/Prototypes/");
   if alternate!=path_text && std::path::Path::new(&alternate).is_file(){
    format!("\\nFound a likely prototype instead: {alternate}")
   }else{
    "\\nE4P prototypes are usually under Mods/<mod>/Prototypes/Vehicles, not Models/Vehicles. Locate yours with PowerShell: Get-ChildItem <mod-folder> -Filter '*.e4p' -Recurse".to_string()
   }
  }else{String::new()};
  return Err(format!("Model file not found: {}{hint}",path.display()).into());
 }
 let transpose_rotation=env::args_os().any(|a|a=="--transpose-attachment-rotation");
 let upscale_factor=if env::args_os().any(|a|a=="--texture-upscale=off"){0}
 else if env::args_os().any(|a|a=="--texture-upscale=4"){4}else{1};
 if upscale_factor!=0 {
  if let Err(e)=texture_upscale::prune_cache(){eprintln!("Texture cache pruning skipped: {e}");}
 }
 println!("Texture enhancement: {}",match upscale_factor{0=>"disabled (original textures)",4=>"forced 4x Lanczos3 + sharpen25",_=>"automatic (conservative 2x/4x + external cache)"});
 let instance_count=env::args().find_map(|arg|arg.strip_prefix("--instances=").and_then(|n|n.parse::<usize>().ok())).unwrap_or(1);
 if !(1..=16).contains(&instance_count){return Err("--instances must be between 1 and 16".into());}
 if instance_count>1&&path.extension().is_some_and(|e|e.to_string_lossy().eq_ignore_ascii_case("e4p")){
  return Err("--instances currently supports standalone V3O meshes only, to preserve E4P attachment animations".into());
 }
 let extra_models=env::args_os().skip(2).filter_map(|arg|{
  let arg=arg.to_string_lossy();
  arg.strip_prefix("--add-v3o=").or_else(||arg.strip_prefix("--add-e4p=")).map(std::path::PathBuf::from)
 }).collect::<Vec<_>>();
 if !extra_models.is_empty()&&instance_count!=1{
  return Err("--add-v3o and --instances cannot currently be combined".into());
 }
 if extra_models.len()>15{return Err("maximum 16 scene objects".into());}
 let (mesh,body_path,doors,scene_ranges,scene_light_offsets)=if extra_models.is_empty()&&path.extension().is_some_and(|e|e.to_string_lossy().eq_ignore_ascii_case("e4p")) {
  {let (mesh,body,doors)=load_vehicle(&path,transpose_rotation)?;(mesh,body,doors,Vec::new(),Vec::new())}
 }else if extra_models.is_empty(){
  (scene_mesh::repeated_v3o(em4_formats::v3o_mesh::parse(&fs::read(&path)?)?,instance_count)?,path.clone(),Vec::new(),Vec::new(),Vec::new())
 }else{
  let paths=std::iter::once(path.clone()).chain(extra_models).collect::<Vec<_>>();
  let mut models=Vec::new();
  for model_path in &paths {
   let is_e4p=model_path.extension().is_some_and(|e|e.to_string_lossy().eq_ignore_ascii_case("e4p"));
   let is_v3o=model_path.extension().is_some_and(|e|e.to_string_lossy().eq_ignore_ascii_case("v3o"));
   if !is_e4p&&!is_v3o{return Err(format!("Scene model must be .v3o or .e4p: {}",model_path.display()).into());}
   let (mut mesh,texture_base)=if is_e4p{
    let (mesh,body_path,_doors)=load_vehicle(model_path,transpose_rotation)?;
    println!("Static E4P scene preview: {} (attached geometry loaded; independent doors/lights not active)",model_path.display());
    (mesh,body_path)
   }else{(em4_formats::v3o_mesh::parse(&fs::read(model_path)?)?,model_path.clone())};
   for surface in &mut mesh.surfaces{
    let texture=surface.texture.replace('\\',"/");
    if texture.trim().is_empty(){continue;}
    let resolved=if Path::new(&texture).is_absolute(){
     Some(std::path::PathBuf::from(&texture))
    }else if let Some(game_ref)=texture.strip_prefix("mod:"){
     data_root(model_path).ok().and_then(|root|resolve_game_ref(&root,game_ref))
    }else{
     texture_base.parent().and_then(|folder|lookup_in_root(folder,&texture))
      .or_else(||data_root(model_path).ok().and_then(|root|resolve_game_ref(&root,&texture)))
    };
    if let Some(found)=resolved {surface.texture=found.to_string_lossy().into_owned();}
    else {eprintln!("Scene texture not resolved for {}: {}",model_path.display(),texture);}
   }
   models.push(mesh);
  }
  println!("Mixed scene: {} independently rendered V3O/E4P models",models.len());
  let layout_offsets=scene_mesh::preview_layout_offsets(&models);
  let mut vertex_start=0usize;
  let entity_ranges=models.iter().map(|model|{let start=vertex_start;vertex_start+=model.vertices.len();start..vertex_start}).collect::<Vec<_>>();
  let objects=models.into_iter().enumerate().map(|(id,mesh)|{
   let entity=scene_entities::SceneEntity{id:scene_entities::EntityId(id as u64+1),name:paths[id].display().to_string(),transform:scene_entities::Transform::default()};
   println!("Scene entity {}: {}",entity.id.0,entity.name);
   (entity,mesh)
  }).collect();
  (scene_mesh::combine_entities(objects)?,path.clone(),Vec::new(),entity_ranges,layout_offsets)
 };
 println!("Scene instances: {} (shared material surfaces)",instance_count);
 println!("Loading {} vertices, {} triangles, {} surfaces",mesh.vertices.len(),mesh.triangles.len(),mesh.surfaces.len());
 let event_loop=EventLoop::new()?;
 let window=Arc::new(WindowBuilder::new().with_title("openEM4 V3O Viewer | LMB Orbit | RMB Pan | Wheel Zoom | R Reset")
  .with_inner_size(winit::dpi::LogicalSize::new(1200.0,800.0)).build(&event_loop)?);
 let mut light_defs=Vec::new();
 let mut light_sources=Vec::new();
 if scene_ranges.is_empty(){
  if path.extension().is_some_and(|e|e.to_string_lossy().eq_ignore_ascii_case("e4p")){
   let parsed=em4_formats::e4p_lights::parse(&fs::read(&path)?);
   light_sources.extend(std::iter::repeat(0usize).take(parsed.len()));
   light_defs.extend(parsed);
  }
 }else{
  let source_paths=std::iter::once(path.clone()).chain(env::args_os().skip(2).filter_map(|arg|{
   let arg=arg.to_string_lossy();
   arg.strip_prefix("--add-v3o=").or_else(||arg.strip_prefix("--add-e4p=")).map(std::path::PathBuf::from)
  })).collect::<Vec<_>>();
  for (source,path) in source_paths.iter().enumerate(){
   if path.extension().is_some_and(|e|e.to_string_lossy().eq_ignore_ascii_case("e4p")){
    let mut parsed=em4_formats::e4p_lights::parse(&fs::read(path)?);
    if let Some(shift)=scene_light_offsets.get(source){
     for light in &mut parsed{for i in 0..3{light.position[i]+=shift[i];}}
    }
    light_sources.extend(std::iter::repeat(source+1).take(parsed.len()));
    light_defs.extend(parsed);
   }
  }
 }
 for (i,l) in light_defs.iter().enumerate(){println!("E4P light {i}: anchor={:?} polygon_points={} corona={} type={} flash_frequency={} flash_offset={}",l.position,l.points.len(),l.corona_texture,l.light_type,l.flash_frequency,l.flash_offset);}
 let mut viewer=pollster::block_on(Viewer::new(window.clone(),&mesh,&body_path,&light_defs,doors,upscale_factor,scene_ranges,light_sources))?;
 event_loop.set_control_flow(ControlFlow::Wait);
 event_loop.run(move |event,target|{
  if let Event::AboutToWait=event {
   target.set_control_flow(if viewer.enhancement_done<viewer.enhancement_total{ControlFlow::WaitUntil(Instant::now()+std::time::Duration::from_millis(33))}else{ControlFlow::Wait});
   let upgraded=viewer.receive_enhancements();
   viewer.queue_evicted_restorations();
   viewer.dispatch_enhancements();
   let door_moving=viewer.tick_doors();
   if upgraded||door_moving||viewer.lights.beacon||viewer.lights.left||viewer.lights.right {window.request_redraw();}
  }
  if let Event::WindowEvent{window_id,event}=event {
   if window_id!=window.id(){return;}
   match event {
    WindowEvent::CloseRequested=>target.exit(),
    WindowEvent::Resized(s)=>{viewer.resize(s.width,s.height);window.request_redraw();}
    WindowEvent::RedrawRequested=>{
     match viewer.render(){
      Ok(())=>{},
      Err(wgpu::SurfaceError::Lost|wgpu::SurfaceError::Outdated)=>{
       let s=window.inner_size();viewer.resize(s.width,s.height);
      },
      Err(wgpu::SurfaceError::OutOfMemory)=>target.exit(),
      Err(wgpu::SurfaceError::Timeout)=>eprintln!("GPU frame timeout"),
     }
    }
    WindowEvent::CursorMoved{position,..}=>{viewer.mouse_motion(position.x,position.y);window.request_redraw();}
    WindowEvent::MouseInput{button,state,..}=>{
     if matches!(button,MouseButton::Left|MouseButton::Right){
      if state==ElementState::Pressed{viewer.drag=Some(button);}else if viewer.drag==Some(button){viewer.drag=None;}
     }
    }
    WindowEvent::MouseWheel{delta,..}=>{
     let amount=match delta{MouseScrollDelta::LineDelta(_,y)=>y,MouseScrollDelta::PixelDelta(p)=>(p.y/60.0) as f32};
     viewer.camera.distance=(viewer.camera.distance*(-amount*0.12).exp()).clamp(0.05,20000.0);
     window.request_redraw();
    }
    WindowEvent::KeyboardInput{event,..}=>{
     if event.state==ElementState::Pressed {
      match event.physical_key {
       PhysicalKey::Code(KeyCode::KeyY)=>{viewer.parent_to_last_spawned();window.request_redraw();},
       PhysicalKey::Code(KeyCode::KeyP)=>{viewer.toggle_parent_source();window.request_redraw();},
       PhysicalKey::Code(KeyCode::KeyN)=>{viewer.spawn_selected_instance();window.request_redraw();},
       PhysicalKey::Code(KeyCode::Delete)=>{viewer.remove_selected_instance();window.request_redraw();},
       PhysicalKey::Code(KeyCode::KeyQ)=>{viewer.rotate_scene_entity(-15.0);window.request_redraw();},
       PhysicalKey::Code(KeyCode::KeyT)=>{viewer.rotate_scene_entity(15.0);window.request_redraw();},
       PhysicalKey::Code(KeyCode::Tab)=>{viewer.select_next_scene_entity();window.request_redraw();},
       PhysicalKey::Code(KeyCode::ArrowLeft)=>{viewer.move_scene_entity(0,-0.5);window.request_redraw();},
       PhysicalKey::Code(KeyCode::ArrowRight)=>{viewer.move_scene_entity(0,0.5);window.request_redraw();},
       PhysicalKey::Code(KeyCode::ArrowUp)=>{viewer.move_scene_entity(2,-0.5);window.request_redraw();},
       PhysicalKey::Code(KeyCode::ArrowDown)=>{viewer.move_scene_entity(2,0.5);window.request_redraw();},
       PhysicalKey::Code(KeyCode::PageUp)=>{viewer.move_scene_entity(1,0.5);window.request_redraw();},
       PhysicalKey::Code(KeyCode::PageDown)=>{viewer.move_scene_entity(1,-0.5);window.request_redraw();},
       PhysicalKey::Code(KeyCode::KeyR)=>{viewer.camera=viewer.initial_camera;window.request_redraw();},
       PhysicalKey::Code(KeyCode::KeyW)=>{viewer.debug_normals=!viewer.debug_normals;window.request_redraw();},
       PhysicalKey::Code(KeyCode::KeyG)=>{viewer.show_ground=!viewer.show_ground;window.request_redraw();},
       PhysicalKey::Code(KeyCode::KeyH)=>{let lights=viewer.selected_lights_mut();lights.headlights=!lights.headlights;println!("Selected headlights: {}",lights.headlights);window.request_redraw();},
       PhysicalKey::Code(KeyCode::KeyB)=>{let lights=viewer.selected_lights_mut();lights.beacon=!lights.beacon;println!("Selected beacons: {}",lights.beacon);window.request_redraw();},
       PhysicalKey::Code(KeyCode::KeyL)=>{let lights=viewer.selected_lights_mut();lights.left=!lights.left;println!("Selected left indicator: {}",lights.left);window.request_redraw();},
       PhysicalKey::Code(KeyCode::KeyK)=>{let lights=viewer.selected_lights_mut();lights.right=!lights.right;println!("Selected right indicator: {}",lights.right);window.request_redraw();},
       PhysicalKey::Code(KeyCode::Digit0)=>viewer.select_door(0),
       PhysicalKey::Code(KeyCode::Digit1)=>viewer.select_door(1),
       PhysicalKey::Code(KeyCode::Digit2)=>viewer.select_door(2),
       PhysicalKey::Code(KeyCode::Digit3)=>viewer.select_door(3),
       PhysicalKey::Code(KeyCode::Digit4)=>viewer.select_door(4),
       PhysicalKey::Code(KeyCode::Digit5)=>viewer.select_door(5),
       PhysicalKey::Code(KeyCode::Digit6)=>viewer.select_door(6),
       PhysicalKey::Code(KeyCode::Digit7)=>viewer.select_door(7),
       PhysicalKey::Code(KeyCode::Digit8)=>viewer.select_door(8),
       PhysicalKey::Code(KeyCode::Digit9)=>viewer.select_door(9),
       PhysicalKey::Code(KeyCode::KeyJ)=>viewer.debug_hinges(),
       PhysicalKey::Code(KeyCode::KeyD)=>{viewer.toggle_doors();window.request_redraw();},
       PhysicalKey::Code(KeyCode::KeyO)=>{viewer.corona_occlusion=!viewer.corona_occlusion;println!("Corona depth occlusion: {}",viewer.corona_occlusion);window.request_redraw();},
       PhysicalKey::Code(KeyCode::KeyE)=>{viewer.lights.beacon=!viewer.lights.beacon;println!("Beacon: {}",viewer.lights.beacon);window.request_redraw();},
       _=>{}
      }
     }
    }
    _=>{}
   }
  }
 })?;
 Ok(())
}
