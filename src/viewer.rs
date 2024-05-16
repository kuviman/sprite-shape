use super::*;

#[derive(ugli::Vertex)]
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
    sensitivity: f32,
    camera: CameraConfig,
}

struct Viewer {
    geng: Geng,
    config: Config,
    framebuffer_size: vec2<f32>,
    program: ugli::Program,
    camera: Camera,
    sprite: sprite_shape::ThickSprite<Vertex>,
    drag: Option<vec2<f64>>,
}

impl Viewer {
    async fn new(geng: &Geng, sprite: sprite_shape::ThickSprite<Vertex>) -> Self {
        let program: ugli::Program = geng
            .asset_manager()
            .load(run_dir().join("assets").join("shader.glsl"))
            .await
            .unwrap();
        let config: Config = file::load_detect(run_dir().join("assets").join("config.toml"))
            .await
            .unwrap();
        Self {
            geng: geng.clone(),
            framebuffer_size: vec2::splat(1.0),
            program,
            sprite,
            camera: Camera {
                fov: Angle::from_degrees(config.camera.fov),
                rotation: Angle::from_degrees(config.camera.rotation),
                attack_angle: Angle::from_degrees(config.camera.attack_angle),
                distance: config.camera.distance,
            },
            drag: None,
            config,
        }
    }

    fn start_drag(&mut self, pos: vec2<f64>) {
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
}

impl geng::State for Viewer {
    fn draw(&mut self, framebuffer: &mut ugli::Framebuffer) {
        self.framebuffer_size = framebuffer.size().map(|x| x as f32);
        ugli::clear(framebuffer, Some(Rgba::WHITE), Some(1.0), None);
        ugli::draw(
            framebuffer,
            &self.program,
            ugli::DrawMode::Triangles,
            &self.sprite.mesh,
            (
                ugli::uniforms! {
                    u_texture: &self.sprite.texture,
                },
                self.camera.uniforms(self.framebuffer_size),
            ),
            ugli::DrawParameters {
                depth_func: Some(ugli::DepthFunc::Less),
                ..default()
            },
        );
    }
    fn handle_event(&mut self, event: geng::Event) {
        match event {
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
}

pub async fn run(geng: &Geng, sprite: sprite_shape::ThickSprite<Vertex>) {
    geng.run_state(Viewer::new(geng, sprite).await).await;
}
