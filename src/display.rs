#[cfg(feature = "graphics")]
use crate::SCREEN_SIZE;
#[cfg(feature = "graphics")]
use raylib::{
    ffi::RenderTexture,
    prelude::{RaylibDraw, RaylibDrawHandle},
    texture::{RaylibTexture2D, RenderTexture2D},
};

use crate::{address_constants, memory::Memory, Address};

pub const WIDTH: usize = 480;
pub const HEIGHT: usize = WIDTH / 4 * 3;

pub trait Display {
    type Handle;
    type Thread;

    fn swap(&mut self);
    fn is_first_framebuffer_visible(&self) -> bool;

    #[cfg(feature = "graphics")]
    fn render(&mut self, memory: &mut Memory, handle: &mut RaylibDrawHandle);

    fn invisible_framebuffer_address(&self) -> Address {
        match self.is_first_framebuffer_visible() {
            true => address_constants::SECOND_FRAMEBUFFER_START,
            false => address_constants::FIRST_FRAMEBUFFER_START,
        }
    }
}

pub struct MockDisplay {
    first_framebuffer_visible: bool,
}

impl MockDisplay {
    pub fn new(_: &mut <Self as Display>::Handle, _: &<Self as Display>::Thread) -> Self {
        Self {
            first_framebuffer_visible: true,
        }
    }
}

impl Display for MockDisplay {
    type Handle = ();
    type Thread = ();

    fn swap(&mut self) {
        self.first_framebuffer_visible = !self.first_framebuffer_visible
    }

    fn is_first_framebuffer_visible(&self) -> bool {
        self.first_framebuffer_visible
    }

    #[cfg(feature = "graphics")]
    fn render(&mut self, _: &mut Memory, _: &mut RaylibDrawHandle) {
        // do nothing
    }
}

pub struct DisplayImplementation {
    first_framebuffer_visible: bool,

    #[cfg(feature = "graphics")]
    texture: RenderTexture2D,
}

#[cfg(feature = "graphics")]
impl DisplayImplementation {
    pub fn new(handle: &mut <Self as Display>::Handle, thread: &<Self as Display>::Thread) -> Self {
        let mut texture = handle
            .load_render_texture(thread, WIDTH as u32, HEIGHT as u32)
            .unwrap();
        let render_texture: &mut RenderTexture = texture.as_mut();
        render_texture.texture.format =
            raylib::ffi::PixelFormat::PIXELFORMAT_PIXELFORMAT_UNCOMPRESSED_R8G8B8A8 as _;
        Self {
            first_framebuffer_visible: true,
            texture,
        }
    }
}

#[cfg(feature = "graphics")]
impl Display for DisplayImplementation {
    type Handle = raylib::RaylibHandle;
    type Thread = raylib::RaylibThread;

    fn render(&mut self, memory: &mut Memory, handle: &mut RaylibDrawHandle) {
        let tint_color = raylib::ffi::Color {
            r: 0xFF,
            g: 0xFF,
            b: 0xFF,
            a: 0xFF,
        };
        let scale = SCREEN_SIZE.height as f32 / HEIGHT as f32;
        let framebuffer_start = match self.is_first_framebuffer_visible() {
            true => address_constants::FIRST_FRAMEBUFFER_START,
            false => address_constants::SECOND_FRAMEBUFFER_START,
        } as usize;
        self.texture.update_texture(
            &memory.data()[framebuffer_start..][..address_constants::FRAMEBUFFER_SIZE],
        );
        handle.draw_texture_ex(
            &self.texture,
            raylib::ffi::Vector2 { x: 0.0, y: 0.0 },
            0.0,
            scale,
            tint_color,
        );
    }

    fn swap(&mut self) {
        self.first_framebuffer_visible = !self.first_framebuffer_visible;
    }

    fn is_first_framebuffer_visible(&self) -> bool {
        self.first_framebuffer_visible
    }
}

#[cfg(not(feature = "graphics"))]
impl DisplayImplementation {
    pub fn new(handle: &mut <Self as Display>::Handle, thread: &<Self as Display>::Thread) -> Self {
        DisplayImplementation {
            first_framebuffer_visible: true,
        }
    }
}

#[cfg(not(feature = "graphics"))]
impl Display for DisplayImplementation {
    type Handle = ();
    type Thread = ();

    fn swap(&mut self) {
        self.first_framebuffer_visible = !self.first_framebuffer_visible;
    }

    fn is_first_framebuffer_visible(&self) -> bool {
        self.first_framebuffer_visible
    }
}
