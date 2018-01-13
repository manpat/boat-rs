#![feature(generators, generator_trait, box_syntax)]
#![feature(inclusive_range_syntax)]
#![feature(specialization)]
#![feature(ord_max_min)]
#![feature(link_args)]
#![feature(const_fn)]

extern crate common;
extern crate noise;

pub use resources as res;
pub use common::*;

#[macro_use] pub mod bindings;
#[macro_use] pub mod coro_util;

pub mod mut_rc;

pub mod resources;
pub mod rendering;
pub mod console;
pub mod webgl;

mod events;

use coro_util::*;
use webgl::*;

use rendering::*;
use rendering::mesh_builder::*;

use std::time::Instant;

#[repr(C)]
#[derive(Copy, Clone)]
pub struct ColorVertex(Vec3, Vec3);

impl Vertex for ColorVertex {
	fn get_layout() -> VertexLayout {
		VertexLayout::new::<Self>()
			.add_binding(0, 3, 0)
			.add_binding(1, 3, 12)
	}
}

const CAMERA_PITCH: f32 = PI/8.0;
const CAMERA_YAW: f32 = PI/4.0;
const CAMERA_FOV: f32 = PI/4.0;
const CAMERA_DISTANCE: f32 = 12.0;

// #[link_args = "-s ASSERTIONS=1"] extern "C" {}
// #[link_args = "-g4"] extern "C" {}

fn main() {
	set_coro_as_main_loop(|| {
		console::init();
		console::set_color("#222");

		let gl_ctx = WebGLContext::new();
		gl_ctx.set_background(Color::grey_a(0.0, 0.0));

		let mut event_queue = Vec::new();

		events::init_event_queue(&mut event_queue);

		unsafe {
			gl::Enable(gl::DEPTH_TEST);
			gl::Enable(gl::BLEND);
			gl::BlendEquation(gl::FUNC_ADD);
			gl::BlendFunc(gl::SRC_ALPHA, gl::ONE_MINUS_SRC_ALPHA);
		}

		let boat_mesh: Mesh = {
			use ColorVertex as V;

			let mut mb = MeshBuilder::new();
			let color = Color::rgb8(211, 126, 78).into();
			let color2 = Color::rgb8(187, 97, 53).into();

			let vs = [
				V(Vec3::new(-0.5, 0.2, 0.5), color),
				V(Vec3::new( 0.9, 0.2, 0.0), color),
				V(Vec3::new(-0.5, 0.2,-0.5), color),

				V(Vec3::new(-0.1,-0.3, 0.0), color2),
			];

			let es = [
				0, 1, 2,

				0, 3, 1,
				1, 3, 2,
				2, 3, 0,
			];

			mb.add_direct(&vs, &es);

			let mast_color = Color::rgb8(187, 112, 70).into();
			let sail_color = Color::rgb8(171, 220, 107).into();

			let mast_width = 0.02 * 2.0f32.sqrt();
			let mast_offset = Vec3::new(0.5, 0.0, 0.0);
			let mast_height = 1.5;

			mb.add_quad(&[
				V(Vec3::new(-mast_width,        0.18,-mast_width) + mast_offset, mast_color),
				V(Vec3::new( mast_width,        0.18, mast_width) + mast_offset, mast_color),
				V(Vec3::new( mast_width, mast_height, mast_width) + mast_offset, mast_color),
				V(Vec3::new(-mast_width, mast_height,-mast_width) + mast_offset, mast_color),
			]);

			mb.add_quad(&[
				V(Vec3::new(-mast_width,        0.18, mast_width) + mast_offset, mast_color),
				V(Vec3::new( mast_width,        0.18,-mast_width) + mast_offset, mast_color),
				V(Vec3::new( mast_width, mast_height,-mast_width) + mast_offset, mast_color),
				V(Vec3::new(-mast_width, mast_height, mast_width) + mast_offset, mast_color),
			]);

			mb.add_convex_poly(&[
				V(Vec3::new(-0.0, mast_height, 0.0) + mast_offset, sail_color),

				V(Vec3::new(-0.45, 0.25, 0.0), sail_color),
				V(Vec3::new(-0.1, 0.25, 0.1), sail_color),
				V(Vec3::new(-0.0, 0.25, 0.0) + mast_offset, sail_color),
			]);

			mb.into()
		};

		let sea_mesh: Mesh = {
			use ColorVertex as V;

			let mut mb = MeshBuilder::new();
			let color = Color::rgb8(122, 158, 198).into();

			mb.add_quad(&[
				V(Vec3::new(-1.0, 0.0,-1.0), color),
				V(Vec3::new( 1.0, 0.0,-1.0), color),
				V(Vec3::new( 1.0, 0.0, 1.0), color),
				V(Vec3::new(-1.0, 0.0, 1.0), color),
			]);

			mb.into()
		};

		let view_mat = Mat4::translate(Vec3::new(0.0, 0.0,-CAMERA_DISTANCE))
			* Mat4::xrot(CAMERA_PITCH)
			* Mat4::yrot(CAMERA_YAW);

		let shader = Shader::new(res::shaders::BASIC_VS, res::shaders::BASIC_FS);
		shader.use_program();
		shader.set_view(&Mat4::ident());

		let drag_threshold = 50.0;
		let mut drag_start = None;

		let mut wave_phase = 0.0;

		let mut target_heading = -3.0 * CAMERA_YAW / 2.0;
		let mut target_speed = 0.0;

		let mut boat_heading_rate = 0.0;
		let mut boat_heading = target_heading;
		let mut boat_speed = 0.0;

		loop {
			let frame_start = Instant::now();

			use events::Event;

			for e in event_queue.iter() {
				match *e {
					Event::Resize(sz) => unsafe {
						gl::Viewport(0, 0, sz.x, sz.y);

						let aspect = sz.x as f32 / sz.y as f32;
						let proj_mat = Mat4::perspective(CAMERA_FOV, aspect, 1.0, 100.0);
						let proj_view = proj_mat * view_mat;

						shader.set_proj(&proj_view);
					}

					Event::Down(pos) => {
						drag_start = Some(pos);
					}

					Event::Move(pos) => if drag_start.is_some() {
						let drag_start = drag_start.unwrap_or(pos);
						let diff = pos - drag_start;
						let dist = diff.length();

						if dist > drag_threshold {
							target_speed = (dist - drag_threshold).min(100.0) / 100.0;
							target_heading = diff.to_vec2().to_angle() - CAMERA_YAW;

						} else {
							target_speed = 0.0;
						}
					}

					Event::Up(_) => {
						drag_start = None;
					}
				}
			}

			event_queue.clear();

			boat_speed += (target_speed - boat_speed) / 60.0;

			let mut heading_diff = target_heading - boat_heading;
			if heading_diff.abs() > PI {
				heading_diff -= 2.0 * PI * heading_diff.signum();
			}

			let heading_factor = 1.0 / 30.0;

			boat_heading_rate *= 1.0 - heading_factor;
			boat_heading_rate += heading_diff.max(-PI/6.0).min(PI/6.0) * heading_factor;
			boat_heading += (1.0 - (1.0 - boat_heading_rate/PI).powf(1.2)) * PI / 60.0;

			let wave_phase_offset = (wave_phase*PI/5.0).sin();
			let wave_omega = wave_phase*PI/3.0 + wave_phase_offset;

			let wave_translate = wave_omega.sin();
			let wave_slope = wave_omega.cos() * (PI/5.0 * (wave_phase*PI/5.0).cos() + PI/3.0);
			// wolfram alpha: d sin(sin(pi x/5) + pi x/3) / dx

			let boat_roll = boat_heading_rate / 3.0;
			let boat_translate = 0.05 * wave_translate - 0.6 * boat_roll.abs() / PI;

			wave_phase += 1.0/60.0 + boat_speed * 1.0 / 60.0;

			console::set_section("boat_heading_rate", format!("{}", boat_heading_rate));
			console::set_section("boat_heading", format!("{}", boat_heading));
			console::set_section("boat_speed", format!("{}", boat_speed));
			console::set_section("boat_roll", format!("{}", boat_roll));

			let boat_model_mat = Mat4::translate(Vec3::new(0.0, boat_translate, 0.0))
				* Mat4::yrot(boat_heading)
				* Mat4::xrot(boat_roll)
				* Mat4::zrot(PI / 64.0 * wave_slope);

			shader.set_view(&boat_model_mat);
			boat_mesh.bind();
			boat_mesh.draw(gl::TRIANGLES);

			shader.set_view(&Mat4::ident());
			sea_mesh.bind();
			sea_mesh.draw(gl::TRIANGLES);

			let now = Instant::now();
			if now > frame_start {
				let dur = now - frame_start;
				console::set_section("Stats", format!("frame time: {:.1}ms", dur.subsec_nanos() as f64 / 1000_000.0));
				console::update();
			}

			yield;
		}
	});
}

#[allow(dead_code)]
fn screen_to_gl(screen_size: Vec2i, v: Vec2i) -> Vec2{
	let sz = screen_size.to_vec2();
	let aspect = sz.x as f32 / sz.y as f32;

	let norm = v.to_vec2() / screen_size.to_vec2() * 2.0 - Vec2::splat(1.0);
	norm * Vec2::new(aspect, -1.0)
}
