use ash::vk;
use egui::{epaint::Vertex, Context};
use gpu_allocator::MemoryLocation;
use inline_spirv::inline_spirv;
use winit::event_loop::{self, EventLoop};

use crate::buffer::Buffer;

pub struct EguiPass {
    context: egui::Context,
    state: egui_winit::State,
}

const VERTEX_SHADER: &[u32] = inline_spirv!(
    r#"
    #version 450 core

    layout (location = 0) in vec2 a_pos;
    layout (location = 1) in vec2 a_tex_coord;
    layout (location = 2) in uint a_color;

    layout (location = 0) out vec2 tex_coord;
    layout (location = 1) out vec4 color;

    layout(set = 0, binding = 0) uniform Locals {
        vec2 screen_size;
        vec2 _pad;
    } locals;      


    // 0-1 linear from 0-1 sRGB gamma
    vec3 linear_from_gamma_rgb(vec3 srgb) {
        vec3 cutoff = step(vec3(0.04045), srgb);
        vec3 lower = srgb / vec3(12.92);
        vec3 higher = pow((srgb + vec3(0.055)) / vec3(1.055), vec3(2.4));
        return mix(higher, lower, cutoff);
    }

    // 0-1 sRGB gamma from 0-1 linear
    vec3 gamma_from_linear_rgb(vec3 rgb) {
        vec3 cutoff = step(vec3(0.0031308), rgb);
        vec3 lower = rgb * vec3(12.92);
        vec3 higher = vec3(1.055) * pow(rgb, vec3(1.0 / 2.4)) - vec3(0.055);
        return mix(higher, lower, cutoff);
    }

    // 0-1 sRGBA gamma from 0-1 linear
    vec4 gamma_from_linear_rgba(vec4 linear_rgba) {
        return vec4(gamma_from_linear_rgb(linear_rgba.rgb), linear_rgba.a);
    }

    // [u8; 4] SRGB as u32 -> [r, g, b, a] in 0.-1
    vec4 unpack_color(uint color) {
        return vec4(
            float(color & 255u),
            float((color >> 8u) & 255u),
            float((color >> 16u) & 255u),
            float((color >> 24u) & 255u)
        ) / 255.0;
    }

    vec4 position_from_screen(vec2 screen_pos) {
        return vec4(
            2.0 * screen_pos.x / locals.screen_size.x - 1.0,
            1.0 - 2.0 * screen_pos.y / locals.screen_size.y,
            0.0,
            1.0
        );
    }

    void main() { 
        tex_coord = a_tex_coord;
        color = unpack_color(a_color);
        gl_Position = position_from_screen(a_pos);
    }
    "#,
    vert
);

const FRAGMENT_SHADER: &[u32] = inline_spirv!(
    r#"
    #version 450 core

    layout (location = 0) in vec2 tex_coord;
    layout (location = 1) in vec4 color;
    
    layout(location = 0) out vec4 frag_color;

    layout(set = 0, binding = 0) uniform Locals {
        vec2 screen_size;
        vec2 _pad;
    } locals;      

    layout(set = 1, binding = 0) uniform sampler2D r_tex_color;

    // 0-1 linear from 0-1 sRGB gamma
    vec3 linear_from_gamma_rgb(vec3 srgb) {
        vec3 cutoff = step(vec3(0.04045), srgb);
        vec3 lower = srgb / vec3(12.92);
        vec3 higher = pow((srgb + vec3(0.055)) / vec3(1.055), vec3(2.4));
        return mix(higher, lower, cutoff);
    }

    // 0-1 sRGB gamma from 0-1 linear
    vec3 gamma_from_linear_rgb(vec3 rgb) {
        vec3 cutoff = step(vec3(0.0031308), rgb);
        vec3 lower = rgb * vec3(12.92);
        vec3 higher = vec3(1.055) * pow(rgb, vec3(1.0 / 2.4)) - vec3(0.055);
        return mix(higher, lower, cutoff);
    }

    // 0-1 sRGBA gamma from 0-1 linear
    vec4 gamma_from_linear_rgba(vec4 linear_rgba) {
        return vec4(gamma_from_linear_rgb(linear_rgba.rgb), linear_rgba.a);
    }

    // [u8; 4] SRGB as u32 -> [r, g, b, a] in 0.-1
    vec4 unpack_color(uint color) {
        return vec4(
            float(color & 255u),
            float((color >> 8u) & 255u),
            float((color >> 16u) & 255u),
            float((color >> 24u) & 255u)
        ) / 255.0;
    }

    vec4 position_from_screen(vec2 screen_pos) {
        return vec4(
            2.0 * screen_pos.x / locals.screen_size.x - 1.0,
            1.0 - 2.0 * screen_pos.y / locals.screen_size.y,
            0.0,
            1.0
        );
    }

    void main() { 
        vec4 tex_linear = texture(r_tex_color, tex_coord);
        vec4 tex_gamma = gamma_from_linear_rgba(tex_linear);
        vec4 out_color_gamma = color * tex_gamma;
        frag_color = vec4(linear_from_gamma_rgb(out_color_gamma.rgb), out_color_gamma.a);
    }
    "#,
    frag
);

impl EguiPass {
    pub fn new(base: &mut crate::ctx::ExampleBase) -> Self {
        let context = Context::default();
        let egui_winit = egui_winit::State::new(&base.event_loop);

        let mut vertex_buffer = {
            let buf = Buffer::new(
                &base.device,
                &mut base.allocator,
                &vk::BufferCreateInfo {
                    usage: vk::BufferUsageFlags::VERTEX_BUFFER,
                    sharing_mode: vk::SharingMode::EXCLUSIVE,
                    ..Default::default()
                },
                MemoryLocation::CpuToGpu,
            );

            buf
        };

        Self {
            context,
            state: egui_winit,
        }
    }

    pub fn start_painting(&mut self, window: &winit::window::Window) -> &Context {
        self.context.begin_frame(self.state.take_egui_input(window));
        &self.context
    }

    pub fn end_painting(&mut self, window: &winit::window::Window) {
        let output = self.context.end_frame();
        self.state
            .handle_platform_output(window, &self.context, output.platform_output);
    }
}
