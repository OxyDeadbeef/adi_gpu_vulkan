// "adi_gpu_vulkan" - Aldaron's Device Interface / GPU / Vulkan
//
// Copyright Jeron A. Lau 2018.
// Distributed under the Boost Software License, Version 1.0.
// (See accompanying file LICENSE_1_0.txt or copy at
// https://www.boost.org/LICENSE_1_0.txt)
//
//! Vulkan implementation for adi_gpu.

use std::{ mem };

use adi_gpu_base::*;

mod ffi;

use asi_vulkan;
use asi_vulkan::types::*;
use asi_vulkan::Image;
use asi_vulkan::Style;
use asi_vulkan::Buffer;

// TODO
use asi_vulkan::TransformUniform;
use asi_vulkan::FogUniform;
use asi_vulkan::Sprite;
use asi_vulkan::Gpu;

use ShapeHandle;

#[derive(Clone)] #[repr(C)] struct TransformFullUniform {
	mat4: [f32; 16],
	hcam: u32,
}

#[derive(Clone)] #[repr(C)] struct TransformAndFadeUniform {
	mat4: [f32; 16],
	fade: f32,
	hcam: u32,
}

#[derive(Clone)] #[repr(C)] struct TransformAndColorUniform {
	mat4: [f32; 16],
	vec4: [f32; 4],
	hcam: u32,
}

pub struct Vw {
	connection: Gpu,
	width:u32, height:u32, // Swapchain Dimensions.
	present_images: [VkImage; 2], // 2 for double-buffering
	frame_buffers: [VkFramebuffer; 2], // 2 for double-buffering
	color_format: VkFormat,
	image_count: u32, // 1 (single-buffering) or 2 (double-buffering)
	submit_fence: asi_vulkan::Fence, // The submit fence
	present_image_views: [VkImageView; 2], // 2 for double-buffering
	ms_image: Image,
	depth_image: Image,
	render_pass: VkRenderPass,
	present_mode: VkPresentModeKHR,
}

/// A texture on the GPU.
pub struct Texture {
	mappable_image: Image,
	image: Option<Image>,
//	view: VkImageView,
	pub(super) w: u32,
	pub(super) h: u32,
	pitch: u32,
	staged: bool,
}

pub struct Shape {
	num_buffers: usize,
	buffers: [VkBuffer; 3],
	instance: Sprite,
	fans: Vec<(u32, u32)>,
	transform: Transform, // Transformation matrix.
}

impl ::adi_gpu_base::Point for Shape {
	fn point(&self) -> Vec3 {
		// Position vector at origin * object transform.
		(self.transform.0 * vec4!(0f32, 0f32, 0f32, 1f32)).xyz()
	}
}

pub struct Model {
	shape: asi_vulkan::Buffer,
	vertex_count: u32,
	fans: Vec<(u32, u32)>,
}

pub struct TexCoords {
	vertex_buffer: Buffer,
	vertex_count: u32,
}

pub struct Gradient {
	vertex_buffer: Buffer,
	vertex_count: u32,
}

impl Shape {
// TODO
/*	pub fn animate(window: &mut Window, index: usize, i: usize,
		texture: *const NativeTexture, style: Style)
	{
		let hastx = window.sprites[index].hastx;

		// Must be same style
		if hastx {
			if (texture as *const _ as usize) == 0 {
				panic!("Can't set Style of a Sprite initialized\
					with Style::Texture to Style::Solid");
			}
		} else {
			if (texture as *const _ as usize) != 0 {
				panic!("Can't set Style of a Sprite initialized\
					with Style::Solid to Style::Texture");
			}
		}

		// Free old Style, and set new uniform buffers.
		unsafe {
			asi_vulkan::destroy_uniforms(&window.vw, &mut
				window.sprites[index].instances[i].instance);
			window.sprites[index].instances[i].instance =
				vw_vulkan_uniforms(&window.vw, style, texture,
					if hastx { 1 } else { 0 });
		}
		// TODO: Optimize when using same value from vw_vulkan_uniforms
		// Set texture
//		unsafe {
//			vw_vulkan_txuniform(&window.vw,
//				&mut window.sprites[index].shape.instances[i].instance, texture,
//				if window.sprites[index].shape.hastx { 1 } else { 0 });
//		}
		Shape::enable(window, index, i, true);
	}

	pub fn vertices(window: &Window, index: usize, v: &[f32]) {
		ffi::copy_memory(window.vw.device,
			window.sprites[index].shape.vertex_buffer_memory, v);
	}*/
}

fn swapchain_resize(connection: &Gpu,
	image_count: &mut u32, size: (u32, u32),
	present_images: &mut [VkImage; 2], color_format: &VkFormat,
	present_mode: &VkPresentModeKHR,
	present_image_views: &mut [VkImageView; 2],
	frame_buffers: &mut [VkFramebuffer; 2])
	-> (asi_vulkan::Fence, Image, Image, VkRenderPass)
{
	unsafe {
		let submit_fence;
		let depth_image;
		let ms_image;
		let render_pass;

		// Link swapchain to vulkan instance.
		asi_vulkan::create_swapchain(
			connection,
			size.0,
			size.1,
			image_count,
			color_format.clone(),
			present_mode.clone(),
			&mut present_images[0]
		);

		// Link Image Views for each framebuffer
		submit_fence = asi_vulkan::create_image_view(
			connection,
			&color_format,
			*image_count,
			present_images,
			present_image_views,
		);

		// Link Depth Buffer to swapchain
		depth_image = asi_vulkan::create_depth_buffer(
			connection,
			&submit_fence,
			size.0,
			size.1,
		);

		// Create multisampling buffer
		ms_image = asi_vulkan::create_ms_buffer(
			connection,
			&color_format,
			size.0,
			size.1,
		);

		// Link Render Pass to swapchain
		render_pass = asi_vulkan::create_render_pass(
			connection,
			&color_format,
		);

		// Link Framebuffers to swapchain
		asi_vulkan::create_framebuffers(
			connection,
			*image_count,
			render_pass,
			present_image_views,
			&ms_image,
			&depth_image,
			size.0,
			size.1,
			frame_buffers,
		);

		(submit_fence, depth_image, ms_image, render_pass)
	}
}

fn swapchain_delete(vw: &mut Vw) {
	unsafe {
		asi_vulkan::destroy_swapchain(
			&vw.connection,
			&vw.frame_buffers,
			&vw.present_image_views,
			vw.render_pass,
			vw.image_count,
		);
	}
}

fn new_texture(vw: &mut Vw, width: u32, height: u32) -> Texture {
//	let mut format_props = unsafe { mem::uninitialized() };
	let staged = !vw.connection.sampled();

	let mappable_image = asi_vulkan::Image::new(
		&mut vw.connection, width, height, VkFormat::R8g8b8a8Unorm,
		VkImageTiling::Linear,
		if staged { VkImageUsage::TransferSrcBit }
		else { VkImageUsage::SampledBit },
		VkImageLayout::Preinitialized,
		0x00000006 /* visible|coherent */,
		VkSampleCount::Sc1
	);

	let layout = unsafe {
		asi_vulkan::subres_layout(&vw.connection, &mappable_image)
	};

	let pitch = layout.row_pitch;

	let image = if staged {
		Some(asi_vulkan::Image::new(
			&mut vw.connection, width, height,
			VkFormat::R8g8b8a8Unorm,
			VkImageTiling::Optimal,
			VkImageUsage::TransferDstAndUsage,
			VkImageLayout::Undefined, 0,
			VkSampleCount::Sc1))
	} else {
		None
	};

	Texture {
		staged, mappable_image,	image, pitch: pitch as u32,
		w: width, h: height,
	}
}

fn set_texture(vw: &mut Vw, texture: &mut Texture, rgba: &[u32]) {
	ffi::copy_memory_pitched(&mut vw.connection,
		texture.image
			.as_ref()
			.unwrap_or(&texture.mappable_image)
			.memory(),
		rgba, texture.w as isize,
		texture.h as isize, texture.pitch as isize);

	if texture.staged {
		// Use optimal tiled image - create from linear tiled image

		// Copy data from linear image to optimal image.
		unsafe {
			asi_vulkan::copy_image(&mut vw.connection,
				&texture.mappable_image,
				texture.image.as_ref().unwrap(),
				texture.w, texture.h
			);
		}
	} else {
		// Use a linear tiled image for the texture, is supported
		texture.image = None;
	}
}

/*pub fn make_styles(vw: &mut Vw, extrashaders: &[Shader], shaders: &mut Vec<Style>)
{
	let mut shadev = Vec::new();
	let default_shaders = [
//		Shader::create(vw, include_bytes!("res/texture-vert.spv"),
//			include_bytes!("res/texture-frag.spv"), 1),
	];
	shadev.extend(default_shaders.iter().cloned());
	shadev.extend(extrashaders.iter().cloned());

	*shaders = vec![Style { pipeline: 0, descsetlayout: 0,
		pipeline_layout: 0 }; shadev.len()];
	unsafe {
		vw_vulkan_pipeline(&mut shaders[0], vw, &shadev[0],
			shadev.len() as u32);
	}
}*/

impl Vw {
	pub fn new(window: Option<(&str, &Graphic)>, rgb: Vec3)
		-> Result<(Vw, Window), String>
	{
		let (mut connection, window)
			= asi_vulkan::Gpu::new(window, rgb)?;

		// END BLOCK 2
		let color_format = unsafe {
			asi_vulkan::get_color_format(&mut connection)
		};
		let mut image_count = unsafe {
			asi_vulkan::get_buffering(&mut connection)
		};
		let present_mode = unsafe {
			asi_vulkan::get_present_mode(&mut connection)
		};

		// Prepare Swapchain
		let mut present_images: [VkImage; 2] = [unsafe { mem::zeroed() }; 2];
		let mut present_image_views = [unsafe { mem::zeroed() }; 2];
		let mut frame_buffers: [VkFramebuffer; 2]
			= [unsafe { mem::uninitialized() }; 2];
		let width = 640; // TODO w
		let height = 360; // TODO h

		let (submit_fence, depth_image, ms_image, render_pass)
			= swapchain_resize(&connection,
				&mut image_count, (width, height),
				&mut present_images, &color_format,
				&present_mode,
				&mut present_image_views, &mut frame_buffers);

		let vw = Vw {
			connection,
			width, height, present_images, frame_buffers,
			color_format, image_count, submit_fence,
			present_image_views, ms_image, depth_image, render_pass,
			present_mode,
		};

		Ok((vw, window))
	}
}

fn draw_shape(connection: &Gpu, shape: &Shape) {
	unsafe {
		// TODO: reduce calls to these functions (for speed).
		asi_vulkan::cmd_bind_vb(connection,
			&shape.buffers[..shape.num_buffers]);
		asi_vulkan::cmd_bind_pipeline(connection,
			shape.instance.pipeline);
		asi_vulkan::cmd_bind_descsets(connection,
			shape.instance.pipeline_layout,
			shape.instance.handles().0/*desc_set*/);

		for i in shape.fans.iter() {
			asi_vulkan::cmd_draw(connection, i.1,
				1, i.0, 0);
		}
	}
}

pub struct Renderer {
	vw: Vw,
	ar: f32,
	opaque_ind: Vec<u32>,
	alpha_ind: Vec<u32>,
	opaque_vec: Vec<Shape>,
	alpha_vec: Vec<Shape>,
	gui_vec: Vec<Shape>,
	models: Vec<Model>,
	texcoords: Vec<TexCoords>,
	gradients: Vec<Gradient>,
	textures: Vec<Texture>,
	style_solid: Style,
	style_nasolid: Style,
	style_texture: Style,
	style_natexture: Style,
	style_gradient: Style,
	style_nagradient: Style,
	style_faded: Style,
	style_tinted: Style,
	style_natinted: Style,
	style_complex: Style,
	style_nacomplex: Style,
	projection: Transform,
	camera_memory: asi_vulkan::Memory<TransformUniform>,
	effect_memory: asi_vulkan::Memory<FogUniform>,
	clear_color: (f32, f32, f32),
	xyz: Vec3,
	rotate_xyz: Vec3,
}

impl Renderer {
	pub fn new(window: Option<(&str, &Graphic)>, rgb: Vec3)
		-> Result<(Renderer, Window), String>
	{
		let (mut vw, window) = Vw::new(window, rgb)?;

		let solid_vert = asi_vulkan::ShaderModule::new(
			&mut vw.connection, include_bytes!(
			"../shaders/res/solid-vert.spv"));
		let solid_frag = asi_vulkan::ShaderModule::new(
			&mut vw.connection, include_bytes!(
			"../shaders/res/solid-frag.spv"));
		let texture_vert = asi_vulkan::ShaderModule::new(
			&mut vw.connection, include_bytes!(
			"../shaders/res/texture-vert.spv"));
		let texture_frag = asi_vulkan::ShaderModule::new(
			&mut vw.connection, include_bytes!(
			"../shaders/res/texture-frag.spv"));
		let gradient_vert = asi_vulkan::ShaderModule::new(
			&mut vw.connection, include_bytes!(
			"../shaders/res/gradient-vert.spv"));
		let gradient_frag = asi_vulkan::ShaderModule::new(
			&mut vw.connection, include_bytes!(
			"../shaders/res/gradient-frag.spv"));
		let faded_vert = asi_vulkan::ShaderModule::new(
			&mut vw.connection, include_bytes!(
			"../shaders/res/faded-vert.spv"));
		let faded_frag = asi_vulkan::ShaderModule::new(
			&mut vw.connection, include_bytes!(
			"../shaders/res/faded-frag.spv"));
		let tinted_vert = asi_vulkan::ShaderModule::new(
			&mut vw.connection, include_bytes!(
			"../shaders/res/gradient-vert.spv"));
		let tinted_frag = asi_vulkan::ShaderModule::new(
			&mut vw.connection, include_bytes!(
			"../shaders/res/gradient-frag.spv"));
		let complex_vert = asi_vulkan::ShaderModule::new(
			&mut vw.connection, include_bytes!(
			"../shaders/res/gradient-vert.spv"));
		let complex_frag = asi_vulkan::ShaderModule::new(
			&mut vw.connection, include_bytes!(
			"../shaders/res/gradient-frag.spv"));
		let style_solid = Style::new(&mut vw.connection, vw.render_pass,
			&solid_vert, &solid_frag, 0, 1, true);
		let style_nasolid = Style::new(&mut vw.connection,
			vw.render_pass,	&solid_vert, &solid_frag, 0, 1, false);
		let style_texture = Style::new(&mut vw.connection,
			vw.render_pass,	&texture_vert, &texture_frag, 1, 2,
			true);
		let style_natexture = Style::new(&mut vw.connection,
			vw.render_pass,	&texture_vert, &texture_frag, 1, 2,
			false);
		let style_gradient = Style::new(&mut vw.connection,
			vw.render_pass,	&gradient_vert, &gradient_frag, 0, 2,
			true);
		let style_nagradient = Style::new(&mut vw.connection,
			vw.render_pass,	&gradient_vert, &gradient_frag, 0, 2,
			false);
		let style_faded = Style::new(&mut vw.connection, vw.render_pass,
			&faded_vert, &faded_frag, 1, 2, true);
		let style_tinted = Style::new(&mut vw.connection,
			vw.render_pass,	&tinted_vert, &tinted_frag, 1, 2, true);
		let style_natinted = Style::new(&mut vw.connection,
			vw.render_pass, &tinted_vert, &tinted_frag, 1, 2,
			false);
		let style_complex = Style::new(&mut vw.connection,
			vw.render_pass, &complex_vert, &complex_frag, 1, 3,
			true);
		let style_nacomplex = Style::new(&mut vw.connection,
			vw.render_pass, &complex_vert, &complex_frag, 1, 3,
			false);

		let ar = vw.width as f32 / vw.height as f32;
		let projection = ::base::projection(ar, 0.5 * PI);
		let (camera_memory, effect_memory) = unsafe {
			asi_vulkan::vw_camera_new(&mut vw.connection,
				(rgb.x, rgb.y, rgb.z, 1.0),
				(::std::f32::MAX, ::std::f32::MAX))
		};

		let mut renderer = Renderer {
			vw, ar, projection,
			camera_memory, effect_memory,
			alpha_ind: Vec::new(),
			opaque_ind: Vec::new(),
			alpha_vec: Vec::new(),
			opaque_vec: Vec::new(),
			gui_vec: Vec::new(),
			gradients: Vec::new(),
			models: Vec::new(),
			texcoords: Vec::new(),
			textures: Vec::new(),
			style_solid, style_nasolid,
			style_texture, style_natexture,
			style_gradient, style_nagradient,
			style_faded,
			style_tinted, style_natinted,
			style_complex, style_nacomplex,
			clear_color: (rgb.x, rgb.y, rgb.z),
			xyz: vec3!(0.0, 0.0, 0.0),
			rotate_xyz: vec3!(0.0, 0.0, 0.0),
		};

		renderer.camera();

		Ok((renderer, window))
	}

	pub fn bg_color(&mut self, rgb: Vec3) {
		self.vw.connection.color(rgb);
	}

	pub fn update(&mut self) {
		let mut presenting_complete_sem = unsafe {
			asi_vulkan::new_semaphore(&self.vw.connection)
		};

		let rendering_complete_sem = unsafe {
			asi_vulkan::new_semaphore(&self.vw.connection)
		};

		let next_image_index = unsafe {
			asi_vulkan::get_next_image(
				&self.vw.connection,
				&mut presenting_complete_sem,
			)
		};

		unsafe {
			asi_vulkan::draw_begin(&self.vw.connection,
				self.vw.render_pass,
				self.vw.present_images[next_image_index as usize],
				self.vw.frame_buffers[next_image_index as usize],
				self.vw.width,
				self.vw.height,
				self.clear_color.0, self.clear_color.1,
				self.clear_color.2
			);
		}

		// sort nearest
		::adi_gpu_base::zsort(&mut self.opaque_ind, &self.opaque_vec,
			true, self.xyz);
		for shape in self.opaque_ind.iter() {
			let shape = &self.opaque_vec[*shape as usize];
			draw_shape(&self.vw.connection, shape);
		}

		// sort farthest
		::adi_gpu_base::zsort(&mut self.alpha_ind, &self.alpha_vec,
			false, self.xyz);
		for shape in self.alpha_ind.iter() {
			let shape = &self.alpha_vec[*shape as usize];
			draw_shape(&self.vw.connection, shape);
		}

		// No need to sort gui elements.
		for shape in self.gui_vec.iter() {
			draw_shape(&self.vw.connection, shape);
		}

		unsafe {
			asi_vulkan::end_render_pass(&self.vw.connection);

			asi_vulkan::pipeline_barrier(&self.vw.connection,
				self.vw.present_images[next_image_index as usize]);

			asi_vulkan::end_cmdbuff(&self.vw.connection);
		}

		unsafe { // Drop fence when it's done use
			let fence = asi_vulkan::Fence::new(&self.vw.connection);

			asi_vulkan::queue_submit(&self.vw.connection,
				&fence,
				VkPipelineStage::BottomOfPipe,
				Some(rendering_complete_sem));
				
			asi_vulkan::wait_fence(&self.vw.connection, &fence);
		}

		unsafe {
			asi_vulkan::queue_present(&self.vw.connection,
				rendering_complete_sem,
				next_image_index);

			asi_vulkan::drop_semaphore(&self.vw.connection,
				rendering_complete_sem);

			asi_vulkan::drop_semaphore(&self.vw.connection,
				presenting_complete_sem);

			asi_vulkan::wait_idle(&self.vw.connection);
		}
	}

	pub fn resize(&mut self, size: (u32, u32)) {
		self.vw.width = size.0;
		self.vw.height = size.1;
		self.ar = size.0 as f32 / size.1 as f32;

		swapchain_delete(&mut self.vw);
		let (submit_fence, depth_image, ms_image, render_pass)
			= swapchain_resize(&self.vw.connection,
				&mut self.vw.image_count, size,
				&mut self.vw.present_images, &self.vw.color_format,
				&self.vw.present_mode,
				&mut self.vw.present_image_views, &mut self.vw.frame_buffers);

		self.vw.submit_fence = submit_fence;
		self.vw.depth_image = depth_image;
		self.vw.ms_image = ms_image;
		self.vw.render_pass = render_pass;

		self.projection = ::base::projection(self.ar, 0.5 * PI);
		self.camera();
	}

	pub fn texture(&mut self, width: u32, height: u32, rgba: &[u32])
		-> usize
	{
		let mut texture = new_texture(&mut self.vw, width, height);

		set_texture(&mut self.vw, &mut texture, rgba);

		let a = self.textures.len();
		self.textures.push(texture);
		a
	}

	pub fn set_texture(&mut self, texture: usize, rgba: &[u32]) {
		set_texture(&mut self.vw, &mut self.textures[texture], rgba);
	}

	/// Push a model (collection of vertices) into graphics memory.
	pub fn model(&mut self, vertices: &[f32], fans: Vec<(u32, u32)>)
		-> usize
	{
		let shape = unsafe {
			asi_vulkan::new_buffer(&self.vw.connection,
				vertices)
		};

		let a = self.models.len();

		self.models.push(Model {
			shape,
			vertex_count: vertices.len() as u32 / 4,
			fans,
		});

		a
	}

	/// Push texture coordinates (collection of vertices) into graphics
	/// memory.
	pub fn texcoords(&mut self, texcoords: &[f32]) -> usize {
		let vertex_buffer = unsafe {
			asi_vulkan::new_buffer(
				&self.vw.connection,
				texcoords,
			)
		};

		let a = self.texcoords.len();

		self.texcoords.push(TexCoords {
			vertex_buffer,
			vertex_count: texcoords.len() as u32 / 4,
		});

		a
	}

	/// Push colors per vertex into graphics memory.
	pub fn colors(&mut self, colors: &[f32]) -> usize {
		let vertex_buffer = unsafe {
			asi_vulkan::new_buffer(
				&self.vw.connection,
				colors,
			)
		};

		let a = self.gradients.len();

		self.gradients.push(Gradient {
			vertex_buffer,
			vertex_count: colors.len() as u32 / 4,
		});

		a
	}

	pub fn textured(&mut self, model: usize, mat4: Transform,
		texture: usize, texcoords: usize, alpha: bool,
		fog: bool, camera: bool) -> ShapeHandle
	{
		if self.models[model].vertex_count
			!= self.texcoords[texcoords].vertex_count
		{
			panic!("TexCoord length doesn't match vertex length");
		}

		// Add an instance
		let instance = unsafe {
			Sprite::new(
				&self.vw.connection,
				if alpha {
					&self.style_texture
				} else {
					&self.style_natexture
				},
				TransformFullUniform {
					mat4: mat4.into(),
					hcam: fog as u32 + camera as u32,
				},
				&self.camera_memory, // TODO: at shader creation, not shape creation
				Some(&self.effect_memory),
				Some(self.textures[texture].image.as_ref()
					.unwrap_or(&self.textures[texture]
						.mappable_image)),
				true, // 1 texure
			)
		};

		let shape = Shape {
			instance,
			num_buffers: 2,
			buffers: [
				self.models[model].shape.buffer(),
				self.texcoords[texcoords].vertex_buffer.buffer(),
				unsafe { mem::uninitialized() }
			],
			fans: self.models[model].fans.clone(),
			transform: mat4,
		};

		if !camera && !fog {
			self.gui_vec.push(shape);
			ShapeHandle::Gui(self.gui_vec.len() as u32 - 1)
		} else if alpha {
			let index = self.alpha_vec.len() as u32;
			self.alpha_vec.push(shape);
			self.alpha_ind.push(index);
			ShapeHandle::Alpha(index)
		} else {
			let index = self.opaque_vec.len() as u32;
			self.opaque_vec.push(shape);
			self.opaque_ind.push(index);
			ShapeHandle::Opaque(index)
		}
	}

	pub fn solid(&mut self, model: usize, mat4: Transform, color: [f32; 4],
		alpha: bool, fog: bool, camera: bool)
		-> ShapeHandle
	{
		// Add an instance
		let instance = unsafe {
			Sprite::new(
				&self.vw.connection,
				if alpha {
					&self.style_solid
				} else {
					&self.style_nasolid
				},
				TransformAndColorUniform {
					vec4: color,
					hcam: fog as u32 + camera as u32,
					mat4: mat4.into(),
				},
				&self.camera_memory,
				Some(&self.effect_memory),
				None,
				false, // no texure
			)
		};

		let shape = Shape {
			instance,
			num_buffers: 1,
			buffers: [
				self.models[model].shape.buffer(),
				unsafe { mem::uninitialized() },
				unsafe { mem::uninitialized() }
			],
			fans: self.models[model].fans.clone(),
			transform: mat4,
		};

		if !camera && !fog {
			self.gui_vec.push(shape);
			ShapeHandle::Gui(self.gui_vec.len() as u32 - 1)
		} else if alpha {
			let index = self.alpha_vec.len() as u32;
			self.alpha_vec.push(shape);
			self.alpha_ind.push(index);
			ShapeHandle::Alpha(index)
		} else {
			let index = self.opaque_vec.len() as u32;
			self.opaque_vec.push(shape);
			self.opaque_ind.push(index);
			ShapeHandle::Opaque(index)
		}
	}

	pub fn gradient(&mut self, model: usize, mat4: Transform, colors: usize,
		alpha: bool, fog: bool, camera: bool)
		-> ShapeHandle
	{
		if self.models[model].vertex_count
			!= self.gradients[colors].vertex_count
		{
			panic!("TexCoord length doesn't match gradient length");
		}

		// Add an instance
		let instance = unsafe {
			Sprite::new(
				&self.vw.connection,
				if alpha {
					&self.style_gradient
				} else {
					&self.style_nagradient
				},
				TransformFullUniform {
					mat4: mat4.into(),
					hcam: fog as u32 + camera as u32,
				},
				&self.camera_memory,
				Some(&self.effect_memory),
				None,
				false, // no texure
			)
		};

		let shape = Shape {
			instance,
			num_buffers: 2,
			buffers: [
				self.models[model].shape.buffer(),
				self.gradients[colors].vertex_buffer.buffer(),
				unsafe { mem::uninitialized() }
			],
			fans: self.models[model].fans.clone(),
			transform: mat4,
		};

		if !camera && !fog {
			self.gui_vec.push(shape);
			ShapeHandle::Gui(self.gui_vec.len() as u32 - 1)
		} else if alpha {
			let index = self.alpha_vec.len() as u32;
			self.alpha_vec.push(shape);
			self.alpha_ind.push(index);
			ShapeHandle::Alpha(index)
		} else {
			let index = self.opaque_vec.len() as u32;
			self.opaque_vec.push(shape);
			self.opaque_ind.push(index);
			ShapeHandle::Opaque(index)
		}
	}

	pub fn faded(&mut self, model: usize, mat4: Transform, texture: usize,
		texcoords: usize, fade_factor: f32, fog: bool,
		camera: bool) -> ShapeHandle
	{
		if self.models[model].vertex_count
			!= self.texcoords[texcoords].vertex_count
		{
			panic!("TexCoord length doesn't match vertex length");
		}

		// Add an instance
		let instance = unsafe {
			Sprite::new(
				&self.vw.connection,
				&self.style_faded,
				TransformAndFadeUniform {
					mat4: mat4.into(),
					hcam: fog as u32 + camera as u32,
					fade: fade_factor,
				},
				&self.camera_memory,
				Some(&self.effect_memory),
				Some(self.textures[texture].image.as_ref()
					.unwrap_or(&self.textures[texture]
						.mappable_image)),
				true, // 1 texure
			)
		};

		let shape = Shape {
			instance,
			num_buffers: 2,
			buffers: [
				self.models[model].shape.buffer(),
				self.texcoords[texcoords].vertex_buffer.buffer(),
				unsafe { mem::uninitialized() }
			],
			fans: self.models[model].fans.clone(),
			transform: mat4,
		};

		if !camera && !fog {
			self.gui_vec.push(shape);
			ShapeHandle::Gui(self.gui_vec.len() as u32 - 1)
		} else {
			let index = self.alpha_vec.len() as u32;
			self.alpha_vec.push(shape);
			self.alpha_ind.push(index);
			ShapeHandle::Alpha(index)
		}
	}

	pub fn tinted(&mut self, model: usize, mat4: Transform,
		texture: usize, texcoords: usize, color: [f32; 4],
		alpha: bool, fog: bool, camera: bool)
		-> ShapeHandle
	{
		if self.models[model].vertex_count
			!= self.texcoords[texcoords].vertex_count
		{
			panic!("TexCoord length doesn't match vertex length");
		}

		// Add an instance
		let instance = unsafe {
			Sprite::new(
				&self.vw.connection,
				if alpha {
					&self.style_tinted
				} else {
					&self.style_natinted
				},
				TransformAndColorUniform {
					mat4: mat4.into(),
					hcam: fog as u32 + camera as u32,
					vec4: color,
				},
				&self.camera_memory,
				Some(&self.effect_memory),
				Some(self.textures[texture].image.as_ref()
					.unwrap_or(&self.textures[texture]
						.mappable_image)),
				true, // 1 texure
			)
		};

		let shape = Shape {
			instance,
			num_buffers: 2,
			buffers: [
				self.models[model].shape.buffer(),
				self.texcoords[texcoords].vertex_buffer.buffer(),
				unsafe { mem::uninitialized() }
			],
			fans: self.models[model].fans.clone(),
			transform: mat4,
		};

		if !camera && !fog {
			self.gui_vec.push(shape);
			ShapeHandle::Gui(self.gui_vec.len() as u32 - 1)
		} else if alpha {
			let index = self.alpha_vec.len() as u32;
			self.alpha_vec.push(shape);
			self.alpha_ind.push(index);
			ShapeHandle::Alpha(index)
		} else {
			let index = self.opaque_vec.len() as u32;
			self.opaque_vec.push(shape);
			self.opaque_ind.push(index);
			ShapeHandle::Opaque(index)
		}
	}

	pub fn complex(&mut self, model: usize, mat4: Transform,
		texture: usize, texcoords: usize, colors: usize, alpha: bool,
		fog: bool, camera: bool) -> ShapeHandle
	{
		if self.models[model].vertex_count
			!= self.texcoords[texcoords].vertex_count ||
			self.models[model].vertex_count
			!= self.gradients[colors].vertex_count
		{
			panic!("TexCoord length doesn't match vertex length");
		}

		// Add an instance
		let instance = unsafe {
			Sprite::new(
				&self.vw.connection,
				if alpha {
					&self.style_complex
				} else {
					&self.style_nacomplex
				},
				TransformFullUniform {
					mat4: mat4.into(),
					hcam: fog as u32 + camera as u32,
				},
				&self.camera_memory,
				Some(&self.effect_memory),
				Some(self.textures[texture].image.as_ref()
					.unwrap_or(&self.textures[texture]
						.mappable_image)),
				true, // 1 texure
			)
		};

		let shape = Shape {
			instance,
			num_buffers: 3,
			buffers: [
				self.models[model].shape.buffer(),
				self.texcoords[texcoords].vertex_buffer.buffer(),
				self.gradients[colors].vertex_buffer.buffer(),
			],
			fans: self.models[model].fans.clone(),
			transform: mat4,
		};

		if !camera && !fog {
			self.gui_vec.push(shape);
			ShapeHandle::Gui(self.gui_vec.len() as u32 - 1)
		} else if alpha {
			let index = self.alpha_vec.len() as u32;
			self.alpha_vec.push(shape);
			self.alpha_ind.push(index);
			ShapeHandle::Alpha(index)
		} else {
			let index = self.opaque_vec.len() as u32;
			self.opaque_vec.push(shape);
			self.opaque_ind.push(index);
			ShapeHandle::Opaque(index)
		}
	}

	pub fn transform(&mut self, shape: &ShapeHandle, transform: Transform) {
		let uniform = TransformUniform {
			mat4: transform.into(),
		};

		match shape {
			ShapeHandle::Opaque(x) => {
				let x = *x as usize; // for indexing
				self.opaque_vec[x].transform = transform;
				ffi::copy_memory(&self.vw.connection,
					self.opaque_vec[x].instance.uniform_memory.memory(),
					&uniform);
			},
			ShapeHandle::Alpha(x) => {
				let x = *x as usize; // for indexing
				self.alpha_vec[x].transform = transform;
				ffi::copy_memory(&self.vw.connection,
					self.alpha_vec[x].instance.uniform_memory.memory(),
					&uniform);
			},
			ShapeHandle::Gui(x) => {
				let x = *x as usize; // for indexing
				self.gui_vec[x].transform = transform;
				ffi::copy_memory(&self.vw.connection,
					self.gui_vec[x].instance.uniform_memory.memory(),
					&uniform);
			},
		}
	}

	pub fn set_camera(&mut self, xyz: Vec3, rxyz: Vec3) {
		self.xyz = xyz;
		self.rotate_xyz = rxyz;
	}

	pub fn camera(&mut self) {
		self.camera_memory.data.mat4 = Transform::IDENTITY
			.t(vec3!()-self.xyz) // Move camera - TODO: negation operator?
			.r(vec3!()-self.rotate_xyz) // Rotate camera - TODO: negation operator?
			.m(self.projection.0) // Apply projection to camera
			.into(); // convert to f32 array

		self.camera_memory.update(&self.vw.connection);
	}

	pub fn fog(&mut self, fog: (f32, f32)) -> () {
		self.effect_memory.data.fogc = [self.clear_color.0,
			self.clear_color.1, self.clear_color.2, 1.0];
		self.effect_memory.data.fogr = [fog.0, fog.1];

		self.effect_memory.update(&self.vw.connection);
	}
}

impl Drop for Renderer {
	fn drop(&mut self) -> () {
		swapchain_delete(&mut self.vw);
	}
}
