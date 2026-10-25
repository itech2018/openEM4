use std::{env, path::PathBuf, sync::Arc};
use winit::{event::{Event, WindowEvent}, event_loop::{ControlFlow, EventLoop}, window::WindowBuilder};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::init();
    let args: Vec<String> = env::args().skip(1).collect();
    if args.iter().any(|a| a == "--help" || a == "-h") {
        println!("openEM4 v0.1.0-dev\n  --help                 Show this message\n  --scan <directory>     Output installation and mod inventory as JSON\n  --headless             Do not create a GPU window (requires --scan)");
        return Ok(());
    }
    if let Some(index) = args.iter().position(|a| a == "--scan") {
        let path = args.get(index + 1).ok_or("--scan requires a directory argument")?;
        let report = em4_vfs::scan_installation(&PathBuf::from(path));
        println!("{}", serde_json::to_string_pretty(&report)?);
        if args.iter().any(|a| a == "--headless") { return Ok(()); }
    } else if args.iter().any(|a| a == "--headless") {
        return Err("--headless requires --scan <directory>".into());
    }
    let event_loop = EventLoop::new()?;
    let window = Arc::new(WindowBuilder::new().with_title("openEM4 - v0.1.0-dev | Renderer Diagnostic")
        .with_inner_size(winit::dpi::LogicalSize::new(1200.0, 720.0)).build(&event_loop)?);
    let mut renderer = pollster::block_on(em4_render::Renderer::new(window.clone()))
        .map_err(|e| format!("Renderer initialization failed: {e}"))?;
    event_loop.set_control_flow(ControlFlow::Poll);
    event_loop.run(move |event, target| {
        match event {
            Event::WindowEvent { window_id, event } if window_id == window.id() => match event {
                WindowEvent::CloseRequested => target.exit(),
                WindowEvent::Resized(size) => renderer.resize(size.width, size.height),
                WindowEvent::RedrawRequested => match renderer.render() {
                    Ok(()) => {},
                    Err(wgpu::SurfaceError::Lost | wgpu::SurfaceError::Outdated) => {
                        let s = window.inner_size(); renderer.resize(s.width, s.height);
                    }
                    Err(wgpu::SurfaceError::OutOfMemory) => target.exit(),
                    Err(wgpu::SurfaceError::Timeout) => log::warn!("GPU surface timeout"),
                },
                _ => {},
            },
            Event::AboutToWait => window.request_redraw(),
            _ => {},
        }
    })?;
    Ok(())
}
