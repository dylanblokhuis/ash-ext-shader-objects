use std::mem::size_of;

use ash::vk::{
    self, CompareOp, CullModeFlags, DeviceSize, FrontFace, PipelineBindPoint, ShaderEXT,
    ShaderStageFlags,
};
use bevy::prelude::{Mat4, Vec3, Vec4};
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
    uniform: Buffer,
}

#[derive(Clone, Debug, Copy, bytemuck::Pod, bytemuck::Zeroable, Default)]
#[repr(C, align(16))]
struct Uniform {
    camera_pointer: u64,
    _pad: [f32; 2],
}

#[repr(C, align(16))]
#[derive(Debug, Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct PushConstants {
    model: Mat4,
}

impl PresentNode {
    pub fn new(render_instance: &RenderInstance, render_allocator: &mut RenderAllocator) -> Self {
        let renderer = &render_instance.0;
        let vert = Shader::from_file(
            r#"./shader/main.vert"#,
            super::shaders::ShaderKind::Vertex,
            "main",
        );
        let frag = Shader::from_file(
            r#"./shader/main.frag"#,
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
                        vert.ext_shader_create_info()
                            .set_layouts(&descriptor_set_layouts),
                        frag.ext_shader_create_info()
                            .set_layouts(&descriptor_set_layouts),
                    ],
                    None,
                )
                .unwrap()
        };

        let pipeline_layout = unsafe {
            renderer
                .device
                .create_pipeline_layout(
                    &vk::PipelineLayoutCreateInfo::default()
                        .set_layouts(&descriptor_set_layouts)
                        .push_constant_ranges(&[vk::PushConstantRange::default()
                            .stage_flags(ShaderStageFlags::ALL_GRAPHICS)
                            .offset(0)
                            .size(size_of::<PushConstants>() as u32)]),
                    None,
                )
                .unwrap()
        };

        let uniform = Buffer::new(
            render_instance.device(),
            render_allocator.allocator(),
            &vk::BufferCreateInfo::default()
                .size(size_of::<Uniform>() as DeviceSize)
                .usage(vk::BufferUsageFlags::UNIFORM_BUFFER)
                .sharing_mode(vk::SharingMode::EXCLUSIVE),
            MemoryLocation::CpuToGpu,
        );

        Self {
            shaders,
            descriptor_sets,
            pipeline_layout,
            uniform,
        }
    }
}

impl SequentialNode for PresentNode {
    fn update(&mut self, world: &mut bevy::prelude::World) {
        let assets = world.resource::<ProcessedRenderAssets>();
        if assets.buffers.contains_key("camera") && !self.uniform.has_been_written_to {
            println!("Updating uniform");
            let camera = assets.buffers.get("camera").unwrap();
            self.uniform.copy_from_slice(
                &[Uniform {
                    camera_pointer: camera.device_addr,
                    ..Default::default()
                }],
                0,
            );

            let uniform_buffer_descriptor = &[vk::DescriptorBufferInfo::default()
                .buffer(self.uniform.buffer)
                .range(self.uniform.size)
                .offset(0)];

            let write_desc_sets = [vk::WriteDescriptorSet::default()
                .dst_set(self.descriptor_sets[0])
                .dst_binding(0)
                .descriptor_type(vk::DescriptorType::UNIFORM_BUFFER)
                .buffer_info(uniform_buffer_descriptor)];

            unsafe {
                world
                    .resource::<RenderInstance>()
                    .device()
                    .update_descriptor_sets(&write_desc_sets, &[]);
            };
        }
    }

    fn run(&self, world: &bevy::prelude::World) -> anyhow::Result<()> {
        let assets = world.resource::<ProcessedRenderAssets>();
        let render_instance = world.resource::<RenderInstance>();

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

                    renderer.synchronization2.cmd_pipeline_barrier2(draw_command_buffer, &dependency_info);
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

                renderer.dynamic_rendering.cmd_begin_rendering(draw_command_buffer, &render_pass_begin_info);
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
                    device.cmd_push_constants(
                        draw_command_buffer,
                        self.pipeline_layout,
                        vk::ShaderStageFlags::ALL_GRAPHICS,
                        0,
                        bytemuck::bytes_of(&PushConstants {
                            model: mesh.model_matrix,
                        }),
                    );

                    renderer
                        .shader_object
                        .cmd_set_primitive_topology(draw_command_buffer, mesh.topology);
                    device.cmd_bind_vertex_buffers(
                        draw_command_buffer,
                        0,
                        &[mesh.vertex_buffer.buffer],
                        &[0],
                    );
                    if let Some(index_buffer) = &mesh.index_buffer {
                        device.cmd_bind_index_buffer(
                            draw_command_buffer,
                            index_buffer.buffer,
                            0,
                            vk::IndexType::UINT32,
                        );
                        device.cmd_draw_indexed(draw_command_buffer, mesh.index_count, 1, 0, 0, 1);
                    } else {
                        device.cmd_draw(draw_command_buffer, mesh.vertex_count, 1, 0, 1);
                    }
                }

                renderer.dynamic_rendering.cmd_end_rendering(draw_command_buffer);
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
                    
                    renderer.synchronization2.cmd_pipeline_barrier2(draw_command_buffer, &dependency_info);
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
