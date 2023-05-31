// Vertex shader
struct VertexInput {
    @location(0) position: vec3<f32>,
    @location(1) uv: vec2<f32>,
}

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) tex_coords: vec2<f32>,
}

struct TextureTransform {
    texture_matrix: mat4x4<f32>,
    bitmap_transform: mat4x4<f32>,
    inverse_bitmap_transform: mat4x4<f32>,
    image_dimension: vec4<f32>,
};

@group(0) @binding(2) var<uniform> u_texture: TextureTransform;

@vertex
fn vs_main(
    model: VertexInput,
) -> VertexOutput {
    var out: VertexOutput;

    let container_dimension = vec2<f32>(
        640.0 * u_texture.texture_matrix[0][0],
        360.0 * u_texture.texture_matrix[1][1],
    );

    let bitmap_transform = u_texture.texture_matrix * u_texture.inverse_bitmap_transform;

    var temp = u_texture.texture_matrix * vec4<f32>(model.position.xy, 0.0, 1.0);
    let pos = vec3<f32>(
        temp.x / 640.0 * 2.0 - 1.0,
        temp.y / 360.0 * -2.0 + 1.0,
        1.0,
    );

    let new_image_dimension = u_texture.bitmap_transform * u_texture.image_dimension;
    let repeat_x = abs(container_dimension.x / new_image_dimension.x);
    let repeat_y = abs(container_dimension.y / new_image_dimension.y);

    out.tex_coords = (model.position.xy / new_image_dimension.xy).yx;
    out.clip_position = vec4<f32>(pos, 1.0);

    return out;
}

// Fragment shader
@group(0) @binding(0)
var t_diffuse: texture_2d<f32>;
@group(0)@binding(1)
var s_diffuse: sampler;

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    return textureSample(t_diffuse, s_diffuse, in.tex_coords);
}