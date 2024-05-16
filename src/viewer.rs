use std::path::Path;

use geng::prelude::itertools::Itertools;

use super::*;
use geng_egui::*;

#[derive(ugli::Vertex, Clone, Copy)]
pub struct Vertex {
    a_pos: vec3<f32>,
    a_uv: vec2<f32>,
    a_color: Rgba<f32>,
}

impl From<geng_sprite_shape::Vertex> for Vertex {
    fn from(value: geng_sprite_shape::Vertex) -> Self {
        Self {
            a_pos: value.a_pos,
            a_uv: value.a_uv,
            a_color: Hsla::new(thread_rng().gen(), 0.5, 0.5, 1.0).into(),
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
    wireframe: bool,
    culling: bool,
}

impl Default for ViewerOptions {
    fn default() -> Self {
        Self {
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
    async fn new(geng: &Geng, path: impl AsRef<Path>, options: &sprite_shape::Options) -> Self {
        let shape: sprite_shape::ThickSprite<Vertex> =
            geng.asset_manager().load_with(path, options).await.unwrap();
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
    options: ViewerOptions,
    white_texture: ugli::Texture,
    config: Config,
    framebuffer_size: vec2<f32>,
    camera: Camera,
    sprite_options: sprite_shape::Options,
    path: PathBuf,
    sprite: Sprite,
    drag: Option<vec2<f64>>,
    should_quit: bool,
    egui: EguiGeng,
    should_reload: bool,
}

impl Viewer {
    pub async fn new(
        geng: &Geng,
        path: impl AsRef<Path>,
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
        Self {
            egui: EguiGeng::new(geng),
            geng: geng.clone(),
            framebuffer_size: vec2::splat(1.0),
            shaders,
            white_texture: ugli::Texture::new_with(geng.ugli(), vec2::splat(1), |_| Rgba::WHITE),
            sprite: Sprite::new(geng, path.as_ref(), &sprite_options).await,
            path: path.as_ref().to_owned(),
            sprite_options,
            camera: Camera {
                fov: Angle::from_degrees(config.camera.fov),
                rotation: Angle::from_degrees(config.camera.rotation),
                attack_angle: Angle::from_degrees(config.camera.attack_angle),
                distance: config.camera.distance,
            },
            drag: None,
            config,
            options: default(),
            should_quit: false,
            should_reload: false,
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
            ui.heading("viewer options");
            ui.checkbox(&mut self.options.wireframe, "wireframe");
            ui.checkbox(&mut self.options.culling, "culling");

            ui.heading("sprite options");
            ui.horizontal(|ui| {
                ui.label("Your name: ");
                // ui.text_edit_singleline(&mut self.name);
            });
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
                    egui::Slider::new(&mut self.sprite_options.cell_size, 1..=50)
                        .text("cell_size"),
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
            // if ui.button("Click each year").clicked() {
            //     self.age += 1;
            // }
            // ui.label(format!("Hello '{}', age {}", self.name, self.age));
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
            Some(self.config.background_color),
            Some(1.0),
            None,
        );
        if self.options.wireframe {
            ugli::draw(
                framebuffer,
                &self.shaders.wireframe,
                ugli::DrawMode::Lines { line_width: 1.0 },
                &self.sprite.wireframe_geometry,
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
            &self.sprite.shape.mesh,
            (
                ugli::uniforms! {
                    u_texture: &self.sprite.shape.texture,
                },
                self.camera.uniforms(self.framebuffer_size),
            ),
            ugli::DrawParameters {
                depth_func: Some(ugli::DepthFunc::Less),
                cull_face: self.options.culling.then_some(ugli::CullFace::Back),
                ..default()
            },
        );

        self.egui.draw(framebuffer);
    }
    fn handle_event(&mut self, event: geng::Event) {
        self.egui.handle_event(event.clone());
        match event {
            geng::Event::KeyPress { key } => match key {
                geng::Key::C => {
                    self.options.culling = !self.options.culling;
                }
                geng::Key::W => {
                    self.options.wireframe = !self.options.wireframe;
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
            self.sprite = Sprite::new(&self.geng, &self.path, &self.sprite_options).await;
            self.should_reload = false;
        }
    }

    pub async fn run(mut self) {
        let geng = self.geng.clone();
        let mut timer = Timer::new();
        while let Some(event) = geng.window().events().next().await {
            if let geng::Event::Draw = event {
                self.update(timer.tick());
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
