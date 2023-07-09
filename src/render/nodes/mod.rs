use std::{mem::size_of, sync::Arc};

use ash::vk::{
    self, CompareOp, CullModeFlags, DeviceSize, FrontFace, PipelineBindPoint, RenderingFlags,
    SampleCountFlags, ShaderEXT, ShaderStageFlags,
};
use bevy::{ecs::system::SystemState, prelude::*};
use gpu_allocator::MemoryLocation;

use crate::{
    buffer::{Buffer, Image},
    ctx::record_submit_commandbuffer,
};

use super::{
    extract::Extract, material::Material, mesh::Mesh, shaders::Shader, GpuMesh,
    ProcessedRenderAssets, RenderAllocator, RenderInstance, SequentialNode, CAMERA_HANDLE,
};

#[derive(Debug)]
pub struct PresentNode {
    shaders: Vec<ShaderEXT>,
    descriptor_sets: Vec<vk::DescriptorSet>,
    pipeline_layout: vk::PipelineLayout,
}

#[repr(C, align(16))]
#[derive(Debug, Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct PushConstants {
    model: Mat4,
    material_pointer: u64,
    camera_pointer: u64,
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
            vert.create_descriptor_set_layouts(render_instance);

        let descriptor_sets =
            vert.create_descriptor_sets(render_instance, &descriptor_set_layouts, &set_layout_info);

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

        Self {
            shaders,
            descriptor_sets,
            pipeline_layout,
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
                    self.descriptor_sets[0],
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

                let queue = crossbeam_queue::ArrayQueue::<usize>::new(objects_count);
                let camera_pointer = global_descriptors
                    .buffers
                    .get(&CAMERA_HANDLE)
                    .unwrap()
                    .device_addr;

                render_instance.0.command_thread_pool.scope(|scope| {
                    let _ = info_span!("PresentNode::run::command_thread_pool").entered();
                    for (mesh_handle, material_handle, transform) in objects.iter(world) {
                        scope.spawn(|_| {
                            let thread_index = rayon::current_thread_index().unwrap();
                            let command_buffers = renderer.threaded_command_buffers.read().unwrap();
                            let command_buffer = command_buffers.get(&thread_index).unwrap();
                            let draw_command_buffer = *command_buffer;

                            let mesh = &assets.meshes.get(mesh_handle).unwrap();
                            device.cmd_push_constants(
                                draw_command_buffer,
                                self.pipeline_layout,
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
                                device.cmd_draw_indexed(
                                    draw_command_buffer,
                                    mesh.index_count,
                                    1,
                                    0,
                                    0,
                                    1,
                                );
                            } else {
                                device.cmd_draw(draw_command_buffer, mesh.vertex_count, 1, 0, 1);
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
