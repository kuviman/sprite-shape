use geng::prelude::{futures::AsyncReadExt, itertools::Itertools};

use super::*;
use geng_egui::*;

#[derive(ugli::Vertex, Clone, Copy)]
pub struct Vertex {
    pub a_pos: vec3<f32>,
    pub a_uv: vec2<f32>,
}

impl From<geng_sprite_shape::Vertex> for Vertex {
    fn from(value: geng_sprite_shape::Vertex) -> Self {
        Self {
            a_pos: value.a_pos,
            a_uv: value.a_uv,
        }
    }
}

struct Camera {
    rotation: Angle,
    attack_angle: Angle,
    distance: f32,
    fov: Angle,
}

impl AbstractCamera3d for Camera {
    fn view_matrix(&self) -> mat4<f32> {
        mat4::translate(vec3(0.0, 0.0, -self.distance))
            * mat4::rotate_x(self.attack_angle)
            * mat4::rotate_z(self.rotation)
    }
    fn projection_matrix(&self, framebuffer_size: vec2<f32>) -> mat4<f32> {
        mat4::perspective(
            self.fov.as_radians(),
            framebuffer_size.aspect(),
            0.1,
            1000.0,
        )
    }
}

#[derive(Deserialize)]
struct CameraConfig {
    fov: f32,
    distance: f32,
    rotation: f32,
    attack_angle: f32,
}

#[derive(Deserialize)]
struct Config {
    background_color: Rgba<f32>,
    wireframe_color: Rgba<f32>,
    sensitivity: f32,
    camera: CameraConfig,
}

struct ViewerOptions {
    background_color: Rgba<f32>,
    wireframe: bool,
    culling: bool,
}

impl ViewerOptions {
    fn new(config: &Config) -> Self {
        Self {
            background_color: config.background_color,
            wireframe: false,
            culling: true,
        }
    }
}

#[derive(geng::asset::Load)]
struct Shaders {
    program: ugli::Program,
    wireframe: ugli::Program,
}

struct Sprite {
    wireframe_geometry: ugli::VertexBuffer<Vertex>,
    shape: sprite_shape::ThickSprite<Vertex>,
}

impl Sprite {
    fn new(geng: &Geng, image: &geng::image::RgbaImage, options: &sprite_shape::Options) -> Self {
        let shape: sprite_shape::ThickSprite<Vertex> =
            sprite_shape::ThickSprite::new(geng.ugli(), image, options);
        Self {
            wireframe_geometry: ugli::VertexBuffer::new_static(
                geng.ugli(),
                shape
                    .mesh
                    .chunks(3)
                    .flat_map(|face| {
                        face.iter()
                            .circular_tuple_windows()
                            .flat_map(|(a, b)| [a, b])
                    })
                    .cloned()
                    .collect(),
            ),
            shape,
        }
    }
}

pub struct Viewer {
    geng: Geng,
    shaders: Shaders,
    viewer_options: ViewerOptions,
    white_texture: ugli::Texture,
    config: Config,
    framebuffer_size: vec2<f32>,
    camera: Camera,
    sprite_options: sprite_shape::Options,
    image: Option<geng::image::RgbaImage>,
    sprite: Option<Sprite>,
    drag: Option<vec2<f64>>,
    should_quit: bool,
    egui: EguiGeng,
    should_reload: bool,
    file_selection: Rc<RefCell<Option<file_dialog::SelectedFile>>>,
}

impl Viewer {
    pub async fn new(
        geng: &Geng,
        path: Option<PathBuf>,
        sprite_options: sprite_shape::Options,
    ) -> Self {
        let config: Config = file::load_detect(run_dir().join("assets").join("config.toml"))
            .await
            .unwrap();
        let shaders: Shaders = geng
            .asset_manager()
            .load(run_dir().join("assets").join("shaders"))
            .await
            .unwrap();
        let image = match path {
            Some(path) => Some(geng.asset_manager().load(path).await.unwrap()),
            None => None,
        };
        Self {
            egui: EguiGeng::new(geng),
            geng: geng.clone(),
            framebuffer_size: vec2::splat(1.0),
            shaders,
            white_texture: ugli::Texture::new_with(geng.ugli(), vec2::splat(1), |_| Rgba::WHITE),
            sprite: image
                .as_ref()
                .map(|image| Sprite::new(geng, image, &sprite_options)),
            sprite_options,
            image,
            camera: Camera {
                fov: Angle::from_degrees(config.camera.fov),
                rotation: Angle::from_degrees(config.camera.rotation),
                attack_angle: Angle::from_degrees(config.camera.attack_angle),
                distance: config.camera.distance,
            },
            drag: None,
            viewer_options: ViewerOptions::new(&config),
            config,
            should_quit: false,
            should_reload: false,
            file_selection: default(),
        }
    }

    fn start_drag(&mut self, pos: vec2<f64>) {
        if self.egui.get_context().is_pointer_over_area() {
            return;
        }
        self.drag = Some(pos);
    }

    fn cursor_move(&mut self, pos: vec2<f64>) {
        if let Some(prev) = self.drag {
            self.drag = Some(pos);
            let delta = pos - prev;
            self.camera.rotation += Angle::from_degrees(delta.x as f32 * self.config.sensitivity);
            self.camera.attack_angle -=
                Angle::from_degrees(delta.y as f32 * self.config.sensitivity);
            self.camera.attack_angle = self
                .camera
                .attack_angle
                .clamp_range(Angle::from_degrees(-180.0)..=Angle::ZERO);
        }
    }

    fn stop_drag(&mut self) {
        self.drag = None;
    }

    fn ui(&mut self) {
        egui::Window::new("SpriteShape").show(self.egui.get_context(), |ui| {
            ui.heading("sprite options");
            if ui.button("Select image").clicked() {
                let selection = self.file_selection.clone();
                file_dialog::select(move |selected| {
                    selection.replace(Some(selected));
                });
            }
            if ui.button("Export GLTF").clicked() {
                if let Some(sprite) = &self.sprite {
                    let _ = file_dialog::save(
                        "sprite-shape.glb",
                        &glb::save(self.geng.ugli(), &sprite.shape),
                    );
                }
            }
            if ui
                .add(egui::Checkbox::new(
                    &mut self.sprite_options.front_face,
                    "front face",
                ))
                .clicked()
            {
                self.should_reload = true;
            }
            if ui
                .add(egui::Checkbox::new(
                    &mut self.sprite_options.back_face,
                    "back face",
                ))
                .clicked()
            {
                self.should_reload = true;
            }
            if ui
                .add(egui::Slider::new(&mut self.sprite_options.iso, 0.0..=1.0).text("iso"))
                .drag_released()
            {
                self.should_reload = true;
            }
            if ui
                .add(
                    egui::Slider::new(&mut self.sprite_options.blur_sigma, 0.0..=50.0)
                        .text("blur_sigma"),
                )
                .drag_released()
            {
                self.should_reload = true;
            }
            if ui
                .add(
                    egui::Slider::new(&mut self.sprite_options.cell_size, 1..=50).text("cell_size"),
                )
                .drag_released()
            {
                self.should_reload = true;
            }
            if ui
                .add(
                    egui::Slider::new(&mut self.sprite_options.thickness, 0.0..=0.1)
                        .text("thickness"),
                )
                .drag_released()
            {
                self.should_reload = true;
            }

            ui.heading("viewer options");
            ui.checkbox(&mut self.viewer_options.wireframe, "wireframe");
            ui.checkbox(&mut self.viewer_options.culling, "culling");

            ui.label("background color");
            let mut color = self.viewer_options.background_color.to_vec4();
            ui.color_edit_button_rgb((&mut color[..3]).try_into().unwrap());
            self.viewer_options.background_color = Rgba::from_vec4(color);
        });
    }
    fn update(&mut self, _delta_time: time::Duration) {
        self.egui.begin_frame();
        self.ui();
        self.egui.end_frame();
    }
    fn draw(&mut self, framebuffer: &mut ugli::Framebuffer) {
        self.framebuffer_size = framebuffer.size().map(|x| x as f32);
        ugli::clear(
            framebuffer,
            Some(self.viewer_options.background_color),
            Some(1.0),
            None,
        );
        if let Some(sprite) = &self.sprite {
            if self.viewer_options.wireframe {
                ugli::draw(
                    framebuffer,
                    &self.shaders.wireframe,
                    ugli::DrawMode::Lines { line_width: 1.0 },
                    &sprite.wireframe_geometry,
                    (
                        ugli::uniforms! {
                            u_texture: &self.white_texture,
                            u_color: self.config.wireframe_color,
                        },
                        self.camera.uniforms(self.framebuffer_size),
                    ),
                    ugli::DrawParameters {
                        depth_func: Some(ugli::DepthFunc::LessOrEqual),
                        ..default()
                    },
                );
            }
            ugli::draw(
                framebuffer,
                &self.shaders.program,
                ugli::DrawMode::Triangles,
                &sprite.shape.mesh,
                (
                    ugli::uniforms! {
                        u_texture: &sprite.shape.texture,
                    },
                    self.camera.uniforms(self.framebuffer_size),
                ),
                ugli::DrawParameters {
                    depth_func: Some(ugli::DepthFunc::Less),
                    cull_face: self.viewer_options.culling.then_some(ugli::CullFace::Back),
                    ..default()
                },
            );
        }

        self.egui.draw(framebuffer);
    }
    fn handle_event(&mut self, event: geng::Event) {
        self.egui.handle_event(event.clone());
        match event {
            geng::Event::KeyPress { key } => match key {
                geng::Key::C => {
                    self.viewer_options.culling = !self.viewer_options.culling;
                }
                geng::Key::W => {
                    self.viewer_options.wireframe = !self.viewer_options.wireframe;
                }
                geng::Key::Escape => {
                    self.should_quit = true;
                }
                _ => {}
            },
            geng::Event::MousePress { .. } => {
                if let Some(cursor_pos) = self.geng.window().cursor_position() {
                    self.start_drag(cursor_pos);
                }
            }
            geng::Event::CursorMove {
                position: cursor_pos,
            } => {
                self.cursor_move(cursor_pos);
            }
            geng::Event::MouseRelease { .. } => {
                self.stop_drag();
            }
            _ => {}
        }
    }

    async fn maybe_reload(&mut self) {
        if self.should_reload {
            if let Some(image) = &self.image {
                self.sprite = Some(Sprite::new(&self.geng, image, &self.sprite_options));
            }
            self.should_reload = false;
        }
    }

    pub async fn run(mut self) {
        let geng = self.geng.clone();
        let mut timer = Timer::new();
        while let Some(event) = geng.window().events().next().await {
            if let geng::Event::Draw = event {
                self.update(timer.tick());
                if let Some(file) = self.file_selection.take() {
                    if let Ok(mut reader) = file.reader() {
                        let mut buf = Vec::new();
                        if reader.read_to_end(&mut buf).await.is_ok() {
                            match geng::image::load_from_memory(&buf) {
                                Ok(image) => {
                                    self.image = Some(image.into());
                                    self.should_reload = true;
                                }
                                Err(e) => {
                                    log::error!("error: {e}");
                                }
                            }
                        }
                    }
                }
                geng.window().with_framebuffer(|framebuffer| {
                    self.draw(framebuffer);
                });
                self.maybe_reload().await;
            } else {
                self.handle_event(event);
            }
        }
    }
}
