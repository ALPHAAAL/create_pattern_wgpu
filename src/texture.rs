use std::num::NonZeroU32;

use anyhow::*;
use image::GenericImageView;
use wgpu::util::DeviceExt;

pub struct Texture {
    pub texture: wgpu::Texture,
    pub view: wgpu::TextureView,
    pub sampler: wgpu::Sampler,
    pub vertex_buffer: wgpu::Buffer,
    pub index_buffer: wgpu::Buffer,
    pub uniform_buffer: wgpu::Buffer,
}

#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct Vertex {
    pub position: [f32; 3],
    pub tex_coords: [f32; 2],
}

#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct TextureTransform {
    pub transform: [[f32; 4]; 4],
    pub bitmap_transform: [[f32; 4]; 4],
    pub inverse_bitmap_transform: [[f32; 4]; 4],
    pub image_dimension: [f32; 4],
}

impl Vertex {
    pub fn desc<'a>() -> wgpu::VertexBufferLayout<'a> {
        use std::mem;
        wgpu::VertexBufferLayout {
            array_stride: mem::size_of::<Vertex>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &[
                wgpu::VertexAttribute {
                    offset: 0,
                    shader_location: 0,
                    format: wgpu::VertexFormat::Float32x3,
                },
                wgpu::VertexAttribute {
                    offset: mem::size_of::<[f32; 3]>() as wgpu::BufferAddress,
                    shader_location: 1,
                    format: wgpu::VertexFormat::Float32x2,
                },
            ],
        }
    }
}

fn inverse(a: [[f32; 4]; 4]) -> Option<[[f32; 4]; 4]> {
    let mut out = [[0.0; 4]; 4];
    let a00 = a[0][0];
    let a01 = a[0][1];
    let a02 = a[0][2];
    let a03 = a[0][3];
    let a10 = a[1][0];
    let a11 = a[1][1];
    let a12 = a[1][2];
    let a13 = a[1][3];
    let a20 = a[2][0];
    let a21 = a[2][1];
    let a22 = a[2][2];
    let a23 = a[2][3];
    let a30 = a[3][0];
    let a31 = a[3][1];
    let a32 = a[3][2];
    let a33 = a[3][3];
    let b00 = a00 * a11 - a01 * a10;
    let b01 = a00 * a12 - a02 * a10;
    let b02 = a00 * a13 - a03 * a10;
    let b03 = a01 * a12 - a02 * a11;
    let b04 = a01 * a13 - a03 * a11;
    let b05 = a02 * a13 - a03 * a12;
    let b06 = a20 * a31 - a21 * a30;
    let b07 = a20 * a32 - a22 * a30;
    let b08 = a20 * a33 - a23 * a30;
    let b09 = a21 * a32 - a22 * a31;
    let b10 = a21 * a33 - a23 * a31;
    let b11 = a22 * a33 - a23 * a32;
    // Calculate the determinant
    let det = b00 * b11 - b01 * b10 + b02 * b09 + b03 * b08 - b04 * b07 + b05 * b06;

    if det == 0.0 {
        return None;
    }

    let det = 1.0 / det;

    out[0][0] = (a11 * b11 - a12 * b10 + a13 * b09) * det;
    out[0][1] = (a02 * b10 - a01 * b11 - a03 * b09) * det;
    out[0][2] = (a31 * b05 - a32 * b04 + a33 * b03) * det;
    out[0][3] = (a22 * b04 - a21 * b05 - a23 * b03) * det;
    out[1][0] = (a12 * b08 - a10 * b11 - a13 * b07) * det;
    out[1][1] = (a00 * b11 - a02 * b08 + a03 * b07) * det;
    out[1][2] = (a32 * b02 - a30 * b05 - a33 * b01) * det;
    out[1][3] = (a20 * b05 - a22 * b02 + a23 * b01) * det;
    out[2][0] = (a10 * b10 - a11 * b08 + a13 * b06) * det;
    out[2][1] = (a01 * b08 - a00 * b10 - a03 * b06) * det;
    out[2][2] = (a30 * b04 - a31 * b02 + a33 * b00) * det;
    out[2][3] = (a21 * b02 - a20 * b04 - a23 * b00) * det;
    out[3][0] = (a11 * b07 - a10 * b09 - a12 * b06) * det;
    out[3][1] = (a00 * b09 - a01 * b07 + a02 * b06) * det;
    out[3][2] = (a31 * b01 - a30 * b03 - a32 * b00) * det;
    out[3][3] = (a20 * b03 - a21 * b01 + a22 * b00) * det;

    Some(out)
}

impl Texture {
    pub fn from_bytes(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        bytes: &[u8],
        label: &str,
    ) -> Result<Self> {
        let img = image::load_from_memory(bytes)?;
        Self::from_image(device, queue, &img, Some(label))
    }

    pub fn from_image(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        img: &image::DynamicImage,
        label: Option<&str>,
    ) -> Result<Self> {
        let rgba = img.to_rgba8();
        let dimensions = img.dimensions();

        let size = wgpu::Extent3d {
            width: dimensions.0,
            height: dimensions.1,
            depth_or_array_layers: 1,
        };
        let format = wgpu::TextureFormat::Rgba8UnormSrgb;
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label,
            size,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });

        queue.write_texture(
            wgpu::ImageCopyTexture {
                aspect: wgpu::TextureAspect::All,
                texture: &texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
            },
            &rgba,
            wgpu::ImageDataLayout {
                offset: 0,
                bytes_per_row: NonZeroU32::new(4 * dimensions.0),
                rows_per_image: NonZeroU32::new(dimensions.1),
            },
            size,
        );

        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            address_mode_u: wgpu::AddressMode::Repeat,
            address_mode_v: wgpu::AddressMode::Repeat,
            address_mode_w: wgpu::AddressMode::Repeat,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });

        let x = 640.0;
        let y = 360.0;

        let image_width = dimensions.0 as f32;
        let image_height = dimensions.1 as f32;

        let vertices = [
            Vertex {
                position: [0.0, 0.0, 0.0],
                tex_coords: [0.0, 0.0],
            }, // A
            Vertex {
                position: [x, 0.0, 0.0],
                // tex_coords: [image_width, 0.0],
                tex_coords: [1.0, 0.0],
            }, // B
            Vertex {
                position: [x, y, 0.0],
                // tex_coords: [image_width, image_height],
                tex_coords: [1.0, 1.0],
            }, // C
            Vertex {
                position: [0.0, y, 0.0],
                // tex_coords: [0.0, image_height],
                tex_coords: [0.0, 1.0],
            }, // D
        ];
        let indices: [u16; 6] = [1, 0, 2, 0, 2, 3];

        let vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Vertex Buffer"),
            contents: bytemuck::cast_slice(&vertices),
            usage: wgpu::BufferUsages::VERTEX,
        });
        let index_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Index Buffer"),
            contents: bytemuck::cast_slice(&indices),
            usage: wgpu::BufferUsages::INDEX,
        });

        let bitmap_rotate = 0.9998476951563913;
        let bitmap_scale = 0.017452406437283376;
        // let bitmap_rotate = 0.7071067811865476;
        // let bitmap_scale = 0.7071067811865476;
        let m = [
            [bitmap_scale, bitmap_rotate, 0.0, 0.0],
            [-bitmap_rotate, bitmap_scale, 0.0, 0.0],
            [0.0, 0.0, 1.0, 0.0],
            [0.0, 0.0, 0.0, 1.0],
        ];

        let uniform_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Texture transform uniform buffer"),
            contents: bytemuck::cast_slice(&[TextureTransform {
                transform: [
                    [1.0, 0.0, 0.0, 0.0],
                    [-0.0, 1.0, 0.0, 0.0],
                    [0.0, 0.0, 1.0, 0.0],
                    [0.0, 0.0, 0.0, 1.0],
                ],
                bitmap_transform: m,
                inverse_bitmap_transform: inverse(m).unwrap(),
                image_dimension: [image_width, image_height, 0.0, 1.0],
            }]),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        Ok(Self {
            texture,
            view,
            sampler,
            vertex_buffer,
            index_buffer,
            uniform_buffer,
        })
    }
}
