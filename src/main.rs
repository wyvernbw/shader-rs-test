#![feature(if_let_guard)]
use std::process::exit;
use std::{ffi::CString, path::PathBuf};

use anyhow::anyhow;
use glutin::config::GlConfig;
use glutin::context::{
    AsRawContext, GlContext, NotCurrentGlContext, PossiblyCurrentContext, PossiblyCurrentGlContext,
    Version,
};
use glutin::surface::{GlSurface, Surface, WindowSurface};
use glutin::{
    config::{Api, Config, ConfigTemplateBuilder},
    context::{ContextApi, ContextAttributesBuilder, GlProfile},
    display::{GetGlDisplay, GlDisplay},
};

use clap::Parser;
use glutin_winit::{DisplayBuilder, GlWindow};
use posh::gl::{Sampler2dSettings, UniformBuffer};
use posh::{
    bytemuck::Zeroable,
    gl::{self},
    sl::{self},
    Gl, Sl,
};
use raw_window_handle::HasRawWindowHandle;
use shader::{fragment_shader, vertex_shader, App, Uniforms};
use tracing::Level;
use winit::event::WindowEvent;
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
    let args = Args::parse();
    let State {
        window,
        config,
        gl_surface,
        event_loop,
        window_builder,
        ctx,
    } = match setup().await {
        Ok(setup) => setup,
        Err(err) => {
            tracing::error!("Failed to setup: {:?}", err);
            return Err(err);
        }
    };
    let display = config.display();
    let window_size = window.inner_size();

    let init = || {
        tracing::info!("Loading OpenGL symbols...");
        let mut count = 0;
        let gl = unsafe {
            glow::Context::from_loader_function(|s| {
                let s = CString::new(s).unwrap();
                count += 1;
                display.get_proc_address(&s) as *const _
            })
        };
        tracing::info!("{} Symbols successfully loaded", count);
        let gl = gl::Context::new(gl)?;
        let program: gl::Program<Uniforms<Sl>, sl::Vec2> =
            gl.create_program(vertex_shader, fragment_shader)?;

        let input_image = image::open(&args.input)?;
        let input_image = input_image
            .as_rgba8()
            .map_or_else(|| Err(anyhow!("Not RGBA")), Ok)?;
        let input_image =
            gl::ColorImage::rgba_u8_slice([input_image.width(), input_image.height()], input_image);

        let texture = gl.create_color_texture_2d(input_image)?;
        let vertices: gl::VertexBuffer<gl::Vec2> =
            gl.create_vertex_buffer(&full_screen_quad(), gl::BufferUsage::StaticDraw)?;
        let app: gl::UniformBuffer<App<Gl>> = gl.create_uniform_buffer(
            App {
                window_size: gl::Vec2::from([window_size.width as f32, window_size.height as f32]),
            },
            gl::BufferUsage::StaticRead,
        )?;
        let uniforms = Uniforms {
            texture: texture.as_color_sampler(Sampler2dSettings {
                wrap_s: gl::SamplerWrap::ClampToEdge,
                wrap_t: gl::SamplerWrap::ClampToEdge,
                ..Default::default()
            }),
            app: app.as_binding(),
        };

        anyhow::Result::<_>::Ok(
            move |window_size: PhysicalSize<u32>| -> anyhow::Result<()> {
                let uniforms = uniforms.clone();

                tracing::debug!(?window_size);
                let app: gl::UniformBuffer<App<Gl>> = gl.create_uniform_buffer(
                    App {
                        window_size: gl::Vec2::from([
                            window_size.width as f32,
                            window_size.height as f32,
                        ]),
                    },
                    gl::BufferUsage::StaticRead,
                )?;
                let uniforms = Uniforms {
                    app: app.as_binding(),
                    ..uniforms
                };
                program
                    .with_uniforms(uniforms)
                    .with_settings(
                        gl::DrawSettings::default().with_clear_color([1.0, 1.0, 1.0, 1.0]),
                    )
                    .draw(vertices.as_vertex_spec(gl::PrimitiveMode::Triangles))?;
                Ok(())
            },
        )
    };
    let mut redraw = None;
    event_loop.run(move |event, target| match event {
        Event::NewEvents(StartCause::Init) => {
            //let res = glutin_winit::finalize_window(target, window_builder.clone(), &config);
            //if let Err(err) = res {
            //    tracing::error!(
            //        event = ?Event::<()>::Resumed,
            //        "Failed to finalize window: {:?}",
            //        err
            //    );
            //}
            redraw = Some(init().expect("Failed to initialize"));
        }
        Event::WindowEvent { event, .. } => match event {
            WindowEvent::CloseRequested => exit(0),
            WindowEvent::RedrawRequested => {}
            _ => {}
        },
        Event::Resumed => {}
        _ => {
            if let Some(ref redraw) = redraw {
                if let Err(err) = redraw(window.inner_size()) {
                    tracing::error!("Failed to redraw: {:?}", err);
                };
            }
            window.request_redraw();
            if let Err(err) = gl_surface.swap_buffers(&ctx) {
                tracing::error!("Failed to swap buffers: {:?}", err);
            }
        }
    })?;
    Ok(())
}

pub struct State<T: glutin::surface::SurfaceTypeTrait> {
    window: Window,
    config: Config,
    gl_surface: Surface<T>,
    event_loop: EventLoop<()>,
    window_builder: WindowBuilder,
    ctx: PossiblyCurrentContext,
}
pub async fn setup() -> anyhow::Result<State<WindowSurface>> {
    let _ = tracing_subscriber::fmt()
        .with_max_level(Level::DEBUG)
        .try_init()
        .map_err(|_| {
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

    let ctx = ctx.make_current(&gl_surface)?;
    tracing::info!("Context made current: {:?}", ctx.is_current());
    ctx.make_current(&gl_surface)?;
    let features = display.supported_features();
    tracing::info!("Display features {:?}", features);
    Ok(State {
        window,
        config,
        gl_surface,
        event_loop,
        window_builder,
        ctx,
    })
}

fn full_screen_quad() -> Vec<gl::Vec2> {
    vec![
        [-1.0, 1.0].into(),
        [-1.0, -1.0].into(),
        [1.0, -1.0].into(),
        [1.0, -1.0].into(),
        [1.0, 1.0].into(),
        [-1.0, 1.0].into(),
    ]
}
