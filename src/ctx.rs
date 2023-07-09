#[cfg(any(target_os = "macos", target_os = "ios"))]
use ash::vk::{
    KhrGetPhysicalDeviceProperties2Fn, KhrPortabilityEnumerationFn, KhrPortabilitySubsetFn,
};
use ash::{
    extensions::{
        ext::{DebugUtils, ShaderObject},
        khr::{DynamicRendering, Surface, Swapchain, Synchronization2},
    },
    vk::{
        BufferImageCopy, CommandBuffer, ExtDescriptorIndexingFn, ImageLayout,
        PhysicalDeviceBufferDeviceAddressFeaturesKHR, PhysicalDeviceDescriptorIndexingFeatures,
        PhysicalDeviceShaderObjectFeaturesEXT, API_VERSION_1_2,
    },
};
use ash::{vk, Entry};
use ash::{Device, Instance};
use bevy::window::{PresentMode, RawHandleWrapper};
use rayon::ThreadPool;
use std::default::Default;
use std::ffi::CStr;
use std::{borrow::Cow, collections::HashMap};
use std::{ops::Drop, sync::RwLock};
use std::{os::raw::c_char, sync::Arc};

use crate::buffer::{Buffer, Image};

// /// Helper function for submitting command buffers. Immediately waits for the fence before the command buffer
// /// is executed. That way we can delay the waiting for the fences by 1 frame which is good for performance.
// /// Make sure to create the fence in a signaled state on the first use.
// pub fn record_commandbuffer<F: FnOnce(&Device, vk::CommandBuffer)>(
//     device: &Device,
//     command_buffer: vk::CommandBuffer,
//     fence: vk::Fence,
//     color_attachment_formats: &[vk::Format],
//     depth_attachment_format: Option<vk::Format>,
//     stencil_attachment_format: Option<vk::Format>,
//     f: F,
// ) {
//     unsafe {
//         // device
//         //     .reset_command_buffer(
//         //         command_buffer,
//         //         vk::CommandBufferResetFlags::RELEASE_RESOURCES,
//         //     )
//         //     .expect("Reset command buffer failed.");
//         // device
//         //     .wait_for_fences(&[fence], true, std::u64::MAX)
//         //     .expect("Wait for fence failed.");

//         // device.reset_fences(&[fence]).expect("Reset fences failed.");

//         let mut command_buffer_inheritance_info =
//             vk::CommandBufferInheritanceRenderingInfo::default()
//                 .view_mask(0)
//                 .color_attachment_formats(color_attachment_formats)
//                 .depth_attachment_format(
//                     depth_attachment_format.map_or(vk::Format::UNDEFINED, Into::into),
//                 )
//                 .stencil_attachment_format(
//                     stencil_attachment_format.map_or(vk::Format::UNDEFINED, Into::into),
//                 )
//                 .rasterization_samples(SampleCountFlags::TYPE_1);

//         let inheritence_info = vk::CommandBufferInheritanceInfo::default()
//             .push_next(&mut command_buffer_inheritance_info);

//         let command_buffer_begin_info = vk::CommandBufferBeginInfo::default()
//             .flags(vk::CommandBufferUsageFlags::RENDER_PASS_CONTINUE)
//             .inheritance_info(&inheritence_info);
//         device
//             .begin_command_buffer(command_buffer, &command_buffer_begin_info)
//             .expect("Begin commandbuffer");
//         f(device, command_buffer);
//         device
//             .end_command_buffer(command_buffer)
//             .expect("End commandbuffer");
//     }
// }

/// Helper function for submitting command buffers. Immediately waits for the fence before the command buffer
/// is executed. That way we can delay the waiting for the fences by 1 frame which is good for performance.
/// Make sure to create the fence in a signaled state on the first use.
pub fn record_submit_commandbuffer<F: FnOnce(&Device, vk::CommandBuffer)>(
    device: &Device,
    command_buffer: vk::CommandBuffer,
    command_buffer_reuse_fence: vk::Fence,
    submit_queue: vk::Queue,
    wait_mask: &[vk::PipelineStageFlags],
    wait_semaphores: &[vk::Semaphore],
    signal_semaphores: &[vk::Semaphore],
    f: F,
) {
    unsafe {
        device
            .wait_for_fences(&[command_buffer_reuse_fence], true, std::u64::MAX)
            .expect("Wait for fence failed.");

        device
            .reset_fences(&[command_buffer_reuse_fence])
            .expect("Reset fences failed.");

        device
            .reset_command_buffer(
                command_buffer,
                vk::CommandBufferResetFlags::RELEASE_RESOURCES,
            )
            .expect("Reset command buffer failed.");

        let command_buffer_begin_info = vk::CommandBufferBeginInfo::default()
            .flags(vk::CommandBufferUsageFlags::ONE_TIME_SUBMIT);

        device
            .begin_command_buffer(command_buffer, &command_buffer_begin_info)
            .expect("Begin commandbuffer");
        f(device, command_buffer);
        device
            .end_command_buffer(command_buffer)
            .expect("End commandbuffer");

        let command_buffers = vec![command_buffer];

        let submit_info = vk::SubmitInfo::default()
            .wait_semaphores(wait_semaphores)
            .wait_dst_stage_mask(wait_mask)
            .command_buffers(&command_buffers)
            .signal_semaphores(signal_semaphores);

        device
            .queue_submit(submit_queue, &[submit_info], command_buffer_reuse_fence)
            .expect("queue submit failed.");
        device.queue_wait_idle(submit_queue).unwrap();
    }
}

unsafe extern "system" fn vulkan_debug_callback(
    message_severity: vk::DebugUtilsMessageSeverityFlagsEXT,
    message_type: vk::DebugUtilsMessageTypeFlagsEXT,
    p_callback_data: *const vk::DebugUtilsMessengerCallbackDataEXT,
    _user_data: *mut std::os::raw::c_void,
) -> vk::Bool32 {
    let callback_data = *p_callback_data;
    let message_id_number = callback_data.message_id_number;

    let message_id_name = if callback_data.p_message_id_name.is_null() {
        Cow::from("")
    } else {
        CStr::from_ptr(callback_data.p_message_id_name).to_string_lossy()
    };

    let message = if callback_data.p_message.is_null() {
        Cow::from("")
    } else {
        CStr::from_ptr(callback_data.p_message).to_string_lossy()
    };

    println!(
      "{message_severity:?}:\n{message_type:?} [{message_id_name} ({message_id_number})] : {message}\n",
  );

    vk::FALSE
}

pub fn find_memorytype_index(
    memory_req: &vk::MemoryRequirements,
    memory_prop: &vk::PhysicalDeviceMemoryProperties,
    flags: vk::MemoryPropertyFlags,
) -> Option<u32> {
    memory_prop.memory_types[..memory_prop.memory_type_count as _]
        .iter()
        .enumerate()
        .find(|(index, memory_type)| {
            (1 << index) & memory_req.memory_type_bits != 0
                && memory_type.property_flags & flags == flags
        })
        .map(|(index, _memory_type)| index as _)
}

#[derive(Clone, Copy, Hash, PartialEq, Eq, Debug)]
pub struct SamplerDesc {
    pub texel_filter: vk::Filter,
    pub mipmap_mode: vk::SamplerMipmapMode,
    pub address_modes: vk::SamplerAddressMode,
}

pub struct ExampleBase {
    pub entry: Entry,
    pub instance: Instance,
    pub device: Device,
    pub shader_object: ShaderObject,
    pub synchronization2: Synchronization2,
    pub dynamic_rendering: DynamicRendering,
    pub surface_loader: Surface,
    pub swapchain_loader: Swapchain,
    pub debug_utils_loader: DebugUtils,
    pub debug_call_back: vk::DebugUtilsMessengerEXT,
    pub immutable_samplers: HashMap<SamplerDesc, vk::Sampler>,
    pub max_descriptor_count: u32,
    pub command_thread_pool: ThreadPool,
    pub threaded_command_buffers: Arc<RwLock<HashMap<usize, CommandBuffer>>>,

    pub pdevice: vk::PhysicalDevice,
    pub device_memory_properties: vk::PhysicalDeviceMemoryProperties,
    pub queue_family_index: u32,
    pub present_queue: vk::Queue,

    pub surface: vk::SurfaceKHR,
    pub surface_format: vk::SurfaceFormatKHR,
    pub surface_resolution: vk::Extent2D,

    pub swapchain: vk::SwapchainKHR,
    pub present_images: Vec<vk::Image>,
    pub present_image_views: Vec<vk::ImageView>,

    pub pool: vk::CommandPool,
    pub draw_command_buffer: vk::CommandBuffer,
    pub setup_command_buffer: vk::CommandBuffer,

    pub depth_image: vk::Image,
    pub depth_image_view: vk::ImageView,
    pub depth_image_memory: vk::DeviceMemory,
    pub depth_image_format: vk::Format,

    pub present_complete_semaphore: vk::Semaphore,
    pub rendering_complete_semaphore: vk::Semaphore,

    pub draw_commands_reuse_fence: vk::Fence,
    pub setup_commands_reuse_fence: vk::Fence,
}

impl ExampleBase {
    pub fn new(window: &RawHandleWrapper, present_mode: PresentMode) -> Self {
        unsafe {
            let entry = Entry::linked();
            let app_name = CStr::from_bytes_with_nul_unchecked(b"VulkanTriangle\0");

            let mut layer_names = vec![
                CStr::from_bytes_with_nul_unchecked(b"VK_LAYER_KHRONOS_synchronization2\0"),
                CStr::from_bytes_with_nul_unchecked(b"VK_LAYER_KHRONOS_shader_object\0"),
            ];

            if cfg!(debug_assertions) {
                println!("{:?}", "Debug mode: enable validation layers");

                layer_names.push(CStr::from_bytes_with_nul_unchecked(
                    b"VK_LAYER_KHRONOS_validation\0",
                ))
            }

            let layers_names_raw: Vec<*const c_char> = layer_names
                .iter()
                .map(|raw_name| raw_name.as_ptr())
                .collect();

            let mut extension_names =
                ash_window::enumerate_required_extensions(window.display_handle)
                    .unwrap()
                    .to_vec();
            extension_names.push(DebugUtils::NAME.as_ptr());
            #[cfg(any(target_os = "macos", target_os = "ios"))]
            {
                extension_names.push(KhrPortabilityEnumerationFn::NAME.as_ptr());
                // Enabling this extension is a requirement when using `VK_KHR_portability_subset`
                extension_names.push(KhrGetPhysicalDeviceProperties2Fn::NAME.as_ptr());
            }

            let appinfo = vk::ApplicationInfo::default()
                .application_name(app_name)
                .application_version(0)
                .engine_name(app_name)
                .engine_version(0)
                .api_version(API_VERSION_1_2);

            let create_flags = if cfg!(any(target_os = "macos", target_os = "ios")) {
                vk::InstanceCreateFlags::ENUMERATE_PORTABILITY_KHR
            } else {
                vk::InstanceCreateFlags::default()
            };

            let create_info = vk::InstanceCreateInfo::default()
                .application_info(&appinfo)
                .enabled_layer_names(&layers_names_raw)
                .enabled_extension_names(&extension_names)
                .flags(create_flags);

            let instance: Instance = entry
                .create_instance(&create_info, None)
                .expect("Instance creation error");

            let debug_info = vk::DebugUtilsMessengerCreateInfoEXT::default()
                .message_severity(
                    vk::DebugUtilsMessageSeverityFlagsEXT::ERROR
                        | vk::DebugUtilsMessageSeverityFlagsEXT::WARNING
                        | vk::DebugUtilsMessageSeverityFlagsEXT::INFO,
                )
                .message_type(
                    vk::DebugUtilsMessageTypeFlagsEXT::GENERAL
                        | vk::DebugUtilsMessageTypeFlagsEXT::VALIDATION
                        | vk::DebugUtilsMessageTypeFlagsEXT::PERFORMANCE,
                )
                .pfn_user_callback(Some(vulkan_debug_callback));

            let debug_utils_loader = DebugUtils::new(&entry, &instance);
            let debug_call_back = debug_utils_loader
                .create_debug_utils_messenger(&debug_info, None)
                .unwrap();
            let surface = ash_window::create_surface(
                &entry,
                &instance,
                window.get_display_handle(),
                window.get_window_handle(),
                None,
            )
            .unwrap();
            let pdevices = instance
                .enumerate_physical_devices()
                .expect("Physical device error");
            let surface_loader = Surface::new(&entry, &instance);
            let (pdevice, queue_family_index) = pdevices
                .iter()
                .find_map(|pdevice| {
                    instance
                        .get_physical_device_queue_family_properties(*pdevice)
                        .iter()
                        .enumerate()
                        .find_map(|(index, info)| {
                            println!("{:?}", info);

                            let supports_graphic_and_surface =
                                info.queue_flags.contains(vk::QueueFlags::GRAPHICS)
                                    && surface_loader
                                        .get_physical_device_surface_support(
                                            *pdevice,
                                            index as u32,
                                            surface,
                                        )
                                        .unwrap();
                            if supports_graphic_and_surface {
                                Some((*pdevice, index))
                            } else {
                                None
                            }
                        })
                })
                .expect("Couldn't find suitable device.");
            let queue_family_index = queue_family_index as u32;
            let device_extension_names_raw = [
                Swapchain::NAME.as_ptr(),
                DynamicRendering::NAME.as_ptr(),
                Synchronization2::NAME.as_ptr(),
                ShaderObject::NAME.as_ptr(),
                ExtDescriptorIndexingFn::NAME.as_ptr(),
                #[cfg(any(target_os = "macos", target_os = "ios"))]
                KhrPortabilitySubsetFn::NAME.as_ptr(),
                #[cfg(any(target_os = "macos", target_os = "ios"))]
                KhrGetMemoryRequirements2Fn::NAME.as_ptr(),
            ];
            let features = vk::PhysicalDeviceFeatures {
                shader_clip_distance: 1,
                sampler_anisotropy: 1,
                ..Default::default()
            };
            let priorities = [1.0];

            let mut dynamic_rendering_features =
                vk::PhysicalDeviceDynamicRenderingFeatures::default().dynamic_rendering(true);
            let mut synchronization2_features =
                vk::PhysicalDeviceSynchronization2Features::default().synchronization2(true);

            let mut shader_object_features =
                PhysicalDeviceShaderObjectFeaturesEXT::default().shader_object(true);

            let mut buffer_features =
                PhysicalDeviceBufferDeviceAddressFeaturesKHR::default().buffer_device_address(true);

            let mut indexing_features = PhysicalDeviceDescriptorIndexingFeatures::default()
                .descriptor_binding_partially_bound(true)
                .runtime_descriptor_array(true)
                .shader_sampled_image_array_non_uniform_indexing(true)
                // .shader_uniform_buffer_array_non_uniform_indexing(true)
                // .shader_storage_buffer_array_non_uniform_indexing(true)
                // after bind
                .descriptor_binding_sampled_image_update_after_bind(true)
                .descriptor_binding_uniform_buffer_update_after_bind(true)
                .descriptor_binding_storage_buffer_update_after_bind(true)
                // dynamic indexing
                .shader_input_attachment_array_dynamic_indexing(true)
                .shader_storage_texel_buffer_array_dynamic_indexing(true)
                .shader_uniform_texel_buffer_array_dynamic_indexing(true);

            let queue_info = vk::DeviceQueueCreateInfo::default()
                .queue_family_index(queue_family_index)
                .queue_priorities(&priorities);

            let device_create_info = vk::DeviceCreateInfo::default()
                .queue_create_infos(std::slice::from_ref(&queue_info))
                .enabled_extension_names(&device_extension_names_raw)
                .enabled_features(&features)
                .push_next(&mut dynamic_rendering_features)
                .push_next(&mut synchronization2_features)
                // .push_next(&mut vertex_dynamic_state_features)
                .push_next(&mut shader_object_features)
                .push_next(&mut buffer_features)
                .push_next(&mut indexing_features);

            let device: Device = instance
                .create_device(pdevice, &device_create_info, None)
                .unwrap();

            let present_queue = device.get_device_queue(queue_family_index, 0);

            let surface_format = surface_loader
                .get_physical_device_surface_formats(pdevice, surface)
                .unwrap()[0];

            let surface_capabilities = surface_loader
                .get_physical_device_surface_capabilities(pdevice, surface)
                .unwrap();
            let mut desired_image_count = surface_capabilities.min_image_count + 1;
            if surface_capabilities.max_image_count > 0
                && desired_image_count > surface_capabilities.max_image_count
            {
                desired_image_count = surface_capabilities.max_image_count;
            }
            let surface_resolution = match surface_capabilities.current_extent.width {
                // std::u32::MAX => vk::Extent2D {
                //     width: 1280,
                //     height: 720,
                // },
                _ => surface_capabilities.current_extent,
            };
            let pre_transform = if surface_capabilities
                .supported_transforms
                .contains(vk::SurfaceTransformFlagsKHR::IDENTITY)
            {
                vk::SurfaceTransformFlagsKHR::IDENTITY
            } else {
                surface_capabilities.current_transform
            };
            let present_modes = surface_loader
                .get_physical_device_surface_present_modes(pdevice, surface)
                .unwrap();

            let present_mode = match present_mode {
                PresentMode::Fifo => vk::PresentModeKHR::FIFO,
                PresentMode::Immediate => vk::PresentModeKHR::IMMEDIATE,
                PresentMode::Mailbox => vk::PresentModeKHR::MAILBOX,
                PresentMode::AutoNoVsync => vk::PresentModeKHR::IMMEDIATE,
                PresentMode::AutoVsync => vk::PresentModeKHR::FIFO_RELAXED,
            };
            println!("{:?}", present_mode);
            assert!(
                present_modes.contains(&present_mode),
                "Present mode not supported."
            );
            let swapchain_loader = Swapchain::new(&instance, &device);

            let swapchain_create_info = vk::SwapchainCreateInfoKHR::default()
                .surface(surface)
                .min_image_count(desired_image_count)
                .image_color_space(surface_format.color_space)
                .image_format(surface_format.format)
                .image_extent(surface_resolution)
                .image_usage(vk::ImageUsageFlags::COLOR_ATTACHMENT)
                .image_sharing_mode(vk::SharingMode::EXCLUSIVE)
                .pre_transform(pre_transform)
                .composite_alpha(vk::CompositeAlphaFlagsKHR::OPAQUE)
                .present_mode(present_mode)
                .clipped(true)
                .image_array_layers(1);

            let swapchain = swapchain_loader
                .create_swapchain(&swapchain_create_info, None)
                .unwrap();

            let pool_create_info = vk::CommandPoolCreateInfo::default()
                .flags(vk::CommandPoolCreateFlags::RESET_COMMAND_BUFFER)
                .queue_family_index(queue_family_index);

            let pool = device.create_command_pool(&pool_create_info, None).unwrap();

            let command_buffer_allocate_info = vk::CommandBufferAllocateInfo::default()
                .command_buffer_count(2)
                .command_pool(pool)
                .level(vk::CommandBufferLevel::PRIMARY);

            let command_buffers = device
                .allocate_command_buffers(&command_buffer_allocate_info)
                .unwrap();
            let setup_command_buffer = command_buffers[0];
            let draw_command_buffer = command_buffers[1];

            let present_images = swapchain_loader.get_swapchain_images(swapchain).unwrap();
            let present_image_views: Vec<vk::ImageView> = present_images
                .iter()
                .map(|&image| {
                    let create_view_info = vk::ImageViewCreateInfo::default()
                        .view_type(vk::ImageViewType::TYPE_2D)
                        .format(surface_format.format)
                        .components(vk::ComponentMapping {
                            r: vk::ComponentSwizzle::R,
                            g: vk::ComponentSwizzle::G,
                            b: vk::ComponentSwizzle::B,
                            a: vk::ComponentSwizzle::A,
                        })
                        .subresource_range(vk::ImageSubresourceRange {
                            aspect_mask: vk::ImageAspectFlags::COLOR,
                            base_mip_level: 0,
                            level_count: 1,
                            base_array_layer: 0,
                            layer_count: 1,
                        })
                        .image(image);
                    device.create_image_view(&create_view_info, None).unwrap()
                })
                .collect();
            let device_memory_properties = instance.get_physical_device_memory_properties(pdevice);
            let depth_image_create_info = vk::ImageCreateInfo::default()
                .image_type(vk::ImageType::TYPE_2D)
                .format(vk::Format::D16_UNORM)
                .extent(surface_resolution.into())
                .mip_levels(1)
                .array_layers(1)
                .samples(vk::SampleCountFlags::TYPE_1)
                .tiling(vk::ImageTiling::OPTIMAL)
                .usage(vk::ImageUsageFlags::DEPTH_STENCIL_ATTACHMENT)
                .sharing_mode(vk::SharingMode::EXCLUSIVE);

            let depth_image = device.create_image(&depth_image_create_info, None).unwrap();
            let depth_image_memory_req = device.get_image_memory_requirements(depth_image);
            let depth_image_memory_index = find_memorytype_index(
                &depth_image_memory_req,
                &device_memory_properties,
                vk::MemoryPropertyFlags::DEVICE_LOCAL,
            )
            .expect("Unable to find suitable memory index for depth image.");

            let depth_image_allocate_info = vk::MemoryAllocateInfo::default()
                .allocation_size(depth_image_memory_req.size)
                .memory_type_index(depth_image_memory_index);

            let depth_image_memory = device
                .allocate_memory(&depth_image_allocate_info, None)
                .unwrap();

            device
                .bind_image_memory(depth_image, depth_image_memory, 0)
                .expect("Unable to bind depth image memory");

            let fence_create_info =
                vk::FenceCreateInfo::default().flags(vk::FenceCreateFlags::SIGNALED);

            let draw_commands_reuse_fence = device
                .create_fence(&fence_create_info, None)
                .expect("Create fence failed.");
            let setup_commands_reuse_fence = device
                .create_fence(&fence_create_info, None)
                .expect("Create fence failed.");

            record_submit_commandbuffer(
                &device,
                setup_command_buffer,
                setup_commands_reuse_fence,
                present_queue,
                &[],
                &[],
                &[],
                |device, setup_command_buffer| {
                    let layout_transition_barriers = vk::ImageMemoryBarrier::default()
                        .image(depth_image)
                        .dst_access_mask(
                            vk::AccessFlags::DEPTH_STENCIL_ATTACHMENT_READ
                                | vk::AccessFlags::DEPTH_STENCIL_ATTACHMENT_WRITE,
                        )
                        .new_layout(vk::ImageLayout::DEPTH_STENCIL_ATTACHMENT_OPTIMAL)
                        .old_layout(vk::ImageLayout::UNDEFINED)
                        .subresource_range(
                            vk::ImageSubresourceRange::default()
                                .aspect_mask(vk::ImageAspectFlags::DEPTH)
                                .layer_count(1)
                                .level_count(1),
                        );

                    device.cmd_pipeline_barrier(
                        setup_command_buffer,
                        vk::PipelineStageFlags::BOTTOM_OF_PIPE,
                        vk::PipelineStageFlags::LATE_FRAGMENT_TESTS,
                        vk::DependencyFlags::empty(),
                        &[],
                        &[],
                        &[layout_transition_barriers],
                    );
                },
            );

            let depth_image_view_info = vk::ImageViewCreateInfo::default()
                .subresource_range(
                    vk::ImageSubresourceRange::default()
                        .aspect_mask(vk::ImageAspectFlags::DEPTH)
                        .level_count(1)
                        .layer_count(1),
                )
                .image(depth_image)
                .format(depth_image_create_info.format)
                .view_type(vk::ImageViewType::TYPE_2D);

            let depth_image_view = device
                .create_image_view(&depth_image_view_info, None)
                .unwrap();

            let semaphore_create_info = vk::SemaphoreCreateInfo::default();

            let present_complete_semaphore = device
                .create_semaphore(&semaphore_create_info, None)
                .unwrap();
            let rendering_complete_semaphore = device
                .create_semaphore(&semaphore_create_info, None)
                .unwrap();

            let shader_object = ShaderObject::new(&instance, &device);
            let immutable_samplers = Self::create_samplers(&device);
            let (command_thread_pool, threaded_command_buffers) =
                Self::create_command_thread_pool(device.clone(), queue_family_index);

            let synchronization2 = Synchronization2::new(&instance, &device);
            let dynamic_rendering = DynamicRendering::new(&instance, &device);

            ExampleBase {
                entry,
                instance,
                shader_object,
                device,
                synchronization2,
                dynamic_rendering,
                queue_family_index,
                pdevice,
                immutable_samplers,
                command_thread_pool,
                threaded_command_buffers,
                // TODO: fetch from device
                max_descriptor_count: 1024,
                device_memory_properties,
                surface_loader,
                surface_format,
                present_queue,
                surface_resolution,
                swapchain_loader,
                swapchain,
                present_images,
                present_image_views,
                pool,
                draw_command_buffer,
                setup_command_buffer,
                depth_image,
                depth_image_view,
                depth_image_format: depth_image_create_info.format,
                present_complete_semaphore,
                rendering_complete_semaphore,
                draw_commands_reuse_fence,
                setup_commands_reuse_fence,
                surface,
                debug_call_back,
                debug_utils_loader,
                depth_image_memory,
            }
        }
    }

    fn create_samplers(device: &ash::Device) -> HashMap<SamplerDesc, vk::Sampler> {
        let texel_filters = [vk::Filter::NEAREST, vk::Filter::LINEAR];
        let mipmap_modes = [
            vk::SamplerMipmapMode::NEAREST,
            vk::SamplerMipmapMode::LINEAR,
        ];
        let address_modes = [
            vk::SamplerAddressMode::REPEAT,
            vk::SamplerAddressMode::CLAMP_TO_EDGE,
        ];

        let mut result = HashMap::new();

        for &texel_filter in &texel_filters {
            for &mipmap_mode in &mipmap_modes {
                for &address_modes in &address_modes {
                    let anisotropy_enable = texel_filter == vk::Filter::LINEAR;

                    result.insert(
                        SamplerDesc {
                            texel_filter,
                            mipmap_mode,
                            address_modes,
                        },
                        unsafe {
                            device.create_sampler(
                                &vk::SamplerCreateInfo::default()
                                    .mag_filter(texel_filter)
                                    .min_filter(texel_filter)
                                    .mipmap_mode(mipmap_mode)
                                    .address_mode_u(address_modes)
                                    .address_mode_v(address_modes)
                                    .address_mode_w(address_modes)
                                    .max_lod(vk::LOD_CLAMP_NONE)
                                    .max_anisotropy(16.0)
                                    .anisotropy_enable(anisotropy_enable),
                                None,
                            )
                        }
                        .expect("create_sampler"),
                    );
                }
            }
        }

        result
    }

    pub fn create_command_thread_pool(
        device: Device,
        queue_family_index: u32,
    ) -> (ThreadPool, Arc<RwLock<HashMap<usize, CommandBuffer>>>) {
        let m_command_buffers: Arc<RwLock<HashMap<usize, CommandBuffer>>> =
            Arc::new(RwLock::new(HashMap::new()));
        let m_command_buffers_clone = m_command_buffers.clone();

        let pool = rayon::ThreadPoolBuilder::new()
            .thread_name(|x| format!("Command buffer generation thread {}", x))
            .start_handler(move |x| {
                let pool_create_info = vk::CommandPoolCreateInfo::default()
                    .flags(vk::CommandPoolCreateFlags::RESET_COMMAND_BUFFER)
                    .queue_family_index(queue_family_index);

                let pool = unsafe { device.create_command_pool(&pool_create_info, None).unwrap() };

                let command_buffer_allocate_info = vk::CommandBufferAllocateInfo::default()
                    .command_buffer_count(1)
                    .command_pool(pool)
                    .level(vk::CommandBufferLevel::SECONDARY);

                let command_buffers = unsafe {
                    device
                        .allocate_command_buffers(&command_buffer_allocate_info)
                        .unwrap()
                };

                m_command_buffers
                    .write()
                    .unwrap()
                    .insert(x, command_buffers[0]);
            })
            .build()
            .unwrap();

        (pool, m_command_buffers_clone)
    }

    pub fn get_sampler(&self, desc: SamplerDesc) -> vk::Sampler {
        *self
            .immutable_samplers
            .get(&desc)
            .unwrap_or_else(|| panic!("Sampler not found: {:?}", desc))
    }

    pub fn get_default_sampler(&self) -> vk::Sampler {
        self.get_sampler(SamplerDesc {
            texel_filter: vk::Filter::LINEAR,
            mipmap_mode: vk::SamplerMipmapMode::LINEAR,
            address_modes: vk::SamplerAddressMode::REPEAT,
        })
    }

    pub fn copy_buffer_to_texture(&self, buffer: &Buffer, texture: &Image) {
        unsafe {
            record_submit_commandbuffer(
                &self.device,
                self.setup_command_buffer,
                self.setup_commands_reuse_fence,
                self.present_queue,
                &[],
                &[],
                &[],
                |device, setup_command_buffer| {
                    {
                        let image_memory_barrier = vk::ImageMemoryBarrier2::default()
                            .src_stage_mask(vk::PipelineStageFlags2::TRANSFER)
                            .dst_stage_mask(vk::PipelineStageFlags2::TRANSFER)
                            .src_access_mask(vk::AccessFlags2::empty())
                            .dst_access_mask(vk::AccessFlags2::TRANSFER_WRITE)
                            .old_layout(vk::ImageLayout::UNDEFINED)
                            .new_layout(vk::ImageLayout::TRANSFER_DST_OPTIMAL)
                            .image(texture.image)
                            .subresource_range(vk::ImageSubresourceRange {
                                aspect_mask: vk::ImageAspectFlags::COLOR,
                                layer_count: 1,
                                level_count: 1,
                                ..Default::default()
                            });

                        let dependency_info = vk::DependencyInfo::default()
                            .image_memory_barriers(std::slice::from_ref(&image_memory_barrier));

                        self.synchronization2
                            .cmd_pipeline_barrier2(setup_command_buffer, &dependency_info);
                    }

                    // println!(
                    //     "{:?} {:?} {:?}",
                    //     buffer.size,
                    //     texture.extent.width * texture.bytes_per_texel(),
                    //     texture.extent.width
                    // );

                    device.cmd_copy_buffer_to_image(
                        setup_command_buffer,
                        buffer.buffer,
                        texture.image,
                        ImageLayout::TRANSFER_DST_OPTIMAL,
                        &[BufferImageCopy::default()
                            .buffer_offset(0)
                            .buffer_row_length(texture.extent.width)
                            .buffer_image_height(0)
                            .image_subresource(vk::ImageSubresourceLayers {
                                aspect_mask: vk::ImageAspectFlags::COLOR,
                                mip_level: 0,
                                base_array_layer: 0,
                                layer_count: 1,
                            })
                            .image_extent(texture.extent)],
                    );

                    // {

                    // }
                },
            );
        }
    }
}

impl Drop for ExampleBase {
    fn drop(&mut self) {
        unsafe {
            self.device.device_wait_idle().unwrap();

            self.device
                .destroy_semaphore(self.present_complete_semaphore, None);
            self.device
                .destroy_semaphore(self.rendering_complete_semaphore, None);
            self.device
                .destroy_fence(self.draw_commands_reuse_fence, None);
            self.device
                .destroy_fence(self.setup_commands_reuse_fence, None);
            self.device.free_memory(self.depth_image_memory, None);
            self.device.destroy_image_view(self.depth_image_view, None);
            self.device.destroy_image(self.depth_image, None);
            for &image_view in self.present_image_views.iter() {
                self.device.destroy_image_view(image_view, None);
            }
            self.device.destroy_command_pool(self.pool, None);
            self.swapchain_loader
                .destroy_swapchain(self.swapchain, None);
            self.device.destroy_device(None);
            self.surface_loader.destroy_surface(self.surface, None);
            self.debug_utils_loader
                .destroy_debug_utils_messenger(self.debug_call_back, None);
            self.instance.destroy_instance(None);
        }
    }
}
