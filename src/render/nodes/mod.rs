use std::mem::size_of_val;

use ash::vk::{
    self, CompareOp, CullModeFlags, FrontFace, PipelineBindPoint, ShaderEXT, ShaderStageFlags,
};
use gpu_allocator::MemoryLocation;

use crate::{buffer::Buffer, ctx::record_submit_commandbuffer};

use super::{
    shaders::Shader, GpuMesh, ProcessedRenderAssets, RenderAllocator, RenderInstance,
    SequentialNode,
};

pub struct PresentNode {
    shaders: Vec<ShaderEXT>,
    descriptor_sets: Vec<vk::DescriptorSet>,
    pipeline_layout: vk::PipelineLayout,
}

#[derive(Clone, Debug, Copy, bytemuck::Pod, bytemuck::Zeroable, Default)]
#[repr(C, align(16))]
struct Uniform {
    buf_pointer: u64,
    _pad: [f32; 2],
}

#[derive(Clone, Debug, Copy, bytemuck::Pod, bytemuck::Zeroable)]
#[repr(C, align(16))]
struct Misc {
    color: [f32; 4],
}

impl PresentNode {
    pub fn new(render_instance: &RenderInstance, render_allocator: &mut RenderAllocator) -> Self {
        let renderer = &render_instance.0;
        let allocator = &mut render_allocator.0;
        let vert = Shader::from_file(
            r#"C:\Users\dylan\dev\someday\shader\main.vert"#,
            super::shaders::ShaderKind::Vertex,
            "main",
        );
        let frag = Shader::from_file(
            r#"C:\Users\dylan\dev\someday\shader\main.frag"#,
            super::shaders::ShaderKind::Fragment,
            "main",
        );

        let (descriptor_set_layouts, set_layout_info) =
            frag.create_descriptor_set_layouts(render_instance);

        let descriptor_sets =
            frag.create_descriptor_sets(render_instance, &descriptor_set_layouts, &set_layout_info);

        println!("{:?}", set_layout_info);

        let shaders = unsafe {
            renderer
                .shader_object
                .create_shaders(
                    &[
                        vert.ext_shader_create_info(),
                        frag.ext_shader_create_info()
                            .set_layouts(&descriptor_set_layouts),
                    ],
                    None,
                )
                .unwrap()
        };

        let misc_buf = {
            let buf = Buffer::new(
                &renderer.device,
                allocator,
                &vk::BufferCreateInfo::default()
                    .size(std::mem::size_of::<Misc>() as u64)
                    .usage(vk::BufferUsageFlags::RESOURCE_DESCRIPTOR_BUFFER_EXT)
                    .sharing_mode(vk::SharingMode::EXCLUSIVE),
                MemoryLocation::CpuToGpu,
            );

            let colors = &[Misc {
                color: [0.0, 1.0, 0.0, 1.0],
            }];

            println!("misc type size {:?}", std::mem::size_of::<Misc>() as u64);
            println!("misc buf size {:?}", size_of_val(colors));

            buf.copy_from_slice(colors, 0);

            buf
        };

        let uniform_buf = {
            let buf = Buffer::new(
                &renderer.device,
                &mut render_allocator.0,
                &vk::BufferCreateInfo::default()
                    .size(std::mem::size_of::<Uniform>() as u64)
                    .usage(
                        vk::BufferUsageFlags::SHADER_DEVICE_ADDRESS
                            | vk::BufferUsageFlags::UNIFORM_BUFFER,
                    )
                    .sharing_mode(vk::SharingMode::EXCLUSIVE),
                MemoryLocation::CpuToGpu,
            );

            let uniform = Uniform {
                buf_pointer: misc_buf.device_addr,
                ..Default::default()
            };

            println!(
                "uniform type size {:?}",
                std::mem::size_of::<Uniform>() as u64
            );
            println!("uniform buf size {:?}", size_of_val(&[uniform]));

            buf.copy_from_slice(&[uniform], 0);

            buf
        };

        let uniform_buffer_descriptor = &[vk::DescriptorBufferInfo::default()
            .buffer(uniform_buf.buffer)
            .range(uniform_buf.size)
            .offset(0)];

        let write_desc_sets = [vk::WriteDescriptorSet::default()
            .dst_set(descriptor_sets[0])
            .dst_binding(0)
            .descriptor_type(vk::DescriptorType::UNIFORM_BUFFER)
            .buffer_info(uniform_buffer_descriptor)];

        unsafe {
            renderer
                .device
                .update_descriptor_sets(&write_desc_sets, &[]);
        };

        let pipeline_layout = unsafe {
            renderer
                .device
                .create_pipeline_layout(
                    &vk::PipelineLayoutCreateInfo::default().set_layouts(&descriptor_set_layouts),
                    None,
                )
                .unwrap()
        };

        Self {
            shaders,
            descriptor_sets,
            pipeline_layout,
        }
    }
}

impl SequentialNode for PresentNode {
    fn run(
        &self,
        render_instance: &super::RenderInstance,
        world: &bevy::prelude::World,
    ) -> anyhow::Result<()> {
        let assets = world.resource::<ProcessedRenderAssets>();

        let renderer = render_instance.0.as_ref();
        let present_index = unsafe {
            renderer
                .swapchain_loader
                .acquire_next_image(
                    renderer.swapchain,
                    std::u64::MAX,
                    renderer.present_complete_semaphore,
                    vk::Fence::null(),
                )
                .unwrap()
                .0
        };

        record_submit_commandbuffer(
            &renderer.device,
            renderer.draw_command_buffer,
            renderer.draw_commands_reuse_fence,
            renderer.present_queue,
            &[vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT],
            &[renderer.present_complete_semaphore],
            &[renderer.rendering_complete_semaphore],
            |device, draw_command_buffer| unsafe {
                {
                    let image_memory_barrier = vk::ImageMemoryBarrier2::default()
                        .src_stage_mask(vk::PipelineStageFlags2::COLOR_ATTACHMENT_OUTPUT)
                        .src_access_mask(vk::AccessFlags2::COLOR_ATTACHMENT_READ)
                        .old_layout(vk::ImageLayout::UNDEFINED)
                        .dst_stage_mask(vk::PipelineStageFlags2::COLOR_ATTACHMENT_OUTPUT)
                        .dst_access_mask(vk::AccessFlags2::COLOR_ATTACHMENT_WRITE)
                        .new_layout(vk::ImageLayout::ATTACHMENT_OPTIMAL)
                        .image(renderer.present_images[present_index as usize])
                        .subresource_range(vk::ImageSubresourceRange {
                            aspect_mask: vk::ImageAspectFlags::COLOR,
                            layer_count: 1,
                            level_count: 1,
                            ..Default::default()
                        });

                    let dependency_info = vk::DependencyInfo::default()
                        .image_memory_barriers(std::slice::from_ref(&image_memory_barrier));

                    device.cmd_pipeline_barrier2(draw_command_buffer, &dependency_info);
                }

                let color_attach = &[vk::RenderingAttachmentInfo::default()
                    .image_view(renderer.present_image_views[present_index as usize])
                    .image_layout(vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL)
                    .load_op(vk::AttachmentLoadOp::CLEAR)
                    .store_op(vk::AttachmentStoreOp::STORE)
                    .clear_value(vk::ClearValue {
                        color: vk::ClearColorValue {
                            float32: [0.1, 0.1, 0.1, 1.0],
                        },
                    })];

                let depth_attach = &vk::RenderingAttachmentInfo::default()
                    .image_view(renderer.depth_image_view)
                    .image_layout(vk::ImageLayout::DEPTH_ATTACHMENT_OPTIMAL)
                    .load_op(vk::AttachmentLoadOp::CLEAR)
                    .store_op(vk::AttachmentStoreOp::STORE)
                    .clear_value(vk::ClearValue {
                        depth_stencil: vk::ClearDepthStencilValue {
                            depth: 1.0,
                            stencil: 0,
                        },
                    });

                let render_pass_begin_info = vk::RenderingInfo::default()
                    .render_area(renderer.surface_resolution.into())
                    .layer_count(1)
                    .color_attachments(color_attach)
                    .depth_attachment(depth_attach);

                device.cmd_begin_rendering(draw_command_buffer, &render_pass_begin_info);
                device.cmd_bind_descriptor_sets(
                    draw_command_buffer,
                    PipelineBindPoint::GRAPHICS,
                    self.pipeline_layout,
                    0,
                    &self.descriptor_sets,
                    &[],
                );

                renderer.shader_object.cmd_set_viewport_with_count(
                    draw_command_buffer,
                    &[vk::Viewport {
                        x: 0.0,
                        y: 0.0,
                        width: renderer.surface_resolution.width as f32,
                        height: renderer.surface_resolution.height as f32,
                        min_depth: 0.0,
                        max_depth: 1.0,
                    }],
                );
                renderer.shader_object.cmd_set_scissor_with_count(
                    draw_command_buffer,
                    &[renderer.surface_resolution.into()],
                );
                renderer
                    .shader_object
                    .cmd_set_cull_mode(draw_command_buffer, CullModeFlags::BACK);
                renderer
                    .shader_object
                    .cmd_set_front_face(draw_command_buffer, FrontFace::COUNTER_CLOCKWISE);
                renderer
                    .shader_object
                    .cmd_set_depth_test_enable(draw_command_buffer, true);
                renderer
                    .shader_object
                    .cmd_set_depth_write_enable(draw_command_buffer, true);
                renderer
                    .shader_object
                    .cmd_set_depth_compare_op(draw_command_buffer, CompareOp::LESS_OR_EQUAL);

                renderer.shader_object.cmd_set_vertex_input(
                    draw_command_buffer,
                    &[GpuMesh::vertex_binding_descriptors()],
                    &GpuMesh::vertex_input_descriptors(),
                );

                renderer.shader_object.cmd_bind_shaders(
                    draw_command_buffer,
                    &[ShaderStageFlags::VERTEX, ShaderStageFlags::FRAGMENT],
                    &self.shaders,
                );

                for (_, mesh) in assets.meshes.iter() {
                    renderer
                        .shader_object
                        .cmd_set_primitive_topology(draw_command_buffer, mesh.topology);
                    device.cmd_bind_vertex_buffers(
                        draw_command_buffer,
                        0,
                        &[mesh.vertex_buffer.buffer],
                        &[0],
                    );
                    device.cmd_bind_index_buffer(
                        draw_command_buffer,
                        mesh.index_buffer.buffer,
                        0,
                        vk::IndexType::UINT32,
                    );
                    device.cmd_draw_indexed(draw_command_buffer, mesh.index_count, 1, 0, 0, 1);
                }

                device.cmd_end_rendering(draw_command_buffer);
                {
                    let image_memory_barrier = vk::ImageMemoryBarrier2::default()
                        .src_stage_mask(vk::PipelineStageFlags2::COLOR_ATTACHMENT_OUTPUT)
                        .src_access_mask(vk::AccessFlags2::COLOR_ATTACHMENT_WRITE)
                        .old_layout(vk::ImageLayout::ATTACHMENT_OPTIMAL)
                        .dst_stage_mask(vk::PipelineStageFlags2::COLOR_ATTACHMENT_OUTPUT)
                        .dst_access_mask(vk::AccessFlags2::COLOR_ATTACHMENT_READ)
                        .new_layout(vk::ImageLayout::PRESENT_SRC_KHR)
                        .image(renderer.present_images[present_index as usize])
                        .subresource_range(vk::ImageSubresourceRange {
                            aspect_mask: vk::ImageAspectFlags::COLOR,
                            layer_count: 1,
                            level_count: 1,
                            ..Default::default()
                        });

                    let dependency_info = vk::DependencyInfo::default()
                        .image_memory_barriers(std::slice::from_ref(&image_memory_barrier));

                    device.cmd_pipeline_barrier2(draw_command_buffer, &dependency_info);
                }
            },
        );

        let wait_semaphors = [renderer.rendering_complete_semaphore];
        let swapchains = [renderer.swapchain];
        let image_indices = [present_index];
        let present_info = vk::PresentInfoKHR::default()
            .wait_semaphores(&wait_semaphors)
            .swapchains(&swapchains)
            .image_indices(&image_indices);

        unsafe {
            renderer
                .swapchain_loader
                .queue_present(renderer.present_queue, &present_info)
                .unwrap();
        };
        Ok(())
    }
}
