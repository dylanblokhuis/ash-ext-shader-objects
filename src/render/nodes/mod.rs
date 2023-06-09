use std::mem::size_of;

use ash::vk::{self, PipelineBindPoint, RenderingFlags, SampleCountFlags, ShaderStageFlags};
use bevy::prelude::*;

use crate::ctx::record_submit_commandbuffer;

use super::{
    material::Material,
    mesh::Mesh,
    pipeline::{GraphicsPipeline, GraphicsPipelineDescriptor, PrimitiveState},
    shaders::Shader,
    GpuMesh, ProcessedRenderAssets, RenderAllocator, RenderInstance, SequentialNode, CAMERA_HANDLE,
};

#[derive(Debug)]
pub struct PresentNode {
    pipeline: GraphicsPipeline,
    draw_command_recording_chunk_size: usize,
}

#[repr(C, align(16))]
#[derive(Debug, Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct PushConstants {
    model: Mat4,
    material_pointer: u64,
    camera_pointer: u64,
}

impl PresentNode {
    pub fn new(render_instance: &RenderInstance, _render_allocator: &mut RenderAllocator) -> Self {
        let vert = Shader::from_file(
            render_instance,
            "./shader/main.vert",
            super::shaders::ShaderKind::Vertex,
            "main",
        );
        let frag = Shader::from_file(
            render_instance,
            "./shader/main.frag",
            super::shaders::ShaderKind::Fragment,
            "main",
        );

        let pipeline = GraphicsPipeline::new(
            render_instance,
            GraphicsPipelineDescriptor {
                vertex_shader: vert,
                vertex_input: vk::PipelineVertexInputStateCreateInfo::default()
                    .vertex_binding_descriptions(&[GpuMesh::vertex_binding_descriptors()])
                    .vertex_attribute_descriptions(&GpuMesh::vertex_input_descriptors()),
                fragment_shader: frag,
                primitive: PrimitiveState {
                    topology: vk::PrimitiveTopology::TRIANGLE_LIST,
                    ..Default::default()
                },
                depth_stencil: None,
                push_constant_range: Some(
                    vk::PushConstantRange::default()
                        .stage_flags(ShaderStageFlags::ALL_GRAPHICS)
                        .offset(0)
                        .size(size_of::<PushConstants>() as u32),
                ),
                viewport: render_instance.0.surface_resolution,
            },
        );

        Self {
            pipeline,
            draw_command_recording_chunk_size: 50,
        }
    }
}

impl SequentialNode for PresentNode {
    #[tracing::instrument(name = "PresentNode::update", skip_all)]
    fn update(&mut self, world: &mut bevy::prelude::World) {
        if !world
            .resource_mut::<super::global_descriptors::GlobalDescriptorSet>()
            .is_changed()
        {
            return;
        }

        world.resource_scope(
            |world, mut global_descriptors: Mut<super::global_descriptors::GlobalDescriptorSet>| {
                global_descriptors.update_descriptor_set(
                    self.pipeline.descriptor_sets[0],
                    world.resource::<RenderInstance>(),
                )
            },
        );
    }

    #[tracing::instrument(name = "PresentNode::run", skip_all)]
    fn run(&self, world: &mut bevy::prelude::World) -> anyhow::Result<()> {
        let mut objects = world.query::<(&Handle<Mesh>, &Handle<Material>, &Transform)>();
        let assets = world.resource::<ProcessedRenderAssets>();
        let global_descriptors = world.resource::<super::global_descriptors::GlobalDescriptorSet>();

        let render_instance = world.resource::<RenderInstance>();
        let objects_count = objects.iter(world).count();

        if objects_count == 0 {
            return Ok(());
        }

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

                    renderer
                        .synchronization2
                        .cmd_pipeline_barrier2(draw_command_buffer, &dependency_info);
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
                    .flags(RenderingFlags::CONTENTS_SECONDARY_COMMAND_BUFFERS)
                    .render_area(renderer.surface_resolution.into())
                    .layer_count(1)
                    .color_attachments(color_attach)
                    .depth_attachment(depth_attach);

                renderer
                    .dynamic_rendering
                    .cmd_begin_rendering(draw_command_buffer, &render_pass_begin_info);

                device.cmd_bind_pipeline(
                    draw_command_buffer,
                    PipelineBindPoint::GRAPHICS,
                    self.pipeline.pipeline,
                );

                device.cmd_bind_descriptor_sets(
                    draw_command_buffer,
                    PipelineBindPoint::GRAPHICS,
                    self.pipeline.layout,
                    0,
                    &self.pipeline.descriptor_sets,
                    &[],
                );

                device.cmd_set_viewport(
                    draw_command_buffer,
                    0,
                    &[vk::Viewport {
                        x: 0.0,
                        y: 0.0,
                        width: renderer.surface_resolution.width as f32,
                        height: renderer.surface_resolution.height as f32,
                        min_depth: 0.0,
                        max_depth: 1.0,
                    }],
                );

                device.cmd_set_scissor(
                    draw_command_buffer,
                    0,
                    &[renderer.surface_resolution.into()],
                );

                let secondary_command_buffers = renderer.threaded_command_buffers.read().unwrap();
                // reset all secondary command buffers
                secondary_command_buffers.iter().for_each(|(_, buffer)| {
                    let color_attachment_formats = &[renderer.surface_format.format];
                    let mut command_buffer_inheritance_info =
                        vk::CommandBufferInheritanceRenderingInfo::default()
                            .view_mask(0)
                            .color_attachment_formats(color_attachment_formats)
                            .depth_attachment_format(renderer.depth_image_format)
                            .rasterization_samples(SampleCountFlags::TYPE_1);

                    let inheritence_info = vk::CommandBufferInheritanceInfo::default()
                        .push_next(&mut command_buffer_inheritance_info);

                    let command_buffer_begin_info = vk::CommandBufferBeginInfo::default()
                        .flags(vk::CommandBufferUsageFlags::RENDER_PASS_CONTINUE)
                        .inheritance_info(&inheritence_info);

                    device
                        .begin_command_buffer(*buffer, &command_buffer_begin_info)
                        .expect("Begin commandbuffer");
                });

                let chunk_amount = self.draw_command_recording_chunk_size;
                let chunked_handles: Vec<Vec<(&Handle<Mesh>, &Handle<Material>, &Transform)>> =
                    objects
                        .iter(world)
                        .collect::<Vec<_>>()
                        .chunks(chunk_amount)
                        .map(|c| c.to_vec())
                        .collect::<Vec<_>>();

                let queue =
                    crossbeam_queue::ArrayQueue::<usize>::new(chunked_handles.len() * chunk_amount);
                let camera_pointer = global_descriptors
                    .buffers
                    .get(&CAMERA_HANDLE)
                    .unwrap()
                    .device_addr;

                render_instance.0.command_thread_pool.scope(|scope| {
                    let _ = info_span!("PresentNode::run::recording_draw_commands").entered();
                    for chunk in chunked_handles.iter() {
                        scope.spawn(|_| {
                            let thread_index = rayon::current_thread_index().unwrap();
                            let command_buffers = renderer.threaded_command_buffers.read().unwrap();
                            let command_buffer = command_buffers.get(&thread_index).unwrap();
                            let draw_command_buffer = *command_buffer;
                            for (mesh_handle, material_handle, transform) in chunk.iter() {
                                device.cmd_push_constants(
                                    draw_command_buffer,
                                    self.pipeline.layout,
                                    vk::ShaderStageFlags::ALL_GRAPHICS,
                                    0,
                                    bytemuck::bytes_of(&PushConstants {
                                        model: transform.compute_matrix(),
                                        camera_pointer,
                                        material_pointer: global_descriptors
                                            .buffers
                                            .get(&material_handle.id())
                                            .unwrap()
                                            .device_addr,
                                    }),
                                );

                                let mesh = &assets.meshes.get(mesh_handle).unwrap();

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
                                    device.cmd_draw_indexed(
                                        draw_command_buffer,
                                        mesh.index_count,
                                        1,
                                        0,
                                        0,
                                        1,
                                    );
                                } else {
                                    device.cmd_draw(
                                        draw_command_buffer,
                                        mesh.vertex_count,
                                        1,
                                        0,
                                        1,
                                    );
                                }
                            }
                            queue.push(thread_index).unwrap();
                        });
                    }
                });

                secondary_command_buffers.iter().for_each(|(_, buffer)| {
                    device
                        .end_command_buffer(*buffer)
                        .expect("End commandbuffer");
                });

                let queue = queue.into_iter().collect::<Vec<_>>();

                renderer.device.cmd_execute_commands(
                    draw_command_buffer,
                    &secondary_command_buffers
                        .iter()
                        .filter(|(thread_index, _)| queue.contains(thread_index))
                        .map(|(_, buffer)| *buffer)
                        .collect::<Vec<_>>(),
                );

                renderer
                    .dynamic_rendering
                    .cmd_end_rendering(draw_command_buffer);

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

                    renderer
                        .synchronization2
                        .cmd_pipeline_barrier2(draw_command_buffer, &dependency_info);
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
