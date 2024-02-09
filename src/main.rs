use std::{ffi::CString, path::PathBuf};

use anyhow::anyhow;
use glutin::config::GlConfig;
use glutin::context::{AsRawContext, GlContext, NotCurrentGlContext, Version};
use glutin::{
    config::{Api, Config, ConfigTemplateBuilder},
    context::{ContextApi, ContextAttributesBuilder, GlProfile},
    display::{GetGlDisplay, GlDisplay},
};

use clap::Parser;
use glutin_winit::{DisplayBuilder, GlWindow};
use posh::{
    bytemuck::Zeroable,
    gl::{self},
    sl::{self},
    Gl, Sl,
};
use raw_window_handle::HasRawWindowHandle;
use shader::{fragment_shader, vertex_shader, Uniforms};
use winit::{
    dpi::PhysicalSize,
    event::{Event, StartCause},
    event_loop::EventLoop,
    window::{Window, WindowBuilder},
};

pub fn main() -> anyhow::Result<()> {
    smol::block_on(run())
}

#[derive(Parser, Debug)]
struct Args {
    #[arg(short, long)]
    input: PathBuf,
    #[arg(short, long, default_value = "./output.png")]
    output: PathBuf,
}

pub async fn run() -> anyhow::Result<()> {
    let (window, config, event_loop, window_builder) = match setup().await {
        Ok(setup) => setup,
        Err(err) => {
            tracing::error!("Failed to setup: {:?}", err);
            return Err(err);
        }
    };
    let display = config.display();

    let init = || {
        let gl = unsafe {
            glow::Context::from_loader_function(|s| {
                let s = CString::new(s).unwrap();
                display.get_proc_address(&s) as *const _
            })
        };
        let gl = gl::Context::new(gl)?;
        let program: gl::Program<Uniforms<Sl>, sl::Vec2> =
            gl.create_program(vertex_shader, fragment_shader)?;
        let uniforms: gl::UniformBuffer<Uniforms<Gl>> = gl.create_uniform_buffer(
            Uniforms {
                fragment: gl::Vec4::zeroed(),
            },
            gl::BufferUsage::DynamicDraw,
        )?;
        let vertices: gl::VertexBuffer<gl::Vec2> = gl.create_vertex_buffer(
            &[
                [0.0f32, 1.0].into(),
                [-0.5, -0.5].into(),
                [0.5, -0.5].into(),
            ],
            gl::BufferUsage::StreamDraw,
        )?;
        program
            .with_uniforms(uniforms.as_binding())
            .with_settings(gl::DrawSettings::default().with_clear_color([0.1, 0.2, 0.3, 1.0]))
            .draw(vertices.as_vertex_spec(gl::PrimitiveMode::Triangles))?;

        anyhow::Result::<()>::Ok(())
    };
    event_loop.run(move |event, target| match event {
        Event::NewEvents(StartCause::Init) => {
            init().expect("Failed to initialize");
        }
        Event::Resumed => {
            let res = glutin_winit::finalize_window(target, window_builder.clone(), &config);
            if let Err(err) = res {
                tracing::error!(
                    event = ?Event::<()>::Resumed,
                    "Failed to finalize window: {:?}",
                    err
                );
            }
        }
        _ => {}
    })?;
    Ok(())
}

pub type Setup = (Window, Config, EventLoop<()>, WindowBuilder);
pub async fn setup() -> anyhow::Result<Setup> {
    let _ = tracing_subscriber::fmt::try_init().map_err(|_| {
        eprintln!("Failed to initialize logger");
        tracing::error!("Logger already initialized, ignoring this error.");
    });
    let event_loop = EventLoop::new()?;
    let window_builder = WindowBuilder::new()
        .with_title("Posh")
        .with_transparent(true)
        .with_inner_size(PhysicalSize::new(800, 600));
    let template = ConfigTemplateBuilder::new().with_api(Api::OPENGL);

    let display = DisplayBuilder::new().with_window_builder(Some(window_builder.clone()));

    let (Some(window), config) = display
        .build(&event_loop, template, |configs| {
            let configs = configs.collect::<Vec<_>>();
            tracing::info!("Configs: {:#?}", configs);
            configs
                .into_iter()
                .inspect(|config| {
                    tracing::info!(?config, api = ?config.api(), "Config: ");
                })
                //.filter(|config| config.api() == Api::GLES3)
                .reduce(|accum, config| {
                    let transparency_check = config.supports_transparency().unwrap_or(false)
                        & !accum.supports_transparency().unwrap_or(false);

                    if transparency_check || config.num_samples() > accum.num_samples() {
                        config
                    } else {
                        accum
                    }
                })
                .expect("No suitable config found")
        })
        .map_err(|_| anyhow!("Failed to initialize window"))?
    else {
        return Err(anyhow!("Failed to create window"));
    };
    let raw_window_handle = window.raw_window_handle();

    let context_attributes = ContextAttributesBuilder::new()
        .with_context_api(ContextApi::OpenGl(Some(Version::new(4, 1))))
        .build(Some(raw_window_handle));

    let display = config.display();
    let version = display.version_string();
    tracing::info!("OpenGL version: {:?}", version);

    let ctx = unsafe { display.create_context(&config, &context_attributes)? };
    tracing::info!("OpenGL context created: {:?}", ctx.context_api());
    let surface_attributes = window.build_surface_attributes(Default::default());

    let gl_surface = unsafe {
        config
            .display()
            .create_window_surface(&config, &surface_attributes)?
    };

    ctx.make_current(&gl_surface)?;
    let features = display.supported_features();
    tracing::info!("Display features {:?}", features);
    Ok((window, config, event_loop, window_builder))
}
