//! Standalone prototype 3D V3O viewer. Geometry is shaded without textures.
//! Left mouse: orbit; right mouse: pan; wheel: zoom; R: reset.
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
  let projection=[
   [f/aspect.max(0.01),0.0,0.0,0.0],
   [0.0,f,0.0,0.0],
   [0.0,0.0,-1.00005,-1.0],
   [0.0,0.0,-0.0500025,0.0],
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
struct Depth { texture:wgpu::Texture,view:wgpu::TextureView }
fn depth(device:&wgpu::Device,w:u32,h:u32)->Depth {
 let texture=device.create_texture(&wgpu::TextureDescriptor {
  label:Some("V3O depth"),size:wgpu::Extent3d{width:w.max(1),height:h.max(1),depth_or_array_layers:1},
  mip_level_count:1,sample_count:1,dimension:wgpu::TextureDimension::D2,format:wgpu::TextureFormat::Depth24Plus,
  usage:wgpu::TextureUsages::RENDER_ATTACHMENT,view_formats:&[],
 });
 let view=texture.create_view(&Default::default());Depth{texture,view}
}
#[derive(Default)]
struct LightControls { headlights:bool, brake:bool, left:bool, right:bool, beacon:bool }
impl LightControls {
 fn phase(&self,seconds:f32)->[u32;4] {
  let indicator=(seconds*1.5).fract()<0.5;
  let emergency=(seconds*3.0).fract()<0.5;
  [u32::from(self.headlights),u32::from(self.brake),
   u32::from(indicator&&(self.left||self.right)),u32::from(self.beacon&&emergency)]
 }
}
struct Viewer {
 surface:wgpu::Surface<'static>,device:wgpu::Device,queue:wgpu::Queue,config:wgpu::SurfaceConfiguration,
 pipeline:wgpu::RenderPipeline,cutout_pipeline:wgpu::RenderPipeline,transparent_pipeline:wgpu::RenderPipeline,vertices:wgpu::Buffer,indices:wgpu::Buffer,index_count:u32,draws:Vec<(std::ops::Range<u32>,wgpu::BindGroup,u8,[f32;3])>,
 uniform:wgpu::Buffer,bind:wgpu::BindGroup,depth:Depth,camera:Camera,
 initial_camera:Camera,drag:Option<MouseButton>,cursor:Option<(f64,f64)>,debug_normals:bool,
 ground_vertex:wgpu::Buffer,ground_index:wgpu::Buffer,ground_material:wgpu::BindGroup,show_ground:bool,
 marker_pipeline:wgpu::RenderPipeline,marker_vertex:wgpu::Buffer,marker_index:wgpu::Buffer,
 light_markers:Vec<(std::ops::Range<u32>,u8,f32,f32)>,
 lights:LightControls,started:Instant,
}
impl Viewer {
 async fn new(window:Arc<winit::window::Window>,mesh:&em4_formats::v3o_mesh::Mesh,model_path:&Path,light_defs:&[em4_formats::e4p_lights::Light])->Result<Self,String>{
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
  let mut indices:Vec<u32>=Vec::with_capacity(mesh.triangles.len()*3);
  let mut ranges=Vec::new();
  for surface in 0..mesh.surfaces.len(){
   let start=indices.len() as u32;
   for t in mesh.triangles.iter().filter(|t|t.surface_index==surface){indices.extend_from_slice(&t.indices);}
   ranges.push(start..indices.len() as u32);
  }
  let instance=wgpu::Instance::default();
  let surface=instance.create_surface(window.clone()).map_err(|e|e.to_string())?;
  let adapter=instance.request_adapter(&wgpu::RequestAdapterOptions{
   power_preference:wgpu::PowerPreference::HighPerformance,compatible_surface:Some(&surface),force_fallback_adapter:false
  }).await.ok_or("No compatible GPU adapter")?;
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
  let sampler=device.create_sampler(&wgpu::SamplerDescriptor{address_mode_u:wgpu::AddressMode::Repeat,address_mode_v:wgpu::AddressMode::Repeat,
   mag_filter:wgpu::FilterMode::Linear,min_filter:wgpu::FilterMode::Linear,mipmap_filter:wgpu::FilterMode::Linear,..Default::default()});
  let mut draws=Vec::new();
  for (i,surface) in mesh.surfaces.iter().enumerate(){
   let (w,h,pixels)=load_surface_image(model_path,&surface.texture);
   let alpha:Vec<u8>=pixels.chunks_exact(4).map(|pixel|pixel[3]).collect();
   let (min_alpha,max_alpha)=alpha.iter().fold((255u8,0u8),|(lo,hi),&a|(lo.min(a),hi.max(a)));
   let has_alpha=alpha.iter().any(|&a|a<250);
   let binary_alpha=alpha.iter().all(|&a|a<24||a>231);
   let material_kind=if !has_alpha{0u8}else if binary_alpha{1u8}else{2u8};
   println!("Material {} {} texture={} mode={} size={}x{} alpha={}..{}",i,surface.name,surface.texture,match material_kind{0=>"opaque",1=>"cutout",_=>"blended"},w,h,min_alpha,max_alpha);
   let levels = 32 - w.max(h).leading_zeros();
   let texture=device.create_texture(&wgpu::TextureDescriptor{label:Some("V3O material"),
    size:wgpu::Extent3d{width:w,height:h,depth_or_array_layers:1},mip_level_count:levels,sample_count:1,
    dimension:wgpu::TextureDimension::D2,format:wgpu::TextureFormat::Rgba8UnormSrgb,
    usage:wgpu::TextureUsages::TEXTURE_BINDING|wgpu::TextureUsages::COPY_DST,view_formats:&[]});
   let mut image=image::RgbaImage::from_raw(w,h,pixels).ok_or("DDS decoder returned invalid RGBA length")?;
   for mip in 0..levels {
    let mw=image.width();let mh=image.height();
    queue.write_texture(wgpu::ImageCopyTexture{texture:&texture,mip_level:mip,origin:wgpu::Origin3d::ZERO,aspect:wgpu::TextureAspect::All},
     image.as_raw(),wgpu::ImageDataLayout{offset:0,bytes_per_row:Some(mw*4),rows_per_image:Some(mh)},
     wgpu::Extent3d{width:mw,height:mh,depth_or_array_layers:1});
    if mip+1<levels {
     image=image::imageops::resize(&image,(mw/2).max(1),(mh/2).max(1),image::imageops::FilterType::Triangle);
    }
   }
   let view=texture.create_view(&Default::default());
   let group=device.create_bind_group(&wgpu::BindGroupDescriptor{label:Some("V3O surface bind"),layout:&material_layout,
    entries:&[wgpu::BindGroupEntry{binding:0,resource:wgpu::BindingResource::TextureView(&view)},
     wgpu::BindGroupEntry{binding:1,resource:wgpu::BindingResource::Sampler(&sampler)}]});
   let mut center=[0.0f32;3];let mut count=0f32;
   for t in mesh.triangles.iter().filter(|t|t.surface_index==i){
    for &v in &t.indices {let p=vertices[v as usize].pos;for axis in 0..3{center[axis]+=p[axis];}count+=1.0;}
   }
   if count>0.0 {for x in &mut center{*x/=count;}}
   draws.push((ranges[i].clone(),group,material_kind,center));
  }
  let shader=device.create_shader_module(wgpu::ShaderModuleDescriptor{
   label:Some("V3O shader"),source:wgpu::ShaderSource::Wgsl(include_str!("em4-view.wgsl").into())});
  let pipeline_layout=device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor{
   label:None,bind_group_layouts:&[&layout,&material_layout],push_constant_ranges:&[]});
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
  let radius=(diagonal*0.014).max(0.04);
  for light in light_defs {
   let tex=light.corona_texture.to_ascii_lowercase();
   let kind:u8=if tex.contains("blue"){1}else if tex.contains("red"){0}
    else if tex.contains("yellow"){3}else{2};
   let id=match kind{0=>0.1,1=>0.3,2=>0.5,_=>0.8};
   let start=mi.len() as u32;
   // Diagnostic mode: each decoded source point gets its own small marker.
   // Never connect unrelated points into a potentially enormous filled polygon.
   for point in &light.points {
    let p=[point[0],point[2],-point[1]];
    let base=mv.len() as u32;
    for (dx,dz) in [(-1.0,-1.0),(1.0,-1.0),(1.0,1.0),(-1.0,1.0)] {
     mv.push(Vertex{pos:[p[0]+dx*radius,p[1],p[2]+dz*radius],
      normal:[0.0,1.0,0.0],uv:[id,0.0]});
    }
    mi.extend_from_slice(&[base,base+1,base+2,base,base+2,base+3]);
   }
   let end=mi.len() as u32;
   if end>start{light_markers.push((start..end,kind,light.flash_frequency,light.flash_offset));}
  }
  println!("Loaded {} original E4P light definitions",light_markers.len());
  // GPU buffers require non-empty data even when the prototype has no lights.
  if mv.is_empty() {mv.push(Vertex{pos:[0.;3],normal:[0.,1.,0.],uv:[0.,0.]});mi.push(0);}
  let marker_vertex=device.create_buffer_init(&wgpu::util::BufferInitDescriptor{
   label:Some("preview light verts"),contents:bytemuck::cast_slice(&mv),usage:wgpu::BufferUsages::VERTEX});
  let marker_index=device.create_buffer_init(&wgpu::util::BufferInitDescriptor{
   label:Some("preview light indices"),contents:bytemuck::cast_slice(&mi),usage:wgpu::BufferUsages::INDEX});
  let marker_pipeline=device.create_render_pipeline(&wgpu::RenderPipelineDescriptor{
   label:Some("preview light pipeline"),layout:Some(&pipeline_layout),
   vertex:wgpu::VertexState{module:&shader,entry_point:"vs_main",buffers:&[Vertex::desc()],compilation_options:Default::default()},
   fragment:Some(wgpu::FragmentState{module:&shader,entry_point:"fs_emissive",
    targets:&[Some(wgpu::ColorTargetState{format,blend:None,write_mask:wgpu::ColorWrites::ALL})],compilation_options:Default::default()}),
   primitive:wgpu::PrimitiveState{cull_mode:None,..Default::default()},
   depth_stencil:Some(wgpu::DepthStencilState{format:wgpu::TextureFormat::Depth24Plus,
    depth_write_enabled:false,depth_compare:wgpu::CompareFunction::LessEqual,stencil:Default::default(),bias:Default::default()}),
   multisample:Default::default(),multiview:None
  });
  let vertex_buffer=device.create_buffer_init(&wgpu::util::BufferInitDescriptor{
   label:Some("V3O vertices"),contents:bytemuck::cast_slice(&vertices),usage:wgpu::BufferUsages::VERTEX});
  let index_buffer=device.create_buffer_init(&wgpu::util::BufferInitDescriptor{
   label:Some("V3O triangles"),contents:bytemuck::cast_slice(&indices),usage:wgpu::BufferUsages::INDEX});
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
  Ok(Self{surface,device,queue,config,pipeline,cutout_pipeline,transparent_pipeline,vertices:vertex_buffer,indices:index_buffer,
   index_count:indices.len() as u32,draws,uniform,bind,depth,camera,initial_camera:camera,drag:None,cursor:None,debug_normals:false,ground_vertex,ground_index,ground_material,show_ground:true,marker_pipeline,marker_vertex,marker_index,light_markers,lights:LightControls::default(),started:Instant::now()})
 }
 fn resize(&mut self,w:u32,h:u32){
  if w==0||h==0{return;}self.config.width=w;self.config.height=h;
  self.surface.configure(&self.device,&self.config);self.depth=depth(&self.device,w,h);
 }
 fn render(&mut self)->Result<(),wgpu::SurfaceError>{
  let matrix=self.camera.matrix(self.config.width as f32/self.config.height as f32);
  self.queue.write_buffer(&self.uniform,0,bytemuck::bytes_of(&Uniform{matrix,debug:[u32::from(self.debug_normals),self.lights.phase(self.started.elapsed().as_secs_f32())[0],self.lights.phase(self.started.elapsed().as_secs_f32())[2],self.lights.phase(self.started.elapsed().as_secs_f32())[3]]}));
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
   pass.set_pipeline(&self.pipeline);pass.set_bind_group(0,&self.bind,&[]);
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
   for (range,material,kind,_) in &self.draws {
    if *kind!=0 {continue;}
    pass.set_bind_group(1,material,&[]);pass.draw_indexed(range.clone(),0,0..1);
   }
   pass.set_pipeline(&self.cutout_pipeline);
   for (range,material,kind,_) in &self.draws {
    if *kind!=1 {continue;}
    pass.set_bind_group(1,material,&[]);pass.draw_indexed(range.clone(),0,0..1);
   }
   let (sy,cy)=self.camera.yaw.sin_cos();let (sp,cp)=self.camera.pitch.sin_cos();
   let eye=[self.camera.target[0]+self.camera.distance*cp*sy,self.camera.target[1]+self.camera.distance*sp,self.camera.target[2]+self.camera.distance*cp*cy];
   let mut blended:Vec<usize>=self.draws.iter().enumerate().filter(|(_,d)|d.2==2).map(|(i,_)|i).collect();
   blended.sort_by(|&a,&b|{
    let da=dot(sub(self.draws[a].3,eye),sub(self.draws[a].3,eye));
    let db=dot(sub(self.draws[b].3,eye),sub(self.draws[b].3,eye));
    db.total_cmp(&da)
   });
   pass.set_pipeline(&self.transparent_pipeline);
   for i in blended {
    let (range,material,_,_)=&self.draws[i];
    pass.set_bind_group(1,material,&[]);pass.draw_indexed(range.clone(),0,0..1);
   }
   // Data-driven diagnostic markers: position, colour and pulse from source E4P.
   let t=self.started.elapsed().as_secs_f32();
   pass.set_pipeline(&self.marker_pipeline);
   pass.set_bind_group(1,&self.ground_material,&[]);
   pass.set_vertex_buffer(0,self.marker_vertex.slice(..));
   pass.set_index_buffer(self.marker_index.slice(..),wgpu::IndexFormat::Uint32);
   for (range,kind,frequency,offset) in &self.light_markers {
    let pulse=*frequency<=0.0 || ((t*frequency+offset).fract()<0.5);
    let enabled=match kind {
     0|1=>self.lights.beacon||self.lights.brake,
     2=>self.lights.headlights,
     _=>self.lights.left||self.lights.right
    };
    if enabled&&pulse {pass.draw_indexed(range.clone(),0,0..1);}
   }
  }
  self.queue.submit(Some(encoder.finish()));frame.present();Ok(())
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

fn load_surface_image(model_path:&Path,filename:&str)->(u32,u32,Vec<u8>){
 let fallback=|| (2,2,vec![220,220,220,255,120,120,120,255,120,120,120,255,220,220,220,255]);
 if filename.is_empty()||filename.contains(".."){return fallback();}
 let base=match model_path.parent(){Some(p)=>p,None=>return fallback()};
 let requested=filename.replace('\\',"/");
 let mut folder=if Path::new(filename).is_absolute(){
  let path=Path::new(filename);
  match fs::read(path){Ok(bytes)=>{
   let decoded=if filename.to_ascii_lowercase().ends_with(".dds"){
    em4_formats::dds::decode_dds(&bytes).map(|im|(im.width,im.height,im.pixels)).map_err(|e|e.to_string())
   }else{image::load_from_memory(&bytes).map(|im|{let rgba=im.to_rgba8();(rgba.width(),rgba.height(),rgba.into_raw())}).map_err(|e|e.to_string())};
   return decoded.unwrap_or_else(|_|fallback());
  },Err(_)=>return fallback()}
 }else{base.to_path_buf()};
 for component in requested.split('/'){
  if component.is_empty()||component=="."{continue;}
  let entry=match std::fs::read_dir(&folder).ok().and_then(|entries|entries.flatten().find(|e|e.file_name().to_string_lossy().eq_ignore_ascii_case(component))){
   Some(e)=>e,None=>{eprintln!("Missing texture: {filename}");return fallback();}
  };
  folder=entry.path();
 }
 let bytes=match fs::read(&folder){Ok(b)=>b,Err(e)=>{eprintln!("Texture read failed {filename}: {e}");return fallback();}};
 let decoded=if filename.to_ascii_lowercase().ends_with(".dds"){
  em4_formats::dds::decode_dds(&bytes).map(|im|(im.width,im.height,im.pixels)).map_err(|e|e.to_string())
 }else{
  image::load_from_memory(&bytes).map(|im|{let rgba=im.to_rgba8();(rgba.width(),rgba.height(),rgba.into_raw())}).map_err(|e|e.to_string())
 };
 match decoded {Ok(result)=>{println!("Loaded texture: {filename}");result},
  Err(e)=>{eprintln!("Texture unsupported {filename}: {e}");fallback()}}
}


fn data_root(path:&Path)->Result<std::path::PathBuf,String>{
 path.ancestors().find(|p|p.file_name().map(|n|n.to_string_lossy().eq_ignore_ascii_case("Data")).unwrap_or(false))
 .map(|p|p.to_path_buf()).ok_or("E4P must be within an Emergency 4 Data directory".into())
}
fn resolve_game_ref(root:&Path,reference:&str)->Option<std::path::PathBuf>{
 let reference=reference.strip_prefix("mod:").unwrap_or(reference).trim_start_matches('/');
 let relative=reference.replace('\\',"/");
 if relative.is_empty()||relative.contains(':'){return None;}
 let mut current=root.to_path_buf();
 for component in relative.split('/'){
  if component.is_empty()||component=="."{continue;}
  if component==".."{return None;}
  current=std::fs::read_dir(&current).ok()?.flatten().find(|e|
   e.file_name().to_string_lossy().eq_ignore_ascii_case(component))?.path();
 }
 if current.is_file() && current.canonicalize().ok()?.starts_with(root.canonicalize().ok()?){Some(current)}else{None}
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
fn load_vehicle(path:&Path,transpose_rotation:bool)->Result<(em4_formats::v3o_mesh::Mesh,std::path::PathBuf),Box<dyn std::error::Error>>{
 let root=data_root(path)?;
 let prototype=em4_formats::e4p_assembly::parse_file(path)?;
 let body=prototype.body_model.ok_or("E4P does not contain a readable body ModelName")?;
 let body_path=resolve_game_ref(&root,&body).ok_or("Main V3O model reference cannot be located in Data")?;
 let mut mesh=em4_formats::v3o_mesh::parse(&fs::read(&body_path)?)?;
 let mut loaded=0usize;
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
    append_model(&mut mesh,part,Some(child),transpose_rotation);loaded+=1;
   },
   Err(err)=>eprintln!("Child model failed {}: {err}",model_path.display()),
  }
 }
 println!("Resolved {} of {} child attachment(s)",loaded,prototype.attachments.len());
 Ok((mesh,body_path))
}

fn main()->Result<(),Box<dyn std::error::Error>>{
 env_logger::init();
 let path=std::path::PathBuf::from(env::args_os().nth(1).ok_or("Usage: cargo run -p openem4 --bin em4-view -- <model.V3O>")?);
 let transpose_rotation=env::args_os().any(|a|a=="--transpose-attachment-rotation");
 let (mesh,body_path)=if path.extension().is_some_and(|e|e.to_string_lossy().eq_ignore_ascii_case("e4p")) {
  load_vehicle(&path,transpose_rotation)?
 }else{(em4_formats::v3o_mesh::parse(&fs::read(&path)?)?,path.clone())};
 println!("Loading {} vertices, {} triangles, {} surfaces",mesh.vertices.len(),mesh.triangles.len(),mesh.surfaces.len());
 let event_loop=EventLoop::new()?;
 let window=Arc::new(WindowBuilder::new().with_title("openEM4 V3O Viewer | LMB Orbit | RMB Pan | Wheel Zoom | R Reset")
  .with_inner_size(winit::dpi::LogicalSize::new(1200.0,800.0)).build(&event_loop)?);
 let light_defs=if path.extension().is_some_and(|e|e.to_string_lossy().eq_ignore_ascii_case("e4p")) {
 em4_formats::e4p_lights::parse(&fs::read(&path)?)
}else{Vec::new()};
 for (i,l) in light_defs.iter().enumerate(){println!("E4P light {i}: pos={:?} corona={} type={} frequency={} offset={}",l.position,l.corona_texture,l.light_type,l.flash_frequency,l.flash_offset);}
 let mut viewer=pollster::block_on(Viewer::new(window.clone(),&mesh,&body_path,&light_defs))?;
 event_loop.set_control_flow(ControlFlow::Wait);
 event_loop.run(move |event,target|{
  if let Event::AboutToWait=event {
   if viewer.lights.beacon||viewer.lights.left||viewer.lights.right {window.request_redraw();}
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
       PhysicalKey::Code(KeyCode::KeyR)=>{viewer.camera=viewer.initial_camera;window.request_redraw();},
       PhysicalKey::Code(KeyCode::KeyW)=>{viewer.debug_normals=!viewer.debug_normals;window.request_redraw();},
       PhysicalKey::Code(KeyCode::KeyG)=>{viewer.show_ground=!viewer.show_ground;window.request_redraw();},
       PhysicalKey::Code(KeyCode::KeyH)=>{viewer.lights.headlights=!viewer.lights.headlights;println!("Headlights: {}",viewer.lights.headlights);window.request_redraw();},
       PhysicalKey::Code(KeyCode::KeyB)=>{viewer.lights.brake=!viewer.lights.brake;println!("Brake lights: {}",viewer.lights.brake);window.request_redraw();},
       PhysicalKey::Code(KeyCode::KeyL)=>{viewer.lights.left=!viewer.lights.left;println!("Left indicator: {}",viewer.lights.left);window.request_redraw();},
       PhysicalKey::Code(KeyCode::KeyK)=>{viewer.lights.right=!viewer.lights.right;println!("Right indicator: {}",viewer.lights.right);window.request_redraw();},
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
