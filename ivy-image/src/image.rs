use crate::{Error, Result};
use libc::*;
use std::path::Path;

#[link(name = "stb")]
extern "C" {
    fn stbi_load(
        filename: *const c_char,
        x: *mut c_int,
        y: *mut c_int,
        channels: *mut c_int,
        desired_channels: c_int,
    ) -> *mut c_uchar;

    fn stbi_load_from_memory(
        buf: *const c_uchar,
        len: c_int,
        x: *mut c_int,
        y: *mut c_int,
        channels: *mut c_int,
        desired_channels: c_int,
    ) -> *mut c_uchar;
}

pub struct Image {
    width: u32,
    height: u32,
    channels: u32,
    pixels: Box<[u8]>,
}

impl Image {
    pub fn new(width: u32, height: u32, channels: u32, pixels: Box<[u8]>) -> Self {
        Self {
            width: width as _,
            height: height as _,
            channels: channels as _,
            pixels,
        }
    }

    /// Loads an image from a path
    pub fn load<P: AsRef<Path>>(path: P, desired_channels: i32) -> Result<Self> {
        let filename = std::ffi::CString::new(path.as_ref().as_os_str().to_str().unwrap())
            .ok()
            .unwrap();

        let mut width: c_int = 0;
        let mut height: c_int = 0;
        let mut channels: c_int = desired_channels;

        let pixels_raw = unsafe {
            stbi_load(
                filename.as_ptr(),
                &mut width,
                &mut height,
                &mut channels,
                desired_channels,
            )
        };

        if pixels_raw.is_null() {
            return Err(Error::FileLoading(path.as_ref().to_owned()));
        }

        // Desired channels override channels
        if desired_channels != 0 {
            channels = desired_channels;
        }

        let image_size = width as usize * height as usize * channels as usize;
        let pixels = unsafe { Vec::from_raw_parts(pixels_raw, image_size, image_size) };
        let pixels = pixels.into_boxed_slice();

        Ok(Image::new(width as _, height as _, channels as _, pixels))
    }

    /// Loads an image from memory, such as a memory mapped file
    pub fn load_from_memory(buf: &[u8], desired_channels: i32) -> Option<Self> {
        let mut width: c_int = 0;
        let mut height: c_int = 0;
        let mut channels: c_int = desired_channels;
        // let desired_channels: c_int = 0;

        let pixels_raw = unsafe {
            stbi_load_from_memory(
                buf.as_ptr(),
                buf.len() as i32,
                &mut width,
                &mut height,
                &mut channels,
                desired_channels,
            )
        };

        if pixels_raw.is_null() {
            return None;
        }

        // Desired channels override channels
        if desired_channels != 0 {
            channels = desired_channels;
        }

        let image_size = width as usize * height as usize * channels as usize;
        let pixels = unsafe { Vec::from_raw_parts(pixels_raw, image_size, image_size) };
        let pixels = pixels.into_boxed_slice();

        Some(Image::new(width as _, height as _, channels as _, pixels))
    }

    pub fn width(&self) -> u32 {
        self.width
    }

    pub fn height(&self) -> u32 {
        self.height
    }

    pub fn channels(&self) -> u32 {
        self.channels
    }

    pub fn pixels(&self) -> &[u8] {
        &self.pixels
    }

    pub fn pixels_mut(&mut self) -> &mut [u8] {
        &mut self.pixels
    }
}
