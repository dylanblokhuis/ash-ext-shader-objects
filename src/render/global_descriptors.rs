use std::collections::{BTreeMap, HashMap};

use ash::vk::{self, ShaderStageFlags};
use bevy::{asset::HandleId, prelude::*};

use super::RenderInstance;

#[derive(Resource)]
pub struct GlobalDescriptorSet {
    // pub set_layouts: Vec<vk::DescriptorSetLayout>,
    // pub descriptor_sets: Vec<vk::DescriptorSet>,
    // set_layout_info: Vec<HashMap<u32, vk::DescriptorType>>,
    pub textures: BTreeMap<Handle<super::image::Image>, crate::buffer::Image>,
    pub buffers: BTreeMap<HandleId, crate::buffer::Buffer>,
    image_infos: HashMap<Handle<super::image::Image>, Vec<vk::DescriptorImageInfo>>,
    buffer_infos: HashMap<HandleId, Vec<vk::DescriptorBufferInfo>>,
}

impl GlobalDescriptorSet {
    /**
     * binding 0: image with sampler
     */
    pub fn new(render_instance: &RenderInstance) -> Self {
        // TODO: Get device maximum
        // const DESCRIPTOR_COUNT: u32 = 1024;
        // let bindings = &[
        //     vk::DescriptorSetLayoutBinding::default()
        //         .binding(0)
        //         .descriptor_count(DESCRIPTOR_COUNT)
        //         .descriptor_type(vk::DescriptorType::COMBINED_IMAGE_SAMPLER)
        //         .stage_flags(ShaderStageFlags::ALL),
        //     // vk::DescriptorSetLayoutBinding::default()
        //     //     .binding(1)
        //     //     .descriptor_count(DESCRIPTOR_COUNT)
        //     //     .descriptor_type(vk::DescriptorType::UNIFORM_BUFFER)
        //     //     .stage_flags(ShaderStageFlags::ALL),
        // ];
        // let set_count = 1;
        // let mut set_layouts: Vec<vk::DescriptorSetLayout> = Vec::with_capacity(set_count as usize);
        // let mut set_layout_info: Vec<HashMap<u32, vk::DescriptorType>> =
        //     Vec::with_capacity(set_count as usize);

        // let binding_flags: Vec<vk::DescriptorBindingFlags> = vec![
        //     vk::DescriptorBindingFlags::PARTIALLY_BOUND
        //         | vk::DescriptorBindingFlags::UPDATE_AFTER_BIND;
        //         // | vk::DescriptorBindingFlags::VARIABLE_DESCRIPTOR_COUNT;
        //     bindings.len()
        // ];

        // let mut binding_flags_create_info =
        //     vk::DescriptorSetLayoutBindingFlagsCreateInfo::default().binding_flags(&binding_flags);

        // let set_layout = unsafe {
        //     render_instance
        //         .device()
        //         .create_descriptor_set_layout(
        //             &vk::DescriptorSetLayoutCreateInfo::default()
        //                 .flags(vk::DescriptorSetLayoutCreateFlags::UPDATE_AFTER_BIND_POOL)
        //                 .bindings(bindings)
        //                 .push_next(&mut binding_flags_create_info),
        //             None,
        //         )
        //         .unwrap()
        // };
        // set_layouts.push(set_layout);
        // set_layout_info.push(
        //     bindings
        //         .iter()
        //         .map(|binding| (binding.binding, binding.descriptor_type))
        //         .collect(),
        // );

        // let mut descriptor_pool_sizes: Vec<vk::DescriptorPoolSize> = Vec::new();
        // for bindings in set_layout_info.iter() {
        //     for ty in bindings.values() {
        //         if let Some(mut dps) = descriptor_pool_sizes.iter_mut().find(|item| item.ty == *ty)
        //         {
        //             dps.descriptor_count += 1;
        //         } else {
        //             descriptor_pool_sizes.push(vk::DescriptorPoolSize {
        //                 ty: *ty,
        //                 descriptor_count: 1,
        //             })
        //         }
        //     }
        // }

        // let descriptor_pool_info: vk::DescriptorPoolCreateInfo<'_> =
        //     vk::DescriptorPoolCreateInfo::default()
        //         .pool_sizes(&descriptor_pool_sizes)
        //         .max_sets(1);

        // let descriptor_pool = unsafe {
        //     render_instance
        //         .device()
        //         .create_descriptor_pool(&descriptor_pool_info, None)
        //         .unwrap()
        // };

        // let desc_alloc_info = vk::DescriptorSetAllocateInfo::default()
        //     .descriptor_pool(descriptor_pool)
        //     .set_layouts(&set_layouts);
        // let descriptor_sets = unsafe {
        //     render_instance
        //         .device()
        //         .allocate_descriptor_sets(&desc_alloc_info)
        //         .unwrap()
        // };

        Self {
            // set_layouts,
            // descriptor_sets,
            // set_layout_info,
            buffers: BTreeMap::new(),
            textures: BTreeMap::new(),
            buffer_infos: HashMap::new(),
            image_infos: HashMap::new(),
        }
    }

    // /// TODO: use a Vec and a hashmap to prevent O(n) lookup
    // pub fn get_buffer_index(&self, key: &HandleId) -> Option<usize> {
    //     self.buffers.iter().position(|(k, _)| k.eq(key))
    // }

    /// TODO: use a Vec and a hashmap to prevent O(n) lookup
    pub fn get_texture_index(&self, key: &Handle<super::image::Image>) -> Option<usize> {
        self.textures.iter().position(|(k, _)| k == key)
    }

    pub fn update_descriptor_set(
        &mut self,
        set: vk::DescriptorSet,
        render_instance: &RenderInstance,
    ) {
        let mut write_desc_sets = vec![];

        for (key, texture) in self.textures.iter_mut() {
            let view = texture.create_view(render_instance.device());

            if !self.image_infos.contains_key(key) {
                self.image_infos.insert(
                    key.clone(),
                    vec![vk::DescriptorImageInfo::default()
                        .image_layout(vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL)
                        .image_view(view)
                        .sampler(render_instance.0.get_default_sampler())],
                );
            }
        }

        // for (key, buffer) in self.buffers.iter_mut() {
        //     if !self.buffer_infos.contains_key(key) {
        //         self.buffer_infos.insert(
        //             *key,
        //             vec![vk::DescriptorBufferInfo::default()
        //                 .buffer(buffer.buffer)
        //                 .offset(0)
        //                 .range(buffer.size)],
        //         );
        //     }
        // }

        for (index, (key, _)) in self.textures.iter_mut().enumerate() {
            write_desc_sets.push(
                vk::WriteDescriptorSet::default()
                    .dst_set(set)
                    .dst_binding(0)
                    .descriptor_type(vk::DescriptorType::COMBINED_IMAGE_SAMPLER)
                    .dst_array_element(index as u32)
                    .image_info(self.image_infos.get(key).unwrap()),
            );
        }

        // for (index, (key, _)) in self.buffers.iter_mut().enumerate() {
        //     write_desc_sets.push(
        //         vk::WriteDescriptorSet::default()
        //             .dst_set(set)
        //             .dst_binding(1)
        //             .descriptor_type(vk::DescriptorType::UNIFORM_BUFFER)
        //             .dst_array_element(index as u32)
        //             .buffer_info(self.buffer_infos.get(key).unwrap()),
        //     );
        // }

        unsafe {
            render_instance
                .device()
                .update_descriptor_sets(&write_desc_sets, &[]);
        };
    }
}
