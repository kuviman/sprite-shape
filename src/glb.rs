use super::*;

use gltf::json as gltf_json;
use gltf::json;
use json::validation::Checked::Valid;
use json::validation::USize64;
use std::borrow::Cow;

#[derive(Copy, Clone, Debug)]
#[repr(C)]
struct Vertex {
    position: [f32; 3],
    uv: [f32; 2],
}

/// Calculate bounding coordinates of a list of vertices, used for the clipping distance of the model
fn bounding_coords(points: &[Vertex]) -> ([f32; 3], [f32; 3]) {
    let mut min = [f32::MAX, f32::MAX, f32::MAX];
    let mut max = [f32::MIN, f32::MIN, f32::MIN];

    for point in points {
        let p = point.position;
        for i in 0..3 {
            min[i] = f32::min(min[i], p[i]);
            max[i] = f32::max(max[i], p[i]);
        }
    }
    (min, max)
}

fn align_to_multiple_of_four(n: &mut usize) {
    *n = (*n + 3) & !3;
}

fn to_padded_byte_vector<T>(vec: Vec<T>) -> Vec<u8> {
    let byte_length = vec.len() * mem::size_of::<T>();
    let byte_capacity = vec.capacity() * mem::size_of::<T>();
    let alloc = vec.into_boxed_slice();
    let ptr = Box::<[T]>::into_raw(alloc) as *mut u8;
    let mut new_vec = unsafe { Vec::from_raw_parts(ptr, byte_length, byte_capacity) };
    while new_vec.len() % 4 != 0 {
        new_vec.push(0); // pad to multiple of four bytes
    }
    new_vec
}

pub fn save(ugli: &Ugli, sprite: &sprite_shape::ThickSprite<viewer::Vertex>) -> Vec<u8> {
    let vertices: Vec<Vertex> = sprite
        .mesh
        .iter()
        .map(|vertex| Vertex {
            position: **vertex.a_pos,
            uv: **vertex.a_uv,
        })
        .collect();
    let vertex_count = vertices.len();

    let image = {
        let texture = &sprite.texture;
        let framebuffer =
            ugli::FramebufferRead::new_color(ugli, ugli::ColorAttachmentRead::Texture(texture));
        let data = framebuffer.read_color();
        let image = geng::image::RgbaImage::from_vec(
            texture.size().x as _,
            texture.size().y as _,
            data.data().to_vec(),
        )
        .unwrap();
        image
    };

    let (min, max) = bounding_coords(&vertices);
    let mut root = gltf_json::Root::default();

    let vertex_data_start;
    let vertex_data_end;
    let texture_data_start;
    let texture_data_end;
    let all_data = {
        let mut writer = std::io::Cursor::new(Vec::new());
        {
            vertex_data_start = writer.position();
            writer.write_all(&to_padded_byte_vector(vertices)).unwrap();
            vertex_data_end = writer.position();
        }
        {
            texture_data_start = writer.position();
            image
                .write_to(&mut writer, geng::image::ImageFormat::Png)
                .unwrap();
            texture_data_end = writer.position();
        }
        writer.into_inner()
    };
    let buffer = root.push(json::Buffer {
        byte_length: USize64::from(all_data.len()),
        extensions: Default::default(),
        extras: Default::default(),
        name: None,
        uri: None,
    });
    let vertex_data_view = root.push(json::buffer::View {
        buffer,
        byte_length: USize64::from(vertex_data_end - vertex_data_start),
        byte_offset: Some(USize64::from(vertex_data_start)),
        byte_stride: Some(json::buffer::Stride(mem::size_of::<Vertex>())),
        extensions: Default::default(),
        extras: Default::default(),
        name: None,
        target: Some(Valid(json::buffer::Target::ArrayBuffer)),
    });

    let image_buffer_view = root.push(json::buffer::View {
        buffer,
        byte_length: USize64::from(texture_data_end - texture_data_start),
        byte_offset: Some(texture_data_start.into()),
        byte_stride: None,
        name: None,
        target: None,
        extensions: None,
        extras: default(),
    });

    let positions = root.push(json::Accessor {
        buffer_view: Some(vertex_data_view),
        byte_offset: Some(USize64::from(std::mem::offset_of!(Vertex, position))),
        count: USize64::from(vertex_count),
        component_type: Valid(json::accessor::GenericComponentType(
            json::accessor::ComponentType::F32,
        )),
        extensions: Default::default(),
        extras: Default::default(),
        type_: Valid(json::accessor::Type::Vec3),
        min: Some(json::Value::from(Vec::from(min))),
        max: Some(json::Value::from(Vec::from(max))),
        name: None,
        normalized: false,
        sparse: None,
    });

    let image = root.push(json::Image {
        buffer_view: Some(image_buffer_view),
        mime_type: Some(json::image::MimeType(
            json::image::VALID_MIME_TYPES
                .iter()
                .find(|mime| mime.contains("png"))
                .unwrap()
                .to_string(),
        )),
        name: None,
        uri: None,
        extensions: None,
        extras: default(),
    });

    let texture = root.push(json::Texture {
        name: None,
        sampler: None,
        source: image,
        extensions: None,
        extras: default(),
    });

    let material = root.push(json::Material {
        alpha_cutoff: None,
        alpha_mode: default(),
        double_sided: false,
        name: None,
        pbr_metallic_roughness: json::material::PbrMetallicRoughness {
            base_color_factor: default(),
            base_color_texture: Some(json::texture::Info {
                index: texture,
                tex_coord: 0,
                extensions: None,
                extras: default(),
            }),
            metallic_factor: default(),
            roughness_factor: default(),
            metallic_roughness_texture: None,
            extensions: None,
            extras: default(),
        },
        normal_texture: None,
        occlusion_texture: None,
        emissive_texture: None,
        emissive_factor: default(),
        extensions: None,
        extras: default(),
    });

    let uvs = root.push(json::Accessor {
        buffer_view: Some(vertex_data_view),
        byte_offset: Some(USize64::from(std::mem::offset_of!(Vertex, uv))),
        count: USize64::from(vertex_count),
        component_type: Valid(json::accessor::GenericComponentType(
            json::accessor::ComponentType::F32,
        )),
        extensions: Default::default(),
        extras: Default::default(),
        type_: Valid(json::accessor::Type::Vec2),
        min: None,
        max: None,
        name: None,
        normalized: false,
        sparse: None,
    });

    let primitive = json::mesh::Primitive {
        attributes: {
            let mut map = std::collections::BTreeMap::new();
            map.insert(Valid(json::mesh::Semantic::Positions), positions);
            map.insert(Valid(json::mesh::Semantic::TexCoords(0)), uvs);
            map
        },
        extensions: Default::default(),
        extras: Default::default(),
        indices: None,
        material: Some(material),
        mode: Valid(json::mesh::Mode::Triangles),
        targets: None,
    };

    let mesh = root.push(json::Mesh {
        extensions: Default::default(),
        extras: Default::default(),
        name: None,
        primitives: vec![primitive],
        weights: None,
    });

    let node = root.push(json::Node {
        mesh: Some(mesh),
        ..Default::default()
    });

    root.push(json::Scene {
        extensions: Default::default(),
        extras: Default::default(),
        name: None,
        nodes: vec![node],
    });

    let json_string = json::serialize::to_string(&root).expect("Serialization error");
    let mut json_offset = json_string.len();
    align_to_multiple_of_four(&mut json_offset);
    let all_data = to_padded_byte_vector(all_data);
    let glb = gltf::binary::Glb {
        header: gltf::binary::Header {
            magic: *b"glTF",
            version: 2,
            // N.B., the size of binary glTF file is limited to range of `u32`.
            length: (json_offset + all_data.len())
                .try_into()
                .expect("file size exceeds binary glTF limit"),
        },
        bin: Some(Cow::Owned(all_data)),
        json: Cow::Owned(json_string.into_bytes()),
    };
    glb.to_vec().expect("glTF binary output error")
}
