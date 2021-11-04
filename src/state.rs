use cgmath::{
    Deg,
    InnerSpace,
    Quaternion,
    Rotation3,
    Vector3,
    Zero,
};
use winit::{
    event::{DeviceEvent, ElementState, KeyboardInput},
    window::Window,
};
use wgpu::util::DeviceExt;

use crate::camera::{CameraController, CameraRig, OrbitCamera, OrbitCameraController};
use crate::instance::{Instance, InstanceRaw};
use crate::model::{Model, ModelVertex, Vertex};
use crate::projection::Projection;
use crate::renderer::Renderer;
use crate::texture::Texture;

const NUM_INSTANCES_PER_ROW: u32 = 1;

pub struct State {
    camera_rig: CameraRig<OrbitCamera, OrbitCameraController>,
    config: wgpu::SurfaceConfiguration,
    device: wgpu::Device,
    instance_buffer: wgpu::Buffer,
    instances: Vec<Instance>,
    mouse_pressed: bool,
    obj_model: Model,
    projection: Projection,
    queue: wgpu::Queue,
    renderer: Renderer,
    pub size: winit::dpi::PhysicalSize<u32>,
    surface: wgpu::Surface,
}

impl State {
    pub async fn new(window: &Window) -> Self {
        let instance = wgpu::Instance::new(wgpu::Backends::all());
        let surface = unsafe { instance.create_surface(window) };
        let adapter = instance.request_adapter(
            &wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::default(),
                compatible_surface: Some(&surface),
            },
        ).await.unwrap();
        let (device, queue) = adapter.request_device(
            &wgpu::DeviceDescriptor {
                features: wgpu::Features::empty(),
                limits: wgpu::Limits::default(),
                label: None,
            },
            None,
        ).await.unwrap();
        let size = window.inner_size();
        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format: surface.get_preferred_format(&adapter).unwrap(),
            width: size.width,
            height: size.height,
            present_mode: wgpu::PresentMode::Fifo,
        };

        surface.configure(&device, &config);

        let camera_rig = CameraRig::new((0.0, 5.0, 10.0));
        let projection = Projection::new(config.width, config.height, Deg(45.0), 0.1, 100.0);

        let camera_bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                }
            ],
            label: Some("camera_binding_group_layout"),
        });
        let light_bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                }
            ],
            label: None,
        });

        let render_pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("render Pipeline Layout"),
            bind_group_layouts: &[
                &camera_bind_group_layout,
                &light_bind_group_layout,
            ],
            push_constant_ranges: &[],
        });
        let mut renderer = Renderer::new(
            &device,
            &render_pipeline_layout,
            &config,
            Some(Texture::DEPTH_FORMAT),
            &[ModelVertex::desc(), InstanceRaw::desc()],
        );

        renderer.update_camera_uniform(&camera_rig.camera, &projection);
        renderer.set_camera_bind_group(&device, &camera_bind_group_layout);
        renderer.set_light_bind_group(&device, &light_bind_group_layout);

        let light_pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Light Pipeline Layout"),
            bind_group_layouts: &[&camera_bind_group_layout, &light_bind_group_layout],
            push_constant_ranges: &[],
        });

        renderer.set_light_render_pipeline(
            &device,
            &light_pipeline_layout,
            config.format,
            Some(Texture::DEPTH_FORMAT),
            &[ModelVertex::desc()],
        );

        let res_dir = std::path::Path::new(env!("OUT_DIR")).join("res");
        let obj_model = Model::load(
            &device,
            res_dir.join("pumpkin.obj"),
        ).unwrap();

        let instances = (0..NUM_INSTANCES_PER_ROW).flat_map(|z| {
            (0..NUM_INSTANCES_PER_ROW).map(move |x| {
                let position = Vector3 { x: x as f32, y: 0.0, z: z as f32 };
                let rotation = if position.is_zero() {
                    Quaternion::from_axis_angle(
                        Vector3::unit_z(),
                        Deg(0.0),
                    )
                } else {
                    Quaternion::from_axis_angle(position.normalize(), Deg(45.0))
                };

                Instance {
                    position,
                    rotation,
                }
            })
        }).collect::<Vec<_>>();
        let instance_data = instances.iter().map(Instance::to_raw).collect::<Vec<_>>();
        let instance_buffer = device.create_buffer_init(
            &wgpu::util::BufferInitDescriptor {
                label: Some("Instance Buffer"),
                contents: bytemuck::cast_slice(&instance_data),
                usage: wgpu::BufferUsages::VERTEX,
            }
        );

        Self {
            camera_rig,
            config,
            device,
            instance_buffer,
            instances,
            mouse_pressed: false,
            obj_model,
            projection,
            queue,
            renderer,
            size,
            surface,
        }
    }

    pub fn resize(&mut self, new_size: winit::dpi::PhysicalSize<u32>) {
        if new_size.width > 0 && new_size.height > 0 {
            self.size = new_size;
            self.config.width = new_size.width;
            self.config.height = new_size.height;
            self.surface.configure(&self.device, &self.config);
            self.renderer.resize(&self.device, &self.config);
            self.projection.resize(new_size.width, new_size.height);
        }
    }

    pub fn input(&mut self, event: &DeviceEvent) -> bool {
        match event {
            DeviceEvent::Key(
                KeyboardInput {
                    virtual_keycode: Some(key),
                    state,
                    ..
                }
            ) => {
                self.camera_rig.controller.process_keyboard(*key, *state);
                true
            }
            DeviceEvent::MouseWheel { delta, .. } => {
                self.camera_rig.controller.process_scroll(delta);
                true
            }
            DeviceEvent::Button {
                button: 1,
                state,
            } => {
                self.mouse_pressed = *state == ElementState::Pressed;
                true
            }
            DeviceEvent::MouseMotion { delta } => {
                if self.mouse_pressed {
                    self.camera_rig.controller.process_mouse(delta.0, delta.1);
                }
                true
            }
            _ => false
        }
    }

    pub fn update(&mut self, dt: std::time::Duration) {
        self.camera_rig.controller.update_camera(&mut self.camera_rig.camera, dt);
        self.renderer.update_camera_uniform(&self.camera_rig.camera, &self.projection);
        self.renderer.update(&self.queue, dt);
    }

    pub fn render(&mut self) -> Result<(), wgpu::SurfaceError> {
        let output = self.surface.get_current_frame()?.output;
        let view = output.texture.create_view(&wgpu::TextureViewDescriptor::default());

        self.renderer.render(
            &self.device,
            &self.queue,
            &view,
            &self.obj_model,
            &self.instance_buffer,
            0..self.instances.len() as u32,
        );

        Ok(())
    }
}
