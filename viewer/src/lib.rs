use js_sys::Float32Array;
use layerprocgen::common::{GridBounds, Point, RandomHash};
use layerprocgen::samples::SampleWorld;
use wasm_bindgen::prelude::*;
use web_sys::{
    HtmlCanvasElement, WebGl2RenderingContext as Gl, WebGlBuffer, WebGlProgram, WebGlShader,
};

struct ViewAngles {
    yaw: f64,
    pitch: f64,
    yaw_sin: f64,
    yaw_cos: f64,
    pitch_sin: f64,
    pitch_cos: f64,
}
impl ViewAngles {
    fn new(yaw: f64, pitch: f64) -> Self {
        let (yaw_sin, yaw_cos) = yaw.sin_cos();
        let (pitch_sin, pitch_cos) = pitch.sin_cos();
        Self {
            yaw,
            pitch,
            yaw_sin,
            yaw_cos,
            pitch_sin,
            pitch_cos,
        }
    }
}

struct RouteScene {
    ground: Vec<f64>,
    width: usize,
    rows: usize,
    spacing: f64,
    min_x: f64,
    min_z: f64,
    points: Vec<[f64; 3]>,
    surface_width: f64,
}

#[wasm_bindgen]
pub struct Viewer {
    world: SampleWorld,
    gl: Gl,
    program: WebGlProgram,
    buffer: WebGlBuffer,
    x: f64,
    y: f64,
    span: f64,
    view: ViewAngles,
    aspect: f64,
    bounds: Option<GridBounds>,
    paused: bool,
    stopped: bool,
    vertices: Vec<f32>,
    route: Option<std::rc::Rc<RouteScene>>,
}

#[wasm_bindgen]
impl Viewer {
    #[wasm_bindgen(constructor)]
    pub fn new(canvas: HtmlCanvasElement) -> Result<Viewer, JsValue> {
        let gl = canvas
            .get_context("webgl2")?
            .ok_or_else(|| {
                JsValue::from_str("WebGL2 is unavailable. Enable WebGL2 in your browser.")
            })?
            .dyn_into::<Gl>()?;
        let vertex = shader(
            &gl,
            Gl::VERTEX_SHADER,
            r#"#version 300 es
            in vec3 position;
            in vec3 color;
            out vec3 v_color;
            void main() { gl_Position = vec4(position, 1.0); v_color = color; }
        "#,
        )?;
        let fragment = shader(
            &gl,
            Gl::FRAGMENT_SHADER,
            r#"#version 300 es
            precision mediump float;
            in vec3 v_color;
            out vec4 out_color;
            void main() { out_color = vec4(v_color, 1.0); }
        "#,
        )?;
        let program = gl
            .create_program()
            .ok_or_else(|| JsValue::from_str("Cannot allocate WebGL program"))?;
        gl.attach_shader(&program, &vertex);
        gl.attach_shader(&program, &fragment);
        gl.bind_attrib_location(&program, 0, "position");
        gl.bind_attrib_location(&program, 1, "color");
        gl.link_program(&program);
        gl.delete_shader(Some(&vertex));
        gl.delete_shader(Some(&fragment));
        if !gl
            .get_program_parameter(&program, Gl::LINK_STATUS)
            .as_bool()
            .unwrap_or(false)
        {
            return Err(JsValue::from_str(
                &gl.get_program_info_log(&program).unwrap_or_default(),
            ));
        }
        let buffer = gl
            .create_buffer()
            .ok_or_else(|| JsValue::from_str("Cannot allocate WebGL buffer"))?;
        gl.enable(Gl::DEPTH_TEST);
        let world = SampleWorld::new().map_err(js_error)?;
        Ok(Self {
            world,
            gl,
            program,
            buffer,
            x: 0.0,
            y: 0.0,
            span: 520.0,
            view: ViewAngles::new(0.6_f64.atan2(0.8), std::f64::consts::FRAC_PI_6),
            aspect: 1.0,
            bounds: None,
            paused: false,
            stopped: false,
            vertices: Vec::new(),
            route: None,
        })
    }

    pub fn show_route(
        &mut self,
        ground: &[f64],
        points: &[f64],
        meta: &[f64],
    ) -> Result<(), JsValue> {
        if meta.len() != 6
            || meta
                .iter()
                .chain(ground)
                .chain(points)
                .any(|v| !v.is_finite())
        {
            return Err(JsValue::from_str("Invalid route display data"));
        }
        let width = meta[0] as usize;
        let rows = meta[1] as usize;
        if !(2..=512).contains(&width)
            || !(2..=512).contains(&rows)
            || width as f64 != meta[0]
            || rows as f64 != meta[1]
            || ground.len() != width * rows
            || points.len() < 6
            || points.len() > 30000
            || !points.len().is_multiple_of(3)
            || meta[2] <= 0.
            || meta[5] <= 0.
        {
            return Err(JsValue::from_str("Invalid route display dimensions"));
        }
        self.shutdown();
        self.route = Some(std::rc::Rc::new(RouteScene {
            ground: ground.to_vec(),
            width,
            rows,
            spacing: meta[2],
            min_x: meta[3],
            min_z: meta[4],
            points: points.as_chunks::<3>().0.to_vec(),
            surface_width: meta[5],
        }));
        self.stopped = false;
        self.x = 0.;
        self.y = 0.;
        self.span = 110.;
        self.set_angles(0.6_f64.atan2(0.8), std::f64::consts::FRAC_PI_4)
    }
    pub fn show_points(&mut self) -> Result<(), JsValue> {
        self.route = None;
        self.x = 0.;
        self.y = 0.;
        self.span = 520.;
        self.restart()
    }

    pub fn set_position(&mut self, x: f64, y: f64) -> Result<(), JsValue> {
        if !x.is_finite() || !y.is_finite() || x.abs() > 10_000_000.0 || y.abs() > 10_000_000.0 {
            return Err(JsValue::from_str(
                "Camera coordinates must be finite and within 10 million units",
            ));
        }
        self.x = x;
        self.y = y;
        Ok(())
    }

    pub fn pan(&mut self, dx: f64, dy: f64) -> Result<(), JsValue> {
        self.set_position(self.x + dx, self.y + dy)
    }

    pub fn set_angles(&mut self, yaw: f64, pitch: f64) -> Result<(), JsValue> {
        if !yaw.is_finite() || !pitch.is_finite() {
            return Err(JsValue::from_str("Camera angles must be finite"));
        }
        self.view = ViewAngles::new(
            yaw.rem_euclid(std::f64::consts::TAU),
            pitch.clamp(15_f64.to_radians(), 80_f64.to_radians()),
        );
        Ok(())
    }

    pub fn pan_screen(&mut self, dx: f64, dy: f64) -> Result<(), JsValue> {
        let ground_y = dy / self.view.pitch_sin;
        self.pan(
            dx * self.view.yaw_cos + ground_y * self.view.yaw_sin,
            -dx * self.view.yaw_sin + ground_y * self.view.yaw_cos,
        )
    }

    pub fn set_zoom(&mut self, span: f64) -> Result<(), JsValue> {
        if !span.is_finite() {
            return Err(JsValue::from_str("Zoom must be finite"));
        }
        self.span = if self.route.is_some() {
            span.clamp(40., 800.)
        } else {
            span.clamp(220., 1200.)
        };
        Ok(())
    }

    pub fn set_paused(&mut self, paused: bool) {
        self.paused = paused;
    }

    pub fn shutdown(&mut self) {
        self.world.shutdown();
        self.world.process(usize::MAX);
        self.stopped = true;
    }

    pub fn restart(&mut self) -> Result<(), JsValue> {
        if self.route.is_some() {
            self.stopped = false;
            return Ok(());
        }
        self.shutdown();
        self.world = SampleWorld::new().map_err(js_error)?;
        self.bounds = None;
        self.stopped = false;
        Ok(())
    }

    pub fn frame(&mut self, width: i32, height: i32) -> Result<(), JsValue> {
        if width <= 0 || height <= 0 || width > 16384 || height > 16384 {
            return Err(JsValue::from_str("Invalid canvas dimensions"));
        }
        self.aspect = f64::from(width) / f64::from(height);
        if self.route.is_some() {
            self.draw_route_scene(width, height);
            return Ok(());
        }
        if !self.stopped {
            let radius = self.view_radius();
            let center = Point::new(self.x as i32, self.y as i32);
            let min = Point::new(
                (center.x - radius).div_euclid(300) * 300,
                (center.y - radius).div_euclid(300) * 300,
            );
            let max = Point::new(
                (center.x + radius).div_euclid(300) * 300 + 300,
                (center.y + radius).div_euclid(300) * 300 + 300,
            );
            let bounds = GridBounds::new(min, max);
            if self.bounds != Some(bounds) {
                self.world.move_to(bounds).map_err(js_error)?;
                self.bounds = Some(bounds);
            }
            if !self.paused {
                self.world.process(usize::MAX);
            }
        }
        self.gl.viewport(0, 0, width, height);
        self.gl.clear_color(0.035, 0.055, 0.075, 1.0);
        self.gl.clear(Gl::COLOR_BUFFER_BIT | Gl::DEPTH_BUFFER_BIT);
        self.vertices.clear();
        let radius = self.view_radius();
        let start_x = ((self.x as i32 - radius).div_euclid(100)) * 100;
        let end_x = self.x as i32 + radius;
        let start_y = ((self.y as i32 - radius).div_euclid(100)) * 100;
        let end_y = self.y as i32 + radius;
        for x in (start_x..end_x).step_by(100) {
            let color = if x % 300 == 0 {
                [0.16, 0.27, 0.32]
            } else {
                [0.08, 0.13, 0.17]
            };
            self.vertex(f64::from(x), -1.0, f64::from(start_y), color);
            self.vertex(f64::from(x), -1.0, f64::from(end_y), color);
        }
        for y in (start_y..end_y).step_by(100) {
            let color = if y % 300 == 0 {
                [0.16, 0.27, 0.32]
            } else {
                [0.08, 0.13, 0.17]
            };
            self.vertex(f64::from(start_x), -1.0, f64::from(y), color);
            self.vertex(f64::from(end_x), -1.0, f64::from(y), color);
        }
        self.draw(Gl::LINES);
        self.vertices.clear();
        for (_, point) in self.world.objects() {
            let variation = RandomHash::new(11).hash_values(&[point.x as u32, point.y as u32])
                as f64
                / f64::from(u32::MAX);
            self.cube(point, 16.0 + variation * 20.0, variation as f32);
        }
        self.draw(Gl::TRIANGLES);
        Ok(())
    }

    pub fn stats(&self) -> String {
        let stats = self.world.stats();
        let objects = self.world.objects();
        let positions = objects
            .iter()
            .map(|(_, p)| format!("[{},{}]", p.x, p.y))
            .collect::<Vec<_>>()
            .join(",");
        format!(
            r#"{{"renderer":"webgl2","mode":"{}","x":{},"y":{},"span":{},"yaw":{},"pitch":{},"objects":{},"created":{},"destroyed":{},"pending":{},"paused":{},"stopped":{},"positions":[{}]}}"#,
            if self.route.is_some() {
                "route"
            } else {
                "points"
            },
            self.x,
            self.y,
            self.span,
            self.view.yaw,
            self.view.pitch,
            objects.len(),
            stats.created,
            stats.destroyed,
            stats.pending,
            self.paused,
            self.stopped,
            positions
        )
    }
}

impl Viewer {
    fn draw_route_scene(&mut self, width: i32, height: i32) {
        let scene = self.route.as_ref().unwrap().clone();
        self.gl.viewport(0, 0, width, height);
        self.gl.clear_color(0.035, 0.055, 0.075, 1.);
        self.gl.clear(Gl::COLOR_BUFFER_BIT | Gl::DEPTH_BUFFER_BIT);
        if self.stopped {
            return;
        }
        self.vertices.clear();
        let low = scene.ground.iter().copied().fold(f64::INFINITY, f64::min);
        let high = scene
            .ground
            .iter()
            .copied()
            .fold(f64::NEG_INFINITY, f64::max);
        for z in 0..scene.rows - 1 {
            for x in 0..scene.width - 1 {
                for (dx, dz) in [(0, 0), (1, 0), (1, 1), (0, 0), (1, 1), (0, 1)] {
                    let h = scene.ground[(z + dz) * scene.width + x + dx];
                    let t = ((h - low) / (high - low).max(1e-9)) as f32;
                    self.vertex(
                        scene.min_x + (x + dx) as f64 * scene.spacing,
                        h * 80.,
                        scene.min_z + (z + dz) as f64 * scene.spacing,
                        [0.10 + t * 0.16, 0.24 + t * 0.24, 0.18 + t * 0.09],
                    );
                }
            }
        }
        self.draw(Gl::TRIANGLES);
        self.vertices.clear();
        for pair in scene.points.windows(2) {
            let a = pair[0];
            let b = pair[1];
            let dx = b[0] - a[0];
            let dz = b[2] - a[2];
            let length = dx.hypot(dz);
            if length == 0. {
                continue;
            }
            let nx = -dz / length * scene.surface_width / 2.;
            let nz = dx / length * scene.surface_width / 2.;
            let corners = [
                [a[0] + nx, a[1] * 80. + 0.5, a[2] + nz],
                [a[0] - nx, a[1] * 80. + 0.5, a[2] - nz],
                [b[0] + nx, b[1] * 80. + 0.5, b[2] + nz],
                [b[0] - nx, b[1] * 80. + 0.5, b[2] - nz],
            ];
            for i in [0, 1, 2, 1, 3, 2] {
                let p = corners[i];
                self.vertex(p[0], p[1], p[2], [0.98, 0.73, 0.20]);
            }
        }
        self.draw(Gl::TRIANGLES);
        self.vertices.clear();
        for (p, color) in [
            (scene.points[0], [0.2, 0.95, 0.95]),
            (*scene.points.last().unwrap(), [1., 0.4, 0.3]),
        ] {
            for (a, b) in [
                ([-2., -2.], [2., -2.]),
                ([2., -2.], [2., 2.]),
                ([2., 2.], [-2., 2.]),
                ([-2., 2.], [-2., -2.]),
            ] {
                self.vertex(p[0] + a[0], p[1] * 80. + 1., p[2] + a[1], color);
                self.vertex(p[0] + b[0], p[1] * 80. + 1., p[2] + b[1], color);
                self.vertex(p[0], p[1] * 80. + 8., p[2], color);
            }
        }
        self.draw(Gl::TRIANGLES);
    }

    fn view_radius(&self) -> i32 {
        let horizontal = self.span * self.aspect;
        let ground = self.span / self.view.pitch_sin;
        let x = self.view.yaw_cos.abs() * horizontal + self.view.yaw_sin.abs() * ground;
        let y = self.view.yaw_sin.abs() * horizontal + self.view.yaw_cos.abs() * ground;
        (x.max(y) + 100.0).ceil().min(6000.0) as i32
    }
    fn vertex(&mut self, x: f64, height: f64, y: f64, color: [f32; 3]) {
        let dx = x - self.x;
        let dy = y - self.y;
        let rx = dx * self.view.yaw_cos - dy * self.view.yaw_sin;
        let ry = dx * self.view.yaw_sin + dy * self.view.yaw_cos;
        self.vertices.extend_from_slice(&[
            (rx / (self.span * self.aspect)) as f32,
            ((height * self.view.pitch_cos - ry * self.view.pitch_sin) / self.span) as f32,
            ((-ry * self.view.pitch_cos - height * self.view.pitch_sin) / 20000.0) as f32,
            color[0],
            color[1],
            color[2],
        ]);
    }

    fn cube(&mut self, point: Point, height: f64, variation: f32) {
        let x = f64::from(point.x);
        let y = f64::from(point.y);
        let corners = [
            [x - 8.0, 0.0, y - 8.0],
            [x + 8.0, 0.0, y - 8.0],
            [x + 8.0, 0.0, y + 8.0],
            [x - 8.0, 0.0, y + 8.0],
            [x - 8.0, height, y - 8.0],
            [x + 8.0, height, y - 8.0],
            [x + 8.0, height, y + 8.0],
            [x - 8.0, height, y + 8.0],
        ];
        for (indices, light) in [
            ([4, 5, 6, 4, 6, 7], 1.0),
            ([0, 1, 5, 0, 5, 4], 0.65),
            ([1, 2, 6, 1, 6, 5], 0.48),
            ([2, 3, 7, 2, 7, 6], 0.57),
            ([3, 0, 4, 3, 4, 7], 0.74),
        ] {
            let color = [
                (0.28 + variation * 0.24) * light,
                (0.68 + variation * 0.14) * light,
                (0.53 + variation * 0.25) * light,
            ];
            for i in indices {
                let p = corners[i];
                self.vertex(p[0], p[1], p[2], color);
            }
        }
    }

    fn draw(&self, mode: u32) {
        self.gl.use_program(Some(&self.program));
        self.gl.bind_buffer(Gl::ARRAY_BUFFER, Some(&self.buffer));
        let data = Float32Array::from(self.vertices.as_slice());
        self.gl
            .buffer_data_with_array_buffer_view(Gl::ARRAY_BUFFER, &data, Gl::DYNAMIC_DRAW);
        self.gl
            .vertex_attrib_pointer_with_i32(0, 3, Gl::FLOAT, false, 24, 0);
        self.gl
            .vertex_attrib_pointer_with_i32(1, 3, Gl::FLOAT, false, 24, 12);
        self.gl.enable_vertex_attrib_array(0);
        self.gl.enable_vertex_attrib_array(1);
        self.gl
            .draw_arrays(mode, 0, (self.vertices.len() / 6) as i32);
    }
}

impl Drop for Viewer {
    fn drop(&mut self) {
        self.world.shutdown();
        self.world.process(usize::MAX);
        self.gl.delete_buffer(Some(&self.buffer));
        self.gl.delete_program(Some(&self.program));
    }
}

fn js_error(error: impl std::fmt::Display) -> JsValue {
    JsValue::from_str(&error.to_string())
}

fn shader(gl: &Gl, kind: u32, source: &str) -> Result<WebGlShader, JsValue> {
    let shader = gl
        .create_shader(kind)
        .ok_or_else(|| JsValue::from_str("Cannot allocate shader"))?;
    gl.shader_source(&shader, source);
    gl.compile_shader(&shader);
    if gl
        .get_shader_parameter(&shader, Gl::COMPILE_STATUS)
        .as_bool()
        .unwrap_or(false)
    {
        Ok(shader)
    } else {
        let message = gl.get_shader_info_log(&shader).unwrap_or_default();
        gl.delete_shader(Some(&shader));
        Err(JsValue::from_str(&message))
    }
}
