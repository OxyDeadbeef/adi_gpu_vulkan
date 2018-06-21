// "adi_gpu_vulkan" - Aldaron's Device Interface / GPU / Vulkan
//
// Copyright Jeron A. Lau 2018.
// Distributed under the Boost Software License, Version 1.0.
// (See accompanying file LICENSE_1_0.txt or copy at
// https://www.boost.org/LICENSE_1_0.txt)
//
//! Vulkan implementation for adi_gpu.

use asi_vulkan;

pub struct Vulkan(pub asi_vulkan::Vk);

impl Vulkan {
	pub fn new() -> Result<Self, &'static str> {
		let connection = asi_vulkan::Vk::new();

		if let Some(c) = connection {
			Ok(Vulkan(c))
		} else {
			Err("Couldn't find Vulkan")
		}
	}
}
