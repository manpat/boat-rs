#![feature(generators, generator_trait, box_syntax)]
#![feature(inclusive_range_syntax)]
#![feature(specialization)]
#![feature(ord_max_min)]
#![feature(link_args)]
#![feature(const_fn)]

extern crate common;

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

		let view_mat = Mat4::translate(Vec3::new(0.0, 0.0,-6.0))
			* Mat4::xrot(PI/8.0)
			* Mat4::yrot(PI/4.0);

		let shader = Shader::new(res::shaders::BASIC_VS, res::shaders::BASIC_FS);
		shader.use_program();
		shader.set_view(&Mat4::ident());

		let drag_threshold = 50.0;
		let mut drag_start = None;

		let mut target_heading = PI/4.0;
		let mut target_speed = 0.0;

		let mut boat_heading = target_heading;
		let mut boat_speed = 0.0;

		let mut time = 0.0f32;

		loop {
			let frame_start = Instant::now();

			use events::Event;

			for e in event_queue.iter() {
				match *e {
					Event::Resize(sz) => unsafe {
						gl::Viewport(0, 0, sz.x, sz.y);

						let aspect = sz.x as f32 / sz.y as f32;
						let proj_mat = Mat4::perspective(PI/5.0, aspect, 1.0, 100.0);
						let proj_view = proj_mat * view_mat;

						shader.set_proj(&proj_view);
					}

					Event::Down(pos) => {
						drag_start = Some(pos);
					}

					Event::Move(pos) => {
						let drag_start = drag_start.unwrap_or(pos);
						let diff = pos - drag_start;
						let dist = diff.length();

						if dist > drag_threshold {
							target_speed = (dist - drag_threshold).min(100.0) / 100.0;
							target_heading = diff.to_vec2().to_angle() - PI/4.0;

						} else {
							target_speed = 0.0;
						}
					}

					Event::Up(_) => {
						drag_start = None;
					}
				}
			}

			time += 1.0 / 60.0;

			event_queue.clear();

			boat_speed += (target_speed - boat_speed) / 60.0;

			let mut heading_diff = target_heading - boat_heading;
			if heading_diff.abs() > PI {
				heading_diff = 2.0 * PI - heading_diff.abs();
			}

			boat_heading += heading_diff.max(-PI/6.0).min(PI/6.0) / 60.0;

			console::set_section("boat_heading", format!("{}", boat_heading));
			console::set_section("boat_speed", format!("{}", boat_speed));

			let boat_model_mat = Mat4::yrot(boat_heading)
				* Mat4::translate(Vec3::new(0.0, 0.05 * (time*PI/2.0).sin() * (time*PI/3.0).sin(), 0.0));

			shader.set_view(&boat_model_mat);
			boat_mesh.bind();
			boat_mesh.draw(gl::TRIANGLES);

			shader.set_view(&Mat4::ident());
			sea_mesh.bind();
			sea_mesh.draw(gl::TRIANGLES);

			let dur = frame_start.elapsed();
			console::set_section("Stats", format!("frame time: {:.1}ms", dur.subsec_nanos() as f64 / 1000_000.0));
			console::update();

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
