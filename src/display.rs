use std::{array::IntoIter, iter::Cycle};

use crate::{
    address_constants::{self, FIRST_FRAMEBUFFER_START, SECOND_FRAMBUFFER_START},
    memory::Memory,
    Address, SCREEN_SIZE,
};

pub const WIDTH: usize = 480;
pub const HEIGHT: usize = WIDTH / 4 * 3;

pub trait Render {
    fn new(memory: &mut Memory) -> Self;
    fn render(&self, memory: &mut Memory);
}

pub struct MockDisplay {}

impl Render for MockDisplay {
    fn render(&self, _: &mut Memory) {
        // do nothing
    }

    fn new(_: &mut Memory) -> Self {
        Self {}
    }
}

pub struct Display {
    visible_framebuffer: Cycle<IntoIter<Address, 2>>,
    texture: raylib::ffi::Texture,
}

impl Display {
    fn create_image_struct_from_memory(memory: &mut Memory, offset: Address) -> raylib::ffi::Image {
        if offset + address_constants::FRAMEBUFFER_SIZE as Address > Memory::SIZE as Address {
            panic!();
        }
        let address = unsafe { memory.as_mut_pointer().offset(offset as isize) };
        dbg!(address);
        raylib::ffi::Image {
            data: address,
            width: WIDTH as i32,
            height: HEIGHT as i32,
            mipmaps: 1,
            format: raylib::ffi::PixelFormat::PIXELFORMAT_PIXELFORMAT_UNCOMPRESSED_R8G8B8A8 as i32,
        }
    }

    fn invisible_framebuffer(&self) -> Address {
        self.visible_framebuffer.clone().next().unwrap() // should not be able to fail since iterator is a cycle
    }
}

impl Render for Display {
    fn new(memory: &mut Memory) -> Self {
        let image_struct = Self::create_image_struct_from_memory(memory, FIRST_FRAMEBUFFER_START);
        Self {
            visible_framebuffer: [FIRST_FRAMEBUFFER_START, SECOND_FRAMBUFFER_START]
                .into_iter()
                .cycle(),
            texture: unsafe { raylib::ffi::LoadTextureFromImage(image_struct) },
        }
    }

    fn render(&self, memory: &mut Memory) {
        let tint_color = raylib::ffi::Color {
            r: 255,
            g: 255,
            b: 255,
            a: 255,
        };
        let scale = SCREEN_SIZE.height as f32 / HEIGHT as f32;
        unsafe {
            raylib::ffi::UpdateTexture(
                self.texture,
                memory
                    .as_mut_pointer()
                    .offset(FIRST_FRAMEBUFFER_START as isize),
            );
            raylib::ffi::DrawTextureEx(
                self.texture,
                raylib::ffi::Vector2 { x: 0.0, y: 0.0 },
                0.0,
                scale,
                tint_color,
            );
        }
    }
}

impl Drop for Display {
    fn drop(&mut self) {
        unsafe {
            raylib::ffi::UnloadTexture(self.texture);
        }
    }
}
