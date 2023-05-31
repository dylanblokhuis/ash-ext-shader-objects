use std::ffi::CStr;

use ash::vk::{
    self, DescriptorPoolSize, PipelineBindPoint, ShaderCodeTypeEXT, ShaderCreateInfoEXT, ShaderEXT,
    ShaderStageFlags,
};

use crate::{buffer::Image, ctx::record_submit_commandbuffer, graph::RenderNode};

pub struct CompPass {
    pipeline_layout: vk::PipelineLayout,
    descriptor_sets: Vec<vk::DescriptorSet>,
    shaders: Vec<ShaderEXT>,
}

impl CompPass {
    pub unsafe fn new(base: &mut crate::ctx::ExampleBase, texture: &mut Image) -> Self {
        let compiler = shaderc::Compiler::new().unwrap();
        let mut options = shaderc::CompileOptions::new().unwrap();
        options.set_target_env(
            shaderc::TargetEnv::Vulkan,
            shaderc::EnvVersion::Vulkan1_2 as u32,
        );
        options.add_macro_definition("EP", Some("main"));
        let binding = compiler
            .compile_into_spirv(
                &std::fs::read_to_string(r#"C:\Users\dylan\dev\someday\shader\comp.glsl"#).unwrap(),
                shaderc::ShaderKind::Compute,
                "comp.glsl",
                "main",
                Some(&options),
            )
            .unwrap();
        let comp_spirv = binding.as_binary_u8();

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

        let refl_info = rspirv_reflect::Reflection::new_from_spirv(&comp_spirv).unwrap();
        let sets = refl_info.get_descriptor_sets().unwrap();

        let sets_amount = sets.len() as u32;
        let mut descriptor_sizes: Vec<DescriptorPoolSize> = vec![];
        for (set_index, descriptors) in sets {
            for (descriptor_index, descriptor) in descriptors {
                if let Some(dps) = descriptor_sizes
                    .iter_mut()
                    .find(|x| x.ty.as_raw() == descriptor.ty.0 as i32)
                {
                    dps.descriptor_count += 1;
                } else {
                    descriptor_sizes.push(DescriptorPoolSize {
                        ty: ash::vk::DescriptorType::from_raw(descriptor.ty.0 as i32),
                        descriptor_count: 1,
                    });
                }
            }
        }

        let descriptor_pool_info = vk::DescriptorPoolCreateInfo::default()
            .pool_sizes(&descriptor_sizes)
            .max_sets(sets_amount);

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
