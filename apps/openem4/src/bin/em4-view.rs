//! Standalone prototype 3D V3O viewer. Geometry is shaded without textures.
//! Left mouse: orbit; right mouse: pan; wheel: zoom; R: reset.
use std::{env, fs, mem, sync::Arc};
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
struct Uniform { matrix:[[f32;4];4] }
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
struct Viewer {
 surface:wgpu::Surface<'static>,device:wgpu::Device,queue:wgpu::Queue,config:wgpu::SurfaceConfiguration,
 pipeline:wgpu::RenderPipeline,vertices:wgpu::Buffer,indices:wgpu::Buffer,index_count:u32,
 uniform:wgpu::Buffer,bind:wgpu::BindGroup,depth:Depth,camera:Camera,
 initial_camera:Camera,drag:Option<MouseButton>,cursor:Option<(f64,f64)>,
}
impl Viewer {
 async fn new(window:Arc<winit::window::Window>,mesh:&em4_formats::v3o_mesh::Mesh)->Result<Self,String>{
  if mesh.vertices.is_empty()||mesh.triangles.is_empty(){return Err("V3O model contains no renderable triangles".into());}
  let mut min=[f32::INFINITY;3];let mut max=[f32::NEG_INFINITY;3];
  let vertices:Vec<Vertex>=mesh.vertices.iter().map(|v|{
   for i in 0..3 {min[i]=min[i].min(v.position[i]);max[i]=max[i].max(v.position[i]);}
   Vertex{pos:v.position,normal:v.normal,uv:v.uv}
  }).collect();
  let target=[(min[0]+max[0])*0.5,(min[1]+max[1])*0.5,(min[2]+max[2])*0.5];
  let diagonal=((max[0]-min[0]).powi(2)+(max[1]-min[1]).powi(2)+(max[2]-min[2]).powi(2)).sqrt();
  let camera=Camera::reset(target,(diagonal*1.5).max(2.0));
  let indices:Vec<u32>=mesh.triangles.iter().flat_map(|t|t.indices).collect();
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
   contents:bytemuck::bytes_of(&Uniform{matrix:camera.matrix(config.width as f32/config.height as f32)}),
   usage:wgpu::BufferUsages::UNIFORM|wgpu::BufferUsages::COPY_DST});
  let layout=device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor{
   label:Some("camera bind layout"),entries:&[wgpu::BindGroupLayoutEntry{
    binding:0,visibility:wgpu::ShaderStages::VERTEX,ty:wgpu::BindingType::Buffer{
     ty:wgpu::BufferBindingType::Uniform,has_dynamic_offset:false,min_binding_size:None},count:None
   }]});
  let bind=device.create_bind_group(&wgpu::BindGroupDescriptor{label:Some("camera bind"),
   layout:&layout,entries:&[wgpu::BindGroupEntry{binding:0,resource:uniform.as_entire_binding()}]});
  let shader=device.create_shader_module(wgpu::ShaderModuleDescriptor{
   label:Some("V3O shader"),source:wgpu::ShaderSource::Wgsl(include_str!("em4-view.wgsl").into())});
  let pipeline_layout=device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor{
   label:None,bind_group_layouts:&[&layout],push_constant_ranges:&[]});
  let pipeline=device.create_render_pipeline(&wgpu::RenderPipelineDescriptor{
   label:Some("V3O model pipeline"),layout:Some(&pipeline_layout),
   vertex:wgpu::VertexState{module:&shader,entry_point:"vs_main",buffers:&[Vertex::desc()],compilation_options:Default::default()},
   fragment:Some(wgpu::FragmentState{module:&shader,entry_point:"fs_main",
    targets:&[Some(wgpu::ColorTargetState{format,blend:None,write_mask:wgpu::ColorWrites::ALL})],
    compilation_options:Default::default()}),
   primitive:wgpu::PrimitiveState{cull_mode:None,..Default::default()},
   depth_stencil:Some(wgpu::DepthStencilState{format:wgpu::TextureFormat::Depth24Plus,
    depth_write_enabled:true,depth_compare:wgpu::CompareFunction::Less,stencil:Default::default(),bias:Default::default()}),
   multisample:Default::default(),multiview:None
  });
  let vertex_buffer=device.create_buffer_init(&wgpu::util::BufferInitDescriptor{
   label:Some("V3O vertices"),contents:bytemuck::cast_slice(&vertices),usage:wgpu::BufferUsages::VERTEX});
  let index_buffer=device.create_buffer_init(&wgpu::util::BufferInitDescriptor{
   label:Some("V3O triangles"),contents:bytemuck::cast_slice(&indices),usage:wgpu::BufferUsages::INDEX});
  let depth=depth(&device,config.width,config.height);
  Ok(Self{surface,device,queue,config,pipeline,vertices:vertex_buffer,indices:index_buffer,
   index_count:indices.len() as u32,uniform,bind,depth,camera,initial_camera:camera,drag:None,cursor:None})
 }
 fn resize(&mut self,w:u32,h:u32){
  if w==0||h==0{return;}self.config.width=w;self.config.height=h;
  self.surface.configure(&self.device,&self.config);self.depth=depth(&self.device,w,h);
 }
 fn render(&mut self)->Result<(),wgpu::SurfaceError>{
  let matrix=self.camera.matrix(self.config.width as f32/self.config.height as f32);
  self.queue.write_buffer(&self.uniform,0,bytemuck::bytes_of(&Uniform{matrix}));
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
   pass.draw_indexed(0..self.index_count,0,0..1);
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
fn main()->Result<(),Box<dyn std::error::Error>>{
 env_logger::init();
 let path=std::path::PathBuf::from(env::args_os().nth(1).ok_or("Usage: cargo run -p openem4 --bin em4-view -- <model.V3O>")?);
 let bytes=fs::read(&path)?;
 let mesh=em4_formats::v3o_mesh::parse(&bytes)?;
 println!("Loading {} vertices, {} triangles, {} surfaces",mesh.vertices.len(),mesh.triangles.len(),mesh.surfaces.len());
 let event_loop=EventLoop::new()?;
 let window=Arc::new(WindowBuilder::new().with_title("openEM4 V3O Viewer | LMB Orbit | RMB Pan | Wheel Zoom | R Reset")
  .with_inner_size(winit::dpi::LogicalSize::new(1200.0,800.0)).build(&event_loop)?);
 let mut viewer=pollster::block_on(Viewer::new(window.clone(),&mesh))?;
 event_loop.set_control_flow(ControlFlow::Wait);
 event_loop.run(move |event,target|{
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
     if event.state==ElementState::Pressed && event.physical_key==PhysicalKey::Code(KeyCode::KeyR){
      viewer.camera=viewer.initial_camera;window.request_redraw();
     }
    }
    _=>{}
   }
  }
 })?;
 Ok(())
}
