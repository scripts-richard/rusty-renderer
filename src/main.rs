use clap::Parser;
use winit::{
    event::{
        ElementState,
        Event,
        KeyboardInput,
        VirtualKeyCode,
        WindowEvent,
    },
    event_loop::{ControlFlow, EventLoop},
    window::WindowBuilder,
};

mod camera;
mod color;
mod draw;
mod instance;
mod light;
mod mesh;
mod model;
mod projection;
mod render;
mod state;
mod texture;
mod uniform;

use model::ModelPrimitive;
use state::State;

#[derive(Parser, Debug)]
#[clap(about, author, version)]
struct Cli {
    #[clap(long, default_value_t = 8)]
    count: u32,
    #[clap(short, long)]
    cube: bool,
    #[clap(short, long)]
    file: bool,
    #[clap(long, default_value_t = 1.0)]
    height: f32,
    #[clap(short, long)]
    house: bool,
    #[clap(long, default_value_t = 1.0)]
    length: f32,
    #[clap(long, default_value_t = 0.5)]
    max: f32,
    #[clap(short, long)]
    plane: bool,
    #[clap(long, default_value_t = 1.0)]
    size: f32,
    #[clap(short, long)]
    surface: bool,
    #[clap(long, default_value_t = 1.0)]
    width: f32,
}

fn main() {
    env_logger::init();
    let cli = Cli::parse();
    let event_loop = EventLoop::new();
    let window = WindowBuilder::new().build(&event_loop).unwrap();
    let mut state = pollster::block_on(State::new(&window));

    state.render().unwrap();

    if cli.cube {
        state.add_model_primitive(ModelPrimitive::Cube, cli.size);
    }
    if cli.file {
        state.prompt_for_file().unwrap();
    }
    if cli.house {
        state.add_house(cli.width, cli.length, cli.height);
    }
    if cli.plane {
        state.add_model_primitive(ModelPrimitive::Plane, cli.size);
    }
    if cli.surface {
        state.add_surface(cli.count, cli.size, cli.max);
    }

    let mut last_render_time = std::time::Instant::now();

    event_loop.run(move |event, _, control_flow| {
        *control_flow = ControlFlow::Poll;

        match event {
            Event::DeviceEvent {
                ref event,
                ..
            } => {
                state.input(event);
            }
            Event::WindowEvent {
                ref event,
                window_id,
            } if window_id == window.id() => {
                match event {
                    WindowEvent::CloseRequested
                    | WindowEvent::KeyboardInput {
                        input:
                            KeyboardInput {
                                state: ElementState::Pressed,
                                virtual_keycode: Some(VirtualKeyCode::Escape),
                                ..
                            },
                        ..
                    } => *control_flow = ControlFlow::Exit,
                    WindowEvent::Resized(physical_size) => {
                        state.resize(*physical_size);
                    }
                    WindowEvent::ScaleFactorChanged { new_inner_size, ..} => {
                        state.resize(**new_inner_size);
                    }
                    _ => {}
                }
            }
            Event::RedrawRequested(_) => {
                let now = std::time::Instant::now();
                let dt = now - last_render_time;

                last_render_time = now;
                state.update(dt);
                match state.render() {
                    Ok(_) => {}
                    Err(wgpu::SurfaceError::Lost) => state.resize(state.size),
                    Err(wgpu::SurfaceError::OutOfMemory) => *control_flow = ControlFlow::Exit,
                    Err(e) => eprintln!("{:?}", e),
                }
            }
            Event::MainEventsCleared => {
                window.request_redraw();
            }
            _ => {}
        }
    });
}


