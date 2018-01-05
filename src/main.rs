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

use bindings::emscripten::*;
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

		let mut events = Vec::new();

		unsafe {
			use std::ptr::null;

			let evt_ptr = std::mem::transmute(&mut events);

			on_resize(0, null(), evt_ptr);
			emscripten_set_resize_callback(null(), evt_ptr, 0, Some(on_resize));

			emscripten_set_mousemove_callback(null(), evt_ptr, 0, Some(on_mouse_move));
			emscripten_set_mousedown_callback(null(), evt_ptr, 0, Some(on_mouse_down));
			emscripten_set_mouseup_callback(null(), evt_ptr, 0, Some(on_mouse_up));

			emscripten_set_touchstart_callback(null(), evt_ptr, 0, Some(on_touch_start));
			emscripten_set_touchmove_callback(null(), evt_ptr, 0, Some(on_touch_move));
			emscripten_set_touchend_callback(null(), evt_ptr, 0, Some(on_touch_end));
			emscripten_set_touchcancel_callback(null(), evt_ptr, 0, Some(on_touch_end));

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

		// let mut screen_size = Vec2i::zero();

		// let click_threshold = 50.0;
		// let mut click_start = None;

		let mut time = 0.0f32;

		loop {
			let frame_start = Instant::now();

			for e in events.iter() {
				match *e {
					Event::Resize(sz) => unsafe {
						// screen_size = sz;

						gl::Viewport(0, 0, sz.x, sz.y);

						let aspect = sz.x as f32 / sz.y as f32;
						// let proj = Mat4::scale(Vec3::new(1.0/aspect, 1.0, 1.0));
						let proj_mat = Mat4::perspective(PI/5.0, aspect, 1.0, 100.0);

						let proj_view = proj_mat * view_mat;

						shader.set_proj(&proj_view);
					}

					Event::Move(_pos) => {}
					Event::Down(_pos) => {}
					Event::Up(_pos) => {}
				}
			}

			time += 1.0 / 60.0;

			events.clear();

			let boat_model_mat = Mat4::yrot(time * PI/16.0)
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

fn screen_to_gl(screen_size: Vec2i, v: Vec2i) -> Vec2{
	let sz = screen_size.to_vec2();
	let aspect = sz.x as f32 / sz.y as f32;

	let norm = v.to_vec2() / screen_size.to_vec2() * 2.0 - Vec2::splat(1.0);
	norm * Vec2::new(aspect, -1.0)
}

enum Event {
	Resize(Vec2i),

	Down(Vec2i),
	Up(Vec2i),
	Move(Vec2i),
}

unsafe extern "C"
fn on_resize(_: i32, _e: *const EmscriptenUiEvent, ud: *mut CVoid) -> i32 {
	let event_queue: &mut Vec<Event> = std::mem::transmute(ud);

	js! { b"Module.canvas = document.getElementById('canvas')\0" };

	let mut screen_size = Vec2i::zero();
	screen_size.x = js! { b"return (Module.canvas.width = Module.canvas.style.width = window.innerWidth)\0" };
	screen_size.y = js! { b"return (Module.canvas.height = Module.canvas.style.height = window.innerHeight)\0" };

	event_queue.push(Event::Resize(screen_size));
	
	0
}

unsafe extern "C"
fn on_mouse_move(_: i32, e: *const EmscriptenMouseEvent, ud: *mut CVoid) -> i32 {
	let event_queue: &mut Vec<Event> = std::mem::transmute(ud);
	let e: &EmscriptenMouseEvent = std::mem::transmute(e);

	event_queue.push(Event::Move(Vec2i::new(e.clientX as _, e.clientY as _)));
	console::set_section("Input(mouse)", "move");
	
	1
}
unsafe extern "C"
fn on_mouse_down(_: i32, e: *const EmscriptenMouseEvent, ud: *mut CVoid) -> i32 {
	let event_queue: &mut Vec<Event> = std::mem::transmute(ud);
	let e: &EmscriptenMouseEvent = std::mem::transmute(e);

	event_queue.push(Event::Down(Vec2i::new(e.clientX as _, e.clientY as _)));
	console::set_section("Input(mouse)", "down");
	
	1
}
unsafe extern "C"
fn on_mouse_up(_: i32, e: *const EmscriptenMouseEvent, ud: *mut CVoid) -> i32 {
	let event_queue: &mut Vec<Event> = std::mem::transmute(ud);
	let e: &EmscriptenMouseEvent = std::mem::transmute(e);

	event_queue.push(Event::Up(Vec2i::new(e.clientX as _, e.clientY as _)));
	console::set_section("Input(mouse)", "up");
	
	1
}


unsafe extern "C"
fn on_touch_move(_: i32, e: *const EmscriptenTouchEvent, ud: *mut CVoid) -> i32 {
	let event_queue: &mut Vec<Event> = std::mem::transmute(ud);
	let e: &EmscriptenTouchEvent = std::mem::transmute(e);

	if e.touches[0].identifier != 0 { return 0 }

	let pos = Vec2i::new(e.touches[0].clientX as _, e.touches[0].clientY as _);
	event_queue.push(Event::Move(pos));
	console::set_section("Input(touch)", "move");
	
	1
}

unsafe extern "C"
fn on_touch_start(_: i32, e: *const EmscriptenTouchEvent, ud: *mut CVoid) -> i32 {
	let event_queue: &mut Vec<Event> = std::mem::transmute(ud);
	let e: &EmscriptenTouchEvent = std::mem::transmute(e);

	if e.touches[0].identifier != 0 { return 0 }

	let pos = Vec2i::new(e.touches[0].clientX as _, e.touches[0].clientY as _);
	event_queue.push(Event::Down(pos));
	console::set_section("Input(touch)", "down");
	
	1
}

unsafe extern "C"
fn on_touch_end(_: i32, e: *const EmscriptenTouchEvent, ud: *mut CVoid) -> i32 {
	let event_queue: &mut Vec<Event> = std::mem::transmute(ud);
	let e: &EmscriptenTouchEvent = std::mem::transmute(e);

	if e.numTouches > 2 {
		use std::mem::uninitialized;

		let mut fs_state: EmscriptenFullscreenChangeEvent = uninitialized();
		emscripten_get_fullscreen_status(&mut fs_state);

		if fs_state.isFullscreen == 0 {
			js! {{ b"Module.requestFullscreen(1, 1)\0" }};
			console::set_section("Fullscreen requested", "");
		}
	}

	if e.touches[0].identifier != 0 { return 0 }

	let pos = Vec2i::new(e.touches[0].clientX as _, e.touches[0].clientY as _);
	event_queue.push(Event::Up(pos));
	console::set_section("Input(touch)", "up");
	
	1
}
