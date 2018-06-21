// "adi_gpu_vulkan" - Aldaron's Device Interface / GPU / Vulkan
//
// Copyright Jeron A. Lau 2018.
// Distributed under the Boost Software License, Version 1.0.
// (See accompanying file LICENSE_1_0.txt or copy at
// https://www.boost.org/LICENSE_1_0.txt)
//
//! Vulkan implementation for adi_gpu.

#version 450
#extension GL_ARB_separate_shader_objects : enable

layout (binding = 0) uniform UniformBuffer {
	mat4 models_tfm; // The Models' Transform Matrix
	vec4 color;
	int has_camera;
} uniforms;
layout (binding = 1) uniform Camera {
	mat4 matrix; // The Camera's Transform & Projection Matrix
	vec4 fog; // The fog color.
	vec2 range; // The range of fog (fog to far clip)
} camera;
layout (binding = 2) uniform Fog {
	vec4 fog; // The fog color.
	vec2 range; // The range of fog (fog to far clip)
} fog;

layout (location = 0) in vec4 pos;

layout (location = 0) out vec4 inColor;
layout (location = 1) out float z;

void main() {
	inColor = uniforms.color;

	vec4 place = uniforms.models_tfm * vec4(pos.xyz, 1.0);

	if(uniforms.has_camera >= 1) {
		gl_Position = camera.matrix * place;
	} else {
		gl_Position = place;
	}

	z = length(gl_Position.xyz);
}
