use std::{
    mem::size_of,
    slice::Iter,
};

use bevy::{
    core::{cast_slice, Pod},
    render::{
        render_resource::*,
        renderer::{RenderDevice, RenderQueue},
    },
};
use thiserror::Error;

pub use controller::*;
pub use input::*;
pub use inventory::*;
pub(crate) use lookup::*;
pub use voxel::*;

mod controller;
mod input;
mod inventory;
mod lookup;
mod voxel;

#[derive(Debug, Error)]
pub enum RonLoaderError {
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error(transparent)]
    RonSpannedError(#[from] ron::error::SpannedError),
    #[error(transparent)]
    LoadDirectError(#[from] bevy::asset::LoadDirectError),
}

pub struct BufVec<T: Pod> {
    read_only: bool,
    buffer_capacity: usize,
    values: Vec<T>,
    staging_buffer: Buffer,
    buffer: Buffer,
}

pub fn create_staging_buffer(read_only: bool, size: usize, device: &RenderDevice) -> Buffer {
    device.create_buffer(&BufferDescriptor {
        label: None,
        size: size as BufferAddress,
        usage: BufferUsages::COPY_DST | if read_only {
            BufferUsages::MAP_READ
        } else {
            BufferUsages::COPY_SRC
        },
        mapped_at_creation: false,
    })
}

pub fn create_buffer(_read_only: bool, size: usize, device: &RenderDevice) -> Buffer {
    // let mut usage = BufferUsages::STORAGE | if read_only {
    //     BufferUsages::COPY_SRC
    // } else {
    //     BufferUsages::COPY_DST
    // };
    let usage = BufferUsages::STORAGE | BufferUsages::COPY_SRC | BufferUsages::COPY_DST;
    device.create_buffer(&BufferDescriptor {
        label: None,
        size: size as BufferAddress,
        usage,
        mapped_at_creation: false,
    })
}

impl<T: Pod> BufVec<T> {
    pub fn with_capacity(read_only: bool, capacity: usize, device: &RenderDevice) -> Self {
        let size = capacity * size_of::<T>();
        let mut buffer = BufVec {
            read_only,
            buffer_capacity: capacity,
            values: Vec::with_capacity(capacity),
            staging_buffer: create_staging_buffer(read_only, size, device),
            buffer: create_buffer(read_only, size, device),
        };
        buffer.ensure_buf_cap(device);
        buffer
    }

    #[inline]
    pub fn buffer(&self) -> &Buffer {
        &self.buffer
    }

    #[inline]
    pub fn capacity(&self) -> usize {
        self.buffer_capacity
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

    fn ensure_buf_cap(&mut self, device: &RenderDevice) {
        if self.values.len() > self.buffer_capacity {
            let size = self.values.len() * size_of::<T>();
            self.staging_buffer = create_staging_buffer(self.read_only, size, device);
            self.buffer = create_buffer(self.read_only, size, device);
            self.buffer_capacity = size;
        }
    }

    pub fn encode_write(&mut self, queue: &RenderQueue, command_encoder: &mut CommandEncoder) {
        if self.values.is_empty() {
            return;
        }

        let size_bytes = size_of::<T>() * self.values.len();
        let range = 0..size_bytes;
        let bytes: &[u8] = cast_slice(&self.values);
        queue.write_buffer(&self.staging_buffer, 0, &bytes[range]);
        command_encoder.copy_buffer_to_buffer(&self.staging_buffer, 0, &self.buffer, 0, size_bytes as BufferAddress);
    }

    pub fn encode_read(&mut self, len: usize, command_encoder: &mut CommandEncoder) {
        let size = size_of::<T>() * len;
        command_encoder.copy_buffer_to_buffer(&self.buffer, 0, &self.staging_buffer, 0, size as BufferAddress);
    }

    pub fn map_buffer(&mut self, len: usize) {
        self.values.resize(len, T::zeroed());
        let buffer_slice = self.staging_buffer.slice(..);
        buffer_slice.map_async(MapMode::Read, |_| {});
    }

    pub fn read_and_unmap_buffer(&mut self, len: usize) {
        self.values.resize(len, T::zeroed());

        let buffer_slice = self.staging_buffer.slice(..);
        let range = 0..size_of::<T>() * len;
        self.values.copy_from_slice(cast_slice(&buffer_slice.get_mapped_range()[range]));
        self.staging_buffer.unmap();
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
