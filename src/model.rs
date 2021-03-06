use anyhow::Result;
use cgmath::{InnerSpace, Vector3};
use rand::Rng;
use rayon::iter::{IntoParallelIterator, ParallelIterator};
use std::path::Path;
use tobj::LoadOptions;
use wgpu::util::DeviceExt;

use crate::mesh::{Mesh, MeshBuilder, MeshVertex};

const MODEL_COLOR: [f32;4] = [1.0, 0.1, 0.1, 1.0];

pub enum ModelPrimitive {
  Cube,
  Plane,
}

pub struct Model {
  pub meshes: Vec<Mesh>,
}

impl Model {
  pub fn add_post(builder: &mut MeshBuilder, position: Vector3<f32>, width: f32, length: f32, height: f32) {
    let up = Vector3::unit_y() * height;
    let right = Vector3::unit_x() * width;
    let forward = Vector3::unit_z() * length;
    let offset = (right + forward) * 0.5;
    let near_corner = position - offset;
    let far_corner = up + right + forward + position - offset;

    builder.add_quad(near_corner, right, up);
    builder.add_quad(near_corner, up, forward);

    builder.add_quad(far_corner, -right, -forward);
    builder.add_quad(far_corner, -up, -right);
    builder.add_quad(far_corner, -forward, -up);
  }

  pub fn cube(device: &wgpu::Device, size: f32) -> Self {
    let mut builder = MeshBuilder::new("Cube");
    let up = size * Vector3::unit_y();
    let right = size * Vector3::unit_x();
    let forward = size * Vector3::unit_z();
    let near_corner = Vector3::new(-size / 2.0, -size / 2.0, -size / 2.0);
    let far_corner = Vector3::new(size / 2.0, size / 2.0, size / 2.0);

    builder.add_quad(near_corner, forward, right);
    builder.add_quad(near_corner, right, up);
    builder.add_quad(near_corner, up, forward);

    builder.add_quad(far_corner, -right, -forward);
    builder.add_quad(far_corner, -up, -right);
    builder.add_quad(far_corner, -forward, -up);

    let mesh = builder.build(device);

    Self { meshes: vec![mesh] }
  }

  pub fn house(device: &wgpu::Device, width: f32, length: f32, height: f32) -> Self {
    let mut builder = MeshBuilder::new("House");

    let up = height * Vector3::unit_y();
    let right = width * Vector3::unit_x();
    let mut forward = length * Vector3::unit_z();
    let near_corner = Vector3::new(-width / 2.0, 0.0, -length / 2.0);
    let far_corner = Vector3::new(width / 2.0, height, length / 2.0);

    builder.add_quad(near_corner, right, up);
    builder.add_quad(near_corner, up, forward);

    builder.add_quad(far_corner, -up, -right);
    builder.add_quad(far_corner, -forward, -up);

    let wall_top_left = near_corner + up;
    let wall_top_right = wall_top_left + right;
    let mut roof_peak = wall_top_left + 0.5 * up + (width / 2.0) * right;

    builder.add_triangle(wall_top_left, roof_peak, wall_top_right);
    builder.add_triangle(wall_top_left + forward, wall_top_right + forward, roof_peak + forward);

    let mut from_peak_left = wall_top_left - roof_peak;
    let mut from_peak_right = wall_top_right - roof_peak;
    let roof_overhang = 0.2;

    from_peak_left += from_peak_left.normalize() * roof_overhang;
    from_peak_right += from_peak_right.normalize() * roof_overhang;

    roof_peak -= Vector3::unit_z() * roof_overhang;

    // Bias roof upwards for z-fighting
    roof_peak += Vector3::unit_y() * 0.001;
    forward += Vector3::unit_z() * roof_overhang * 2.0;

    builder.add_quad(roof_peak, forward, from_peak_left);
    builder.add_quad(roof_peak, from_peak_left, forward);
    builder.add_quad(roof_peak, from_peak_right, forward);
    builder.add_quad(roof_peak, forward, from_peak_right);

    let mesh = builder.build(device);

    Self { meshes: vec![mesh] }
  }

  pub fn load<P: AsRef<Path>>(
    device: &wgpu::Device,
    path: P,
  ) -> Result<Self> {
    let (obj_models, _) = tobj::load_obj(path.as_ref(), &LoadOptions {
      triangulate: true,
      single_index: true,
      ..Default::default()
    })?;
    let meshes = obj_models.iter().map(|m| {
      let vertices = (0..m.mesh.positions.len() / 3).into_par_iter().map(|i| {
        MeshVertex {
          position: [
            m.mesh.positions[i * 3],
            m.mesh.positions[i * 3 + 1],
            m.mesh.positions[i * 3 + 2],
          ].into(),
          normal: [
            m.mesh.normals[i * 3],
            m.mesh.normals[i * 3 + 1],
            m.mesh.normals[i * 3 + 2],
          ].into(),
          color: MODEL_COLOR,
        }
      }).collect::<Vec<_>>();

      let vertex_buffer = device.create_buffer_init(
        &wgpu::util::BufferInitDescriptor {
          label: Some(&format!("{:?} Vertex Buffer", path.as_ref())),
          contents: bytemuck::cast_slice(&vertices),
          usage: wgpu::BufferUsages::VERTEX,
        }
      );
      let index_buffer = device.create_buffer_init(
        &wgpu::util::BufferInitDescriptor {
          label: Some(&format!("{:?} Index Buffer", path.as_ref())),
          contents: bytemuck::cast_slice(&m.mesh.indices),
          usage: wgpu::BufferUsages::INDEX,
        }
      );

      Ok(Mesh {
        name: String::from(&m.name),
        vertex_buffer,
        index_buffer,
        num_elements: m.mesh.indices.len() as u32,
        material: m.mesh.material_id.unwrap_or(0),
      })
    }).collect::<Result<Vec<_>>>()?;

    Ok(Self { meshes })
  }

  pub fn plane(device: &wgpu::Device, size: f32) -> Self {
    let mut builder = MeshBuilder::new("Plane");

    builder.add_quad(
      Vector3::new(-size / 2.0, 0.0, -size / 2.0),
      Vector3::new(size, 0.0, 0.0),
      Vector3::new(0.0, 0.0, size),
    );

    let mesh = builder.build(device);

    Self { meshes: vec![mesh] }
  }

  pub fn surface(device: &wgpu::Device, count: u32, size: f32, height_max: f32) -> Self {
    let mut builder = MeshBuilder::new("Quad Grid");
    let half_count = count as i32 / 2;
    let mut rng = rand::thread_rng();

    for i in -half_count..half_count + 1 {
      let z = 2.0 * size * i as f32;

      for j in -half_count..half_count + 1 {
        let x = 2.0 * size * j as f32;
        let y = rng.gen_range(0.0..height_max);
        let position = Vector3::new(x, y, z);
        let link = i > -half_count && j > -half_count;

        builder.add_linked_quad(position, link, count + 1);
      }
    }

    let mesh = builder.build(device);

    Self { meshes: vec![mesh] }
  }
}
