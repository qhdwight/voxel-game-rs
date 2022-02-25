use std::slice::Iter;

use bevy::{
    core::{cast_slice, Pod},
    render::{
        render_resource::*,
        renderer::{RenderDevice, RenderQueue},
    },
};
use wgpu::{BufferUsages, MapMode};

mod voxel;
mod lookup;
mod controller;

pub use lookup::*;
pub use controller::*;
pub(crate) use voxel::*;

pub struct BufVec<T: Pod> {
    values: Vec<T>,
    buffer: Option<Buffer>,
    capacity: usize,
    item_size: usize,
    buffer_usage: BufferUsages,
}

impl<T: Pod> Default for BufVec<T> {
    fn default() -> Self {
        Self {
            values: Vec::new(),
            buffer: None,
            capacity: 0,
            buffer_usage: BufferUsages::all(),
            item_size: std::mem::size_of::<T>(),
        }
    }
}

impl<T: Pod> BufVec<T> {
    pub fn new(buffer_usage: BufferUsages) -> Self {
        Self {
            buffer_usage,
            ..Default::default()
        }
    }

    pub fn with_capacity(buffer_usage: BufferUsages, capacity: usize, device: &RenderDevice) -> Self {
        let mut buffer = BufVec::new(buffer_usage);
        buffer.reserve(capacity, device);
        buffer
    }

    #[inline]
    pub fn buffer(&self) -> Option<&Buffer> {
        self.buffer.as_ref()
    }

    #[inline]
    pub fn capacity(&self) -> usize {
        self.capacity
    }

    #[inline]
    pub fn len(&self) -> usize {
        self.values.len()
    }

    #[inline]
    pub fn is_empty(&self) -> bool {
        self.values.is_empty()
    }

    pub fn push(&mut self, value: T) -> usize {
        let index = self.values.len();
        self.values.push(value);
        index
    }

    pub fn reserve(&mut self, capacity: usize, device: &RenderDevice) {
        if capacity > self.capacity {
            self.capacity = capacity;
            let size = self.item_size * capacity;
            self.buffer = Some(device.create_buffer(&wgpu::BufferDescriptor {
                label: None,
                size: size as wgpu::BufferAddress,
                usage: BufferUsages::COPY_DST | self.buffer_usage,
                mapped_at_creation: false,
            }));
        }
    }

    pub fn write_buffer(&mut self, device: &RenderDevice, queue: &RenderQueue) {
        if self.values.is_empty() {
            return;
        }
        self.reserve(self.values.len(), device);
        if let Some(buffer) = &self.buffer {
            let range = 0..self.item_size * self.values.len();
            let bytes: &[u8] = cast_slice(&self.values);
            queue.write_buffer(buffer, 0, &bytes[range]);
        }
    }

    pub fn read_buffer(&mut self, len: usize, device: &RenderDevice)
    {
        if self.values.is_empty() {
            self.reserve(len, device);
        }
        if let Some(buffer) = &self.buffer {
            let buffer_slice = &buffer.slice(..);
            device.map_buffer(buffer_slice, MapMode::Read);
            let range = 0..self.item_size * len;
            self.values.resize(len, unsafe { std::mem::zeroed() });
            self.values.copy_from_slice(cast_slice(&buffer_slice.get_mapped_range()[range]));
            buffer.unmap();
        }
    }

    pub fn as_slice(&self) -> &[T] {
        self.values.as_slice()
    }

    pub fn iter(&self) -> Iter<'_, T> {
        self.values.iter()
    }

    pub fn clear(&mut self) {
        self.values.clear();
    }
}

pub(crate) fn make_compute_uniform_bind_group_layout_entry(binding: u32) -> BindGroupLayoutEntry {
    BindGroupLayoutEntry {
        binding,
        visibility: ShaderStages::COMPUTE,
        ty: BindingType::Buffer {
            ty: BufferBindingType::Uniform,
            has_dynamic_offset: false,
            min_binding_size: None,
        },
        count: None,
    }
}

pub(crate) fn make_compute_storage_bind_group_layout_entry(binding: u32, read_only: bool) -> BindGroupLayoutEntry {
    BindGroupLayoutEntry {
        binding,
        visibility: ShaderStages::COMPUTE,
        ty: BindingType::Buffer {
            ty: BufferBindingType::Storage { read_only },
            has_dynamic_offset: false,
            min_binding_size: None,
        },
        count: None,
    }
}
