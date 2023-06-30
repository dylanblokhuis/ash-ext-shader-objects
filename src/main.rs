#![allow(clippy::too_many_arguments, clippy::type_complexity)]

use bevy::app::PluginGroupBuilder;
use bevy::diagnostic::FrameTimeDiagnosticsPlugin;
use bevy::diagnostic::LogDiagnosticsPlugin;
use bevy::prelude::*;
use bevy::window::close_on_esc;
use bevy::window::WindowMode;
use bevy_runner::config::VulkanSettings;
use bevy_runner::VulkanWinitPlugin;
use render::bundles::MaterialMeshBundle;
use render::mesh::Mesh;
use render::primitives::Box;
use render::RenderPlugin;
use std::default::Default;

mod bevy_runner;
mod buffer;
mod chunky_list;
mod ctx;
mod passes;
mod render;

pub struct PluginBundle;

impl PluginGroup for PluginBundle {
    fn build(self) -> PluginGroupBuilder {
        PluginGroupBuilder::start::<PluginBundle>()
            .add(bevy::input::InputPlugin)
            .add(bevy::window::WindowPlugin::default())
            .add(VulkanWinitPlugin::default())
    }
}

fn main() {
    App::new()
        .insert_resource(VulkanSettings::default())
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                resolution: (1280.0, 720.0).into(),
                title: "Bevy Vulkano Primary Window".to_string(),
                present_mode: bevy::window::PresentMode::Mailbox,
                resizable: false,
                mode: WindowMode::Windowed,
                ..default()
            }),
            ..default()
        }))
        .add_plugin(VulkanWinitPlugin::default())
        .add_plugin(RenderPlugin::default())
        .add_plugin(LogDiagnosticsPlugin::default())
        .add_plugin(FrameTimeDiagnosticsPlugin)
        .add_system(close_on_esc)
        .add_startup_system(spawn_stuff)
        // .add_system(frame)
        .run();
}

fn spawn_stuff(mut commands: Commands, mut meshes: ResMut<Assets<Mesh>>) {
    commands.spawn(MaterialMeshBundle {
        mesh: meshes.add(Box::new(1.0, 1.0, 1.0).into()),
        transform: Transform::from_translation(Vec3::new(0.0, 0.0, 0.0)),
    });

    commands.spawn(MaterialMeshBundle {
        mesh: meshes.add(Box::new(1.0, 1.0, 1.0).into()),
        transform: Transform::from_translation(Vec3::new(5.0, 0.0, 5.0)),
    });
}

// fn frame(
//     window_query: Query<(Entity, Option<&PrimaryWindow>), With<Window>>,
//     vulkano_windows: NonSendMut<BevyVulkanoWindows>,
// ) {
//     for (window, _) in window_query.iter() {
//         if let Some(vulkano_window) = vulkano_windows.get_vulkano_window(window) {}
//     }
// }

// #[derive(Clone, Debug, Copy, bytemuck::Pod, bytemuck::Zeroable)]
// #[repr(C, align(16))]
// struct Vertex {
//     pos: [f32; 4],
//     color: [f32; 4],
// }

// #[derive(Clone, Debug, Copy, bytemuck::Pod, bytemuck::Zeroable)]
// #[repr(C, align(16))]
// struct Misc {
//     color: [f32; 4],
// }

// #[derive(Clone, Debug, Copy, bytemuck::Pod, bytemuck::Zeroable, Default)]
// #[repr(C, align(16))]
// struct Uniform {
//     buf_pointer: u64,
//     _pad: [f32; 2],
// }

// fn main() {
//     unsafe {
//         let (window_width, window_height) = (1280, 720);
//         let mut base = ExampleBase::new(window_width, window_height);
//         // let mut event_loop = base.event_loop.borrow_mut();

//         // boot egui
//         // let mut egui_pass = EguiPass::new(&mut base);
//         // let context = egui_pass.start_painting(&base.window);
//         // egui::SidePanel::left("my_side_panel").show(context, |ui| {
//         //     ui.heading("Hello!");
//         // });
//         // egui_pass.end_painting(&base.window);

//         // let meshes = context.tessellate(output.shapes);
//         // output.textures_delta

//         // println!("{:?}", meshes);

//         let compiler = shaderc::Compiler::new().unwrap();
//         let mut options = shaderc::CompileOptions::new().unwrap();
//         options.add_macro_definition("EP", Some("main"));
//         options.set_target_env(
//             shaderc::TargetEnv::Vulkan,
//             shaderc::EnvVersion::Vulkan1_2 as u32,
//         );
//         options.set_generate_debug_info();
//         let vert = compiler
//             .compile_into_spirv(
//                 &std::fs::read_to_string(r#"C:\Users\dylan\dev\someday\shader\main.vert"#).unwrap(),
//                 shaderc::ShaderKind::Vertex,
//                 "main.vert",
//                 "main",
//                 Some(&options),
//             )
//             .unwrap();
//         let vert_spirv = vert.as_binary_u8();
//         let frag = compiler
//             .compile_into_spirv(
//                 &std::fs::read_to_string(r#"C:\Users\dylan\dev\someday\shader\main.frag"#).unwrap(),
//                 shaderc::ShaderKind::Fragment,
//                 "main.frag",
//                 "main",
//                 Some(&options),
//             )
//             .unwrap();
//         let frag_spirv = frag.as_binary_u8();

//         let refl_info = rspirv_reflect::Reflection::new_from_spirv(frag_spirv).unwrap();
//         let sets = refl_info.get_descriptor_sets().unwrap();

//         println!("{:?}", sets);

//         let (mut index_buffer, index_len) = {
//             let index_buffer_data = [0u32, 1, 2];

//             let buf = Buffer::new(
//                 &base.device,
//                 &mut base.allocator,
//                 &vk::BufferCreateInfo::default()
//                     .size(std::mem::size_of_val(&index_buffer_data) as u64)
//                     .usage(vk::BufferUsageFlags::INDEX_BUFFER)
//                     .sharing_mode(vk::SharingMode::EXCLUSIVE),
//                 MemoryLocation::CpuToGpu,
//             );

//             buf.copy_from_slice(&index_buffer_data, 0);
//             (buf, index_buffer_data.len() as u32)
//         };

//         let mut vertex_buffer = {
//             let buf = Buffer::new(
//                 &base.device,
//                 &mut base.allocator,
//                 &vk::BufferCreateInfo {
//                     size: 3 * std::mem::size_of::<Vertex>() as u64,
//                     usage: vk::BufferUsageFlags::VERTEX_BUFFER,
//                     sharing_mode: vk::SharingMode::EXCLUSIVE,
//                     ..Default::default()
//                 },
//                 MemoryLocation::CpuToGpu,
//             );

//             let vertices = [
//                 Vertex {
//                     pos: [-1.0, 1.0, 0.0, 1.0],
//                     color: [0.0, 1.0, 0.0, 1.0],
//                 },
//                 Vertex {
//                     pos: [1.0, 1.0, 0.0, 1.0],
//                     color: [0.0, 0.0, 1.0, 1.0],
//                 },
//                 Vertex {
//                     pos: [0.0, -1.0, 0.0, 1.0],
//                     color: [1.0, 0.0, 0.0, 1.0],
//                 },
//             ];

//             buf.copy_from_slice(&vertices, 0);

//             buf
//         };

//         let vertex_input_binding = VertexInputBindingDescription2EXT::default()
//             .binding(0)
//             .input_rate(VertexInputRate::VERTEX)
//             .divisor(1)
//             .stride(std::mem::size_of::<Vertex>() as u32);
//         let vertex_input_attribute = &[
//             VertexInputAttributeDescription2EXT::default()
//                 .binding(0)
//                 .location(0)
//                 .format(Format::R32G32B32A32_SFLOAT)
//                 .offset(offset_of!(Vertex, pos) as u32),
//             VertexInputAttributeDescription2EXT::default()
//                 .binding(0)
//                 .location(1)
//                 .format(Format::R32G32B32A32_SFLOAT)
//                 .offset(offset_of!(Vertex, color) as u32),
//         ];

//         let viewports = [vk::Viewport {
//             x: 0.0,
//             y: 0.0,
//             width: base.surface_resolution.width as f32,
//             height: base.surface_resolution.height as f32,
//             min_depth: 0.0,
//             max_depth: 1.0,
//         }];
//         let scissors = [base.surface_resolution.into()];

//         // let mut output_tex = Image::new(
//         //     &base.device,
//         //     &mut base.allocator,
//         //     &vk::ImageCreateInfo {
//         //         image_type: vk::ImageType::TYPE_2D,
//         //         format: vk::Format::R16G16B16A16_SFLOAT,
//         //         extent: vk::Extent3D {
//         //             width: window_width,
//         //             height: window_height,
//         //             depth: 1,
//         //         },
//         //         mip_levels: 1,
//         //         array_layers: 1,
//         //         samples: vk::SampleCountFlags::TYPE_1,
//         //         tiling: vk::ImageTiling::OPTIMAL,
//         //         usage: vk::ImageUsageFlags::STORAGE,
//         //         sharing_mode: vk::SharingMode::EXCLUSIVE,
//         //         ..Default::default()
//         //     },
//         //     MemoryLocation::GpuOnly,
//         // );

//         // let device_address = base.device.get_buffer_device_address(
//         //     &BufferDeviceAddressInfo::default().buffer(vertex_buffer.buffer),
//         // );
//         // let comp_pass = CompPass::new(&mut base, &mut output_tex);

//         let descriptor_sizes = [vk::DescriptorPoolSize {
//             ty: vk::DescriptorType::UNIFORM_BUFFER,
//             descriptor_count: 1,
//         }];
//         let descriptor_pool_info = vk::DescriptorPoolCreateInfo::default()
//             .pool_sizes(&descriptor_sizes)
//             .max_sets(1);

//         let descriptor_pool = base
//             .device
//             .create_descriptor_pool(&descriptor_pool_info, None)
//             .unwrap();

//         let desc_layout_bindings = [vk::DescriptorSetLayoutBinding {
//             binding: 0,
//             descriptor_type: vk::DescriptorType::UNIFORM_BUFFER,
//             descriptor_count: 1,
//             stage_flags: vk::ShaderStageFlags::ALL,
//             ..Default::default()
//         }];

//         let descriptor_info =
//             vk::DescriptorSetLayoutCreateInfo::default().bindings(&desc_layout_bindings);

//         let desc_set_layouts = [base
//             .device
//             .create_descriptor_set_layout(&descriptor_info, None)
//             .unwrap()];

//         let desc_alloc_info = vk::DescriptorSetAllocateInfo::default()
//             .descriptor_pool(descriptor_pool)
//             .set_layouts(&desc_set_layouts);
//         let descriptor_sets = base
//             .device
//             .allocate_descriptor_sets(&desc_alloc_info)
//             .unwrap();

//         let misc_buf = {
//             let buf = Buffer::new(
//                 &base.device,
//                 &mut base.allocator,
//                 &vk::BufferCreateInfo::default()
//                     .size(std::mem::size_of::<Misc>() as u64)
//                     .usage(vk::BufferUsageFlags::RESOURCE_DESCRIPTOR_BUFFER_EXT)
//                     .sharing_mode(vk::SharingMode::EXCLUSIVE),
//                 MemoryLocation::CpuToGpu,
//             );

//             let colors = &[Misc {
//                 color: [0.0, 1.0, 0.0, 1.0],
//             }];

//             println!("misc type size {:?}", std::mem::size_of::<Misc>() as u64);
//             println!("misc buf size {:?}", size_of_val(colors));

//             buf.copy_from_slice(colors, 0);

//             buf
//         };

//         let uniform_buf = {
//             let buf = Buffer::new(
//                 &base.device,
//                 &mut base.allocator,
//                 &vk::BufferCreateInfo::default()
//                     .size(std::mem::size_of::<Uniform>() as u64)
//                     .usage(
//                         vk::BufferUsageFlags::SHADER_DEVICE_ADDRESS
//                             | vk::BufferUsageFlags::UNIFORM_BUFFER,
//                     )
//                     .sharing_mode(vk::SharingMode::EXCLUSIVE),
//                 MemoryLocation::CpuToGpu,
//             );

//             let uniform = Uniform {
//                 buf_pointer: misc_buf.device_addr,
//                 ..Default::default()
//             };

//             println!(
//                 "uniform type size {:?}",
//                 std::mem::size_of::<Uniform>() as u64
//             );
//             println!("uniform buf size {:?}", size_of_val(&[uniform]));

//             buf.copy_from_slice(&[uniform], 0);

//             buf
//         };

//         let uniform_buffer_descriptor = &[vk::DescriptorBufferInfo::default()
//             .buffer(uniform_buf.buffer)
//             .range(uniform_buf.size)
//             .offset(0)];

//         let write_desc_sets = [vk::WriteDescriptorSet::default()
//             .dst_set(descriptor_sets[0])
//             .dst_binding(0)
//             .descriptor_type(vk::DescriptorType::UNIFORM_BUFFER)
//             .buffer_info(uniform_buffer_descriptor)];

//         base.device.update_descriptor_sets(&write_desc_sets, &[]);

//         let pipeline_layout = base
//             .device
//             .create_pipeline_layout(
//                 &vk::PipelineLayoutCreateInfo::default().set_layouts(&desc_set_layouts),
//                 None,
//             )
//             .unwrap();

//         let shaders = base
//             .shader_object
//             .create_shaders(
//                 &[
//                     ShaderCreateInfoEXT::default()
//                         .name(CStr::from_bytes_with_nul_unchecked(b"main\0"))
//                         .code(vert_spirv)
//                         .code_type(ShaderCodeTypeEXT::SPIRV)
//                         .stage(ShaderStageFlags::VERTEX)
//                         .set_layouts(&desc_set_layouts),
//                     ShaderCreateInfoEXT::default()
//                         .name(CStr::from_bytes_with_nul_unchecked(b"main\0"))
//                         .code(frag_spirv)
//                         .code_type(ShaderCodeTypeEXT::SPIRV)
//                         .stage(ShaderStageFlags::FRAGMENT)
//                         .set_layouts(&desc_set_layouts), // .push_constant_ranges(&[vk::PushConstantRange::default()
//                                                          //     .stage_flags(ShaderStageFlags::VERTEX)
//                                                          //     .size(mem::size_of::<PushConstants>() as u32)
//                                                          //     .offset(0)]),
//                 ],
//                 None,
//             )
//             .unwrap();

//         // VK_EXT_descriptor_buffer
//         // let descriptor_buffer = {
//         //     let descriptor_size = base
//         //         .descriptor_buffer
//         //         .get_descriptor_set_layout_size(desc_set_layouts[0]);

//         //     println!("descriptor size {:?}", descriptor_size);

//         //     let buf = Buffer::new(
//         //         &base.device,
//         //         &mut base.allocator,
//         //         &vk::BufferCreateInfo::default()
//         //             .size(descriptor_size)
//         //             .usage(
//         //                 vk::BufferUsageFlags::RESOURCE_DESCRIPTOR_BUFFER_EXT
//         //                     | vk::BufferUsageFlags::SHADER_DEVICE_ADDRESS,
//         //             )
//         //             .sharing_mode(vk::SharingMode::EXCLUSIVE),
//         //         MemoryLocation::CpuToGpu,
//         //     );

//         //     let mut data = vec![0; descriptor_size as usize];

//         //     base.descriptor_buffer.get_descriptor(
//         //         &DescriptorGetInfoEXT::default()
//         //             .ty(DescriptorType::UNIFORM_BUFFER)
//         //             .data(DescriptorDataEXT {
//         //                 p_uniform_buffer: &DescriptorAddressInfoEXT::default()
//         //                     .address(uniform_buf.device_addr)
//         //                     .range(uniform_buf.size)
//         //                     .format(vk::Format::UNDEFINED),
//         //             }),
//         //         &mut data,
//         //     );

//         //     buf.copy_from_slice(&data, 0);

//         //     buf
//         // };

//         base.event_loop.run_return(|event, _, control_flow| {
//             // comp_pass.run(&base);
//             *control_flow = ControlFlow::Poll;

//             match event {
//                 winit::event::Event::WindowEvent {
//                     event:
//                         WindowEvent::CloseRequested
//                         | WindowEvent::KeyboardInput {
//                             input:
//                                 KeyboardInput {
//                                     state: ElementState::Pressed,
//                                     virtual_keycode: Some(VirtualKeyCode::Escape),
//                                     ..
//                                 },
//                             ..
//                         },
//                     ..
//                 } => {
//                     *control_flow = ControlFlow::Exit;
//                 }
//                 winit::event::Event::MainEventsCleared => {
//                     let present_index = base
//                         .swapchain_loader
//                         .acquire_next_image(
//                             base.swapchain,
//                             std::u64::MAX,
//                             base.present_complete_semaphore,
//                             vk::Fence::null(),
//                         )
//                         .unwrap()
//                         .0;

//                     record_submit_commandbuffer(
//                         &base.device,
//                         base.draw_command_buffer,
//                         base.draw_commands_reuse_fence,
//                         base.present_queue,
//                         &[vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT],
//                         &[base.present_complete_semaphore],
//                         &[base.rendering_complete_semaphore],
//                         |device, draw_command_buffer| {
//                             {
//                                 let image_memory_barrier = vk::ImageMemoryBarrier2::default()
//                                     .src_stage_mask(
//                                         vk::PipelineStageFlags2::COLOR_ATTACHMENT_OUTPUT,
//                                     )
//                                     .src_access_mask(vk::AccessFlags2::COLOR_ATTACHMENT_READ)
//                                     .old_layout(vk::ImageLayout::UNDEFINED)
//                                     .dst_stage_mask(
//                                         vk::PipelineStageFlags2::COLOR_ATTACHMENT_OUTPUT,
//                                     )
//                                     .dst_access_mask(vk::AccessFlags2::COLOR_ATTACHMENT_WRITE)
//                                     .new_layout(vk::ImageLayout::ATTACHMENT_OPTIMAL)
//                                     .image(base.present_images[present_index as usize])
//                                     .subresource_range(vk::ImageSubresourceRange {
//                                         aspect_mask: vk::ImageAspectFlags::COLOR,
//                                         layer_count: 1,
//                                         level_count: 1,
//                                         ..Default::default()
//                                     });

//                                 let dependency_info = vk::DependencyInfo::default()
//                                     .image_memory_barriers(std::slice::from_ref(
//                                         &image_memory_barrier,
//                                     ));

//                                 device.cmd_pipeline_barrier2(draw_command_buffer, &dependency_info);
//                             }

//                             let color_attach = &[vk::RenderingAttachmentInfo::default()
//                                 .image_view(base.present_image_views[present_index as usize])
//                                 .image_layout(vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL)
//                                 .load_op(vk::AttachmentLoadOp::CLEAR)
//                                 .store_op(vk::AttachmentStoreOp::STORE)
//                                 .clear_value(vk::ClearValue {
//                                     color: vk::ClearColorValue {
//                                         float32: [0.1, 0.1, 0.1, 1.0],
//                                     },
//                                 })];

//                             let depth_attach = &vk::RenderingAttachmentInfo::default()
//                                 .image_view(base.depth_image_view)
//                                 .image_layout(vk::ImageLayout::DEPTH_ATTACHMENT_OPTIMAL)
//                                 .load_op(vk::AttachmentLoadOp::CLEAR)
//                                 .store_op(vk::AttachmentStoreOp::STORE)
//                                 .clear_value(vk::ClearValue {
//                                     depth_stencil: vk::ClearDepthStencilValue {
//                                         depth: 1.0,
//                                         stencil: 0,
//                                     },
//                                 });

//                             let render_pass_begin_info = vk::RenderingInfo::default()
//                                 .render_area(base.surface_resolution.into())
//                                 .layer_count(1)
//                                 .color_attachments(color_attach)
//                                 .depth_attachment(depth_attach);

//                             device
//                                 .cmd_begin_rendering(draw_command_buffer, &render_pass_begin_info);
//                             device.cmd_bind_descriptor_sets(
//                                 draw_command_buffer,
//                                 PipelineBindPoint::GRAPHICS,
//                                 pipeline_layout,
//                                 0,
//                                 &descriptor_sets,
//                                 &[],
//                             );

//                             // println!("yo");

//                             // base.descriptor_buffer.cmd_bind_descriptor_buffers(
//                             //     draw_command_buffer,
//                             //     &[vk::DescriptorBufferBindingInfoEXT::default()
//                             //         .address(descriptor_buffer.device_addr)
//                             //         .usage(vk::BufferUsageFlags::RESOURCE_DESCRIPTOR_BUFFER_EXT)],
//                             // );
//                             // println!("yo1");

//                             base.shader_object
//                                 .cmd_set_viewport_with_count(draw_command_buffer, &viewports);
//                             base.shader_object
//                                 .cmd_set_scissor_with_count(draw_command_buffer, &scissors);
//                             base.shader_object
//                                 .cmd_set_cull_mode(draw_command_buffer, CullModeFlags::BACK);
//                             base.shader_object.cmd_set_front_face(
//                                 draw_command_buffer,
//                                 FrontFace::COUNTER_CLOCKWISE,
//                             );
//                             base.shader_object
//                                 .cmd_set_depth_test_enable(draw_command_buffer, true);
//                             base.shader_object
//                                 .cmd_set_depth_write_enable(draw_command_buffer, true);
//                             base.shader_object.cmd_set_depth_compare_op(
//                                 draw_command_buffer,
//                                 CompareOp::LESS_OR_EQUAL,
//                             );
//                             base.shader_object.cmd_set_primitive_topology(
//                                 draw_command_buffer,
//                                 PrimitiveTopology::TRIANGLE_LIST,
//                             );

//                             base.shader_object.cmd_set_vertex_input(
//                                 draw_command_buffer,
//                                 &[vertex_input_binding],
//                                 vertex_input_attribute,
//                             );

//                             base.shader_object.cmd_bind_shaders(
//                                 draw_command_buffer,
//                                 &[ShaderStageFlags::VERTEX, ShaderStageFlags::FRAGMENT],
//                                 &shaders,
//                             );

//                             device.cmd_bind_vertex_buffers(
//                                 draw_command_buffer,
//                                 0,
//                                 &[vertex_buffer.buffer],
//                                 &[0],
//                             );
//                             device.cmd_bind_index_buffer(
//                                 draw_command_buffer,
//                                 index_buffer.buffer,
//                                 0,
//                                 vk::IndexType::UINT32,
//                             );
//                             device.cmd_draw_indexed(draw_command_buffer, index_len, 1, 0, 0, 1);
//                             device.cmd_end_rendering(draw_command_buffer);
//                             {
//                                 let image_memory_barrier = vk::ImageMemoryBarrier2::default()
//                                     .src_stage_mask(
//                                         vk::PipelineStageFlags2::COLOR_ATTACHMENT_OUTPUT,
//                                     )
//                                     .src_access_mask(vk::AccessFlags2::COLOR_ATTACHMENT_WRITE)
//                                     .old_layout(vk::ImageLayout::ATTACHMENT_OPTIMAL)
//                                     .dst_stage_mask(
//                                         vk::PipelineStageFlags2::COLOR_ATTACHMENT_OUTPUT,
//                                     )
//                                     .dst_access_mask(vk::AccessFlags2::COLOR_ATTACHMENT_READ)
//                                     .new_layout(vk::ImageLayout::PRESENT_SRC_KHR)
//                                     .image(base.present_images[present_index as usize])
//                                     .subresource_range(vk::ImageSubresourceRange {
//                                         aspect_mask: vk::ImageAspectFlags::COLOR,
//                                         layer_count: 1,
//                                         level_count: 1,
//                                         ..Default::default()
//                                     });

//                                 let dependency_info = vk::DependencyInfo::default()
//                                     .image_memory_barriers(std::slice::from_ref(
//                                         &image_memory_barrier,
//                                     ));

//                                 device.cmd_pipeline_barrier2(draw_command_buffer, &dependency_info);
//                             }
//                         },
//                     );

//                     let wait_semaphors = [base.rendering_complete_semaphore];
//                     let swapchains = [base.swapchain];
//                     let image_indices = [present_index];
//                     let present_info = vk::PresentInfoKHR::default()
//                         .wait_semaphores(&wait_semaphors)
//                         .swapchains(&swapchains)
//                         .image_indices(&image_indices);

//                     base.swapchain_loader
//                         .queue_present(base.present_queue, &present_info)
//                         .unwrap();
//                 }
//                 _ => {}
//             }
//         });

//         base.device.device_wait_idle().unwrap();

//         for shader in shaders.iter() {
//             base.shader_object.destroy_shader(*shader, None);
//         }

//         // output_tex.destroy(&base.device, &mut base.allocator);
//         index_buffer.destroy(&base.device, &mut base.allocator);
//         vertex_buffer.destroy(&base.device, &mut base.allocator);
//     }
// }
