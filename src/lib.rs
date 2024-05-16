use std::collections::{BTreeMap, VecDeque};

use geng::prelude::{itertools::Itertools, *};

pub struct ThickSprite<V: ugli::Vertex> {
    pub texture: ugli::Texture,
    pub mesh: ugli::VertexBuffer<V>,
}

impl<V: ugli::Vertex + From<Vertex>> ThickSprite<V> {
    pub fn new(ugli: &Ugli, image: &geng::image::RgbaImage, options: &Options) -> Self {
        let vertices = generate_mesh(image, options);
        let texture = ugli::Texture::from_image_image(ugli, image.clone());
        let fixed_texture = fix_texture(ugli, &texture);
        let mesh = ugli::VertexBuffer::new_static(ugli, vertices);
        Self {
            texture: fixed_texture,
            mesh,
        }
    }
}

#[derive(ugli::Vertex)]
pub struct Vertex {
    pub a_pos: vec3<f32>,
    pub a_uv: vec2<f32>,
    pub a_normal: vec3<f32>,
}

#[derive(Copy, Clone)]
struct MarchVertex {
    pos: vec2<f32>,
    value: f32,
}

type MarchFace = [MarchVertex; 3];

/// mesh for value >= iso
fn marching_triangles(bb: Aabb2<i32>, f: impl Fn(vec2<i32>) -> f32, iso: f32) -> Vec<MarchFace> {
    let mut result = Vec::new();
    let mut march = |vs: &[vec2<i32>]| {
        let mut current = Vec::new();
        for (&ia, &ib) in vs.iter().circular_tuple_windows() {
            let va = f(ia);
            let vb = f(ib);
            let a = ia.map(|x| x as f32);
            let b = ib.map(|x| x as f32);
            if va >= iso {
                current.push(MarchVertex { pos: a, value: va });
            }
            {
                let (a, b, va, vb) = if **ia < **ib {
                    (a, b, va, vb)
                } else {
                    (b, a, vb, va)
                };
                let t = (iso - va) / (vb - va);
                if t > 0.0 && t < 1.0 {
                    current.push(MarchVertex {
                        pos: a + (b - a) * t,
                        value: iso,
                    });
                }
            }
            if vb >= iso {
                current.push(MarchVertex { pos: b, value: vb });
            }
        }
        if current.len() >= 3 {
            let o = current[0];
            for (&a, &b) in current[1..].iter().tuple_windows() {
                result.push([o, a, b]);
            }
        }
    };
    for x in bb.min.x..bb.max.x {
        for y in bb.min.y..bb.max.y {
            // march([vec2(x, y), vec2(x + 1, y), vec2(x + 1, y + 1)]);
            // march([vec2(x, y), vec2(x + 1, y + 1), vec2(x, y + 1)]);
            march(&[
                vec2(x, y),
                vec2(x + 1, y),
                vec2(x + 1, y + 1),
                vec2(x, y + 1),
            ]);
        }
    }
    result
}

fn generate_mesh<V: From<Vertex>>(image: &geng::image::RgbaImage, options: &Options) -> Vec<V> {
    let image_size = vec2(image.width(), image.height());
    let blurred = geng::image::imageops::blur(image, options.blur_sigma);
    let iso = options.iso;

    let cells = Aabb2::ZERO
        .extend_positive(
            image_size
                .map(|x| (x as i32 + options.cell_size as i32 - 1) / options.cell_size as i32),
        )
        .extend_uniform(2);

    let faces = marching_triangles(
        cells,
        |cell_pos| {
            let pos = cell_pos * options.cell_size as i32;
            if pos.x < 0
                || pos.y < 0
                || pos.x >= image.width() as i32
                || pos.y >= image.height() as i32
            {
                return 0.0;
            }
            let vec2(x, y) = pos.map(|x| x as u32);
            blurred.get_pixel(x, image_size.y - 1 - y)[3] as f32 / u8::MAX as f32
        },
        options.iso,
    );

    let normals: BTreeMap<[R32; 2], vec2<f32>> = faces
        .iter()
        .flat_map(|face| {
            face.iter()
                .circular_tuple_windows()
                .filter_map(|(a, b)| {
                    let normal = (b.pos - a.pos).rotate_90().normalize_or_zero();
                    (a.value == iso && b.value == iso).then_some([(a.pos, normal), (b.pos, normal)])
                })
                .flatten()
        })
        .map(|(pos, normal)| (**pos.map(r32), normal))
        .collect();

    let front = options
        .front_face
        .then_some(faces.iter().flatten().map(|v| (v, 1.0)))
        .into_iter()
        .flatten();
    let back = options
        .back_face
        .then_some(
            faces
                .iter()
                .flat_map(|face| face.iter().rev())
                .map(|v| (v, -1.0)),
        )
        .into_iter()
        .flatten();

    let side = faces.iter().flat_map(|face| {
        face.iter()
            .circular_tuple_windows()
            .filter_map(|(a, b)| {
                (a.value == iso && b.value == iso).then_some([
                    (a, 1.0),
                    (a, -1.0),
                    (b, -1.0),
                    (a, 1.0),
                    (b, -1.0),
                    (b, 1.0),
                ])
            })
            .flatten()
    });

    itertools::chain![front, back, side]
        .map(|(v, z)| {
            let normal = normals.get(&**v.pos.map(r32)).copied();
            let pixel_pos = v.pos.map(|x| x * options.cell_size as f32);
            let uv = pixel_pos / image_size.map(|x| x as f32);
            Vertex {
                a_pos: uv.map(|x| x * 2.0 - 1.0).extend(z),
                a_uv: uv,
                a_normal: normal
                    .map(|normal| normal.extend(0.0))
                    .unwrap_or(vec3(0.0, 0.0, 1.0)),
            }
        })
        .map(|mut v| {
            v.a_pos.z *= options.thickness * 0.5;
            match options.scaling {
                ScalingMode::FixedHeight(height) => {
                    v.a_pos.y *= height * 0.5;
                    v.a_pos.x *= height * 0.5 * image_size.map(|x| x as f32).aspect();
                }
            }
            v
        })
        .map(|v| v.into())
        .collect()
}

#[derive(Debug, Copy, Clone)]
pub enum ScalingMode {
    FixedHeight(f32),
}

#[derive(Debug, Copy, Clone)]
pub struct Options {
    pub blur_sigma: f32,
    pub cell_size: usize,
    pub iso: f32,
    pub thickness: f32,
    pub scaling: ScalingMode,
    pub front_face: bool,
    pub back_face: bool,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            blur_sigma: 10.0,
            cell_size: 10,
            iso: 0.5,
            thickness: 0.01,
            scaling: ScalingMode::FixedHeight(1.0),
            front_face: true,
            back_face: true,
        }
    }
}

fn fix_texture(ugli: &Ugli, texture: &ugli::Texture) -> ugli::Texture {
    let framebuffer =
        ugli::FramebufferRead::new_color(ugli, ugli::ColorAttachmentRead::Texture(texture));
    let data = framebuffer.read_color();
    let mut queue = VecDeque::new();
    let mut visited = HashSet::new();
    for x in 0..texture.size().x {
        for y in 0..texture.size().y {
            if data.get(x, y).a == u8::MAX {
                queue.push_back((vec2(x, y), vec2(x, y)));
                visited.insert(vec2(x, y));
            }
        }
    }
    let mut new_data = vec![vec![Rgba::<f32>::BLUE; texture.size().y]; texture.size().x];
    while let Some((v, nearest)) = queue.pop_front() {
        new_data[v.x][v.y] = data.get(nearest.x, nearest.y).convert();
        for d in [vec2(-1, 0), vec2(1, 0), vec2(0, 1), vec2(0, -1)] {
            let nv = v.map(|x| x as i32) + d;
            if nv.x < 0 || nv.y < 0 {
                continue;
            }
            let nv = nv.map(|x| x as usize);
            if nv.x >= texture.size().x || nv.y >= texture.size().y {
                continue;
            }
            if visited.contains(&nv) {
                continue;
            }
            queue.push_back((nv, nearest));
            visited.insert(nv);
        }
    }
    ugli::Texture::new_with(ugli, texture.size(), |pos| new_data[pos.x][pos.y])
}

impl<V: ugli::Vertex + From<Vertex> + 'static> geng::asset::Load for ThickSprite<V> {
    type Options = Options;
    fn load(
        manager: &geng::asset::Manager,
        path: &std::path::Path,
        options: &Self::Options,
    ) -> geng::asset::Future<Self> {
        let path = path.to_owned();
        let manager = manager.clone();
        let options = *options;
        async move {
            let image: geng::image::RgbaImage = manager.load(path).await?;
            Ok(Self::new(manager.ugli(), &image, &options))
        }
        .boxed_local()
    }
    const DEFAULT_EXT: Option<&'static str> = Some("png");
}
