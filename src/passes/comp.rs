use std::ffi::CStr;

use ash::vk::{
    self, PipelineBindPoint, ShaderCodeTypeEXT, ShaderCreateInfoEXT, ShaderEXT, ShaderStageFlags,
};
use hassle_rs::compile_hlsl;

use crate::{buffer::Image, ctx::record_submit_commandbuffer, graph::RenderNode};

pub struct CompPass {
    pipeline_layout: vk::PipelineLayout,
    descriptor_sets: Vec<vk::DescriptorSet>,
    shaders: Vec<ShaderEXT>,
}

impl CompPass {
    pub unsafe fn new(base: &mut crate::ctx::ExampleBase, texture: &mut Image) -> Self {
        let comp_spirv = compile_hlsl(
            "comp.hlsl",
            &std::fs::read_to_string(r#"C:\Users\dylan\dev\someday\shader\comp.hlsl"#).unwrap(),
            "main",
            "cs_6_5",
            &["-spirv"],
            &[],
        )
        .unwrap();

        let shaders = base
            .shader_object
            .create_shaders(
                &[ShaderCreateInfoEXT::default()
                    .name(CStr::from_bytes_with_nul_unchecked(b"main\0"))
                    .code(&comp_spirv)
                    .code_type(ShaderCodeTypeEXT::SPIRV)
                    .stage(ShaderStageFlags::COMPUTE)],
                None,
            )
            .unwrap();

        let descriptor_sizes = [vk::DescriptorPoolSize {
            ty: vk::DescriptorType::STORAGE_IMAGE,
            descriptor_count: 1,
        }];
        let descriptor_pool_info = vk::DescriptorPoolCreateInfo::default()
            .pool_sizes(&descriptor_sizes)
            .max_sets(1);

        let descriptor_pool = base
            .device
            .create_descriptor_pool(&descriptor_pool_info, None)
            .unwrap();

        let desc_layout_bindings = [vk::DescriptorSetLayoutBinding {
            descriptor_type: vk::DescriptorType::STORAGE_IMAGE,
            descriptor_count: 1,
            stage_flags: vk::ShaderStageFlags::COMPUTE,
            ..Default::default()
        }];

        let descriptor_info =
            vk::DescriptorSetLayoutCreateInfo::default().bindings(&desc_layout_bindings);

        let desc_set_layouts = [base
            .device
            .create_descriptor_set_layout(&descriptor_info, None)
            .unwrap()];

        let desc_alloc_info = vk::DescriptorSetAllocateInfo::default()
            .descriptor_pool(descriptor_pool)
            .set_layouts(&desc_set_layouts);
        let descriptor_sets = base
            .device
            .allocate_descriptor_sets(&desc_alloc_info)
            .unwrap();

        let view = texture.create_view(&base.device);

        let write_desc_sets = [vk::WriteDescriptorSet {
            dst_set: descriptor_sets[0],
            dst_binding: 0,
            descriptor_count: 1,
            descriptor_type: vk::DescriptorType::STORAGE_IMAGE,
            p_image_info: &vk::DescriptorImageInfo {
                image_layout: vk::ImageLayout::GENERAL,
                image_view: view,
                ..Default::default()
            },
            ..Default::default()
        }];

        base.device.update_descriptor_sets(&write_desc_sets, &[]);
        let layout_create_info =
            vk::PipelineLayoutCreateInfo::default().set_layouts(&desc_set_layouts);

        let pipeline_layout = base
            .device
            .create_pipeline_layout(&layout_create_info, None)
            .unwrap();

        Self {
            pipeline_layout,
            descriptor_sets,
            shaders,
        }
    }
}

impl Drop for CompPass {
    fn drop(&mut self) {
        unsafe {
            // ..
        }
    }
}

impl RenderNode for CompPass {
    unsafe fn run(&self, base: &crate::ctx::ExampleBase) {
        record_submit_commandbuffer(
            &base.device,
            base.draw_command_buffer,
            base.draw_commands_reuse_fence,
            base.present_queue,
            &[],
            &[],
            &[],
            |device, draw_command_buffer| {
                base.shader_object.cmd_bind_shaders(
                    draw_command_buffer,
                    &[ShaderStageFlags::COMPUTE],
                    &self.shaders,
                );

                device.cmd_bind_descriptor_sets(
                    draw_command_buffer,
                    PipelineBindPoint::COMPUTE,
                    self.pipeline_layout,
                    0,
                    &[self.descriptor_sets[0]],
                    &[],
                );

                device.cmd_dispatch(
                    draw_command_buffer,
                    base.window.inner_size().width / 8,
                    base.window.inner_size().width / 8,
                    1,
                );
            },
        );
    }
}
