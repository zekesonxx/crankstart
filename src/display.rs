//! Display-related functions
use crate::{
    geometry::{ScreenPoint, ScreenSize},
    pd_func_caller,
};
use anyhow::Error;
use core::ptr;
use euclid::{default::Vector2D, size2};

/// Global Display handle
#[derive(Clone, Debug)]
pub struct Display(*const crankstart_sys::playdate_display);

impl Display {
    pub(crate) fn new(display: *const crankstart_sys::playdate_display) {
        unsafe {
            DISPLAY = Self(display);
        }
    }

    /// Return a copy of the global Display object to access
    pub fn get() -> Self {
        unsafe { DISPLAY.clone() }
    }

    /// Get the current screen size, taking scale into account
    /// e.g. if the current scale is `2`, this returns `200x120` instead of `400x240`
    /// 
    /// [Playdate SDK Reference](https://sdk.play.date/2.1.1/Inside%20Playdate%20with%20C.html#f-display.getHeight)
    pub fn get_size(&self) -> Result<ScreenSize, Error> {
        Ok(size2(
            pd_func_caller!((*self.0).getWidth)?,
            pd_func_caller!((*self.0).getHeight)?,
        ))
    }

    /// If set to `true`, the frame buffer is drawn inverted—black instead of white, and vice versa.
    /// 
    /// [Playdate SDK Reference](https://sdk.play.date/inside-playdate-with-c/#f-display.setInverted)
    pub fn set_inverted(&self, inverted: bool) -> Result<(), Error> {
        pd_func_caller!((*self.0).setInverted, inverted as i32)
    }

    /// Sets the display scale factor. Valid values for scale are 1, 2, 4, and 8.
    /// The top-left corner of the frame buffer is scaled up to fill the display;
    /// e.g., if the scale is set to 4, the pixels in rectangle [0,100] x [0,60] are drawn on the screen as 4 x 4 squares.
    /// 
    /// [Playdate SDK Reference](https://sdk.play.date/inside-playdate-with-c/#f-display.setScale)
    pub fn set_scale(&self, scale_factor: u32) -> Result<(), Error> {
        debug_assert!(scale_factor == 1 || scale_factor == 2 || scale_factor == 4 || scale_factor == 8, "scale_factor must be 1/2/4/8");
        pd_func_caller!((*self.0).setScale, scale_factor)
    }

    /// Adds a mosaic effect to the display.
    /// Valid `x` and `y` values are between 0 and 3, inclusive.
    /// 
    /// [Playdate SDK Reference](https://sdk.play.date/inside-playdate-with-c/#f-display.setMosaic)
    pub fn set_mosaic(&self, amount: Vector2D<u32>) -> Result<(), Error> {
        debug_assert!(amount.x <= 3 && amount.y <= 3, "valid mosaic x/y values are 0-3 inclusive");
        pd_func_caller!((*self.0).setMosaic, amount.x, amount.y)
    }

    /// Offsets the display by the given amount.
    /// Areas outside of the displayed area are filled with the current background color.
    /// 
    /// [Playdate SDK Reference](https://sdk.play.date/inside-playdate-with-c/#f-display.setOffset)
    pub fn set_offset(&self, offset: ScreenPoint) -> Result<(), Error> {
        pd_func_caller!((*self.0).setOffset, offset.x, offset.y)
    }

    /// Sets the nominal refresh rate in frames per second.
    /// Default is `20` fps, the maximum rate supported by the hardware for full-frame updates.
    /// 
    /// [Playdate SDK Reference](https://sdk.play.date/inside-playdate-with-c/#f-display.setRefreshRate)
    pub fn set_refresh_rate(&self, rate: f32) -> Result<(), Error> {
        pd_func_caller!((*self.0).setRefreshRate, rate)
    }

    /// Flips the display on the x or y axis, or both.
    /// 
    /// [Playdate SDK Reference](https://sdk.play.date/inside-playdate-with-c/#f-display.setFlipped)
    pub fn set_flipped(&self, flip_x: bool, flip_y: bool) -> Result<(), Error> {
        pd_func_caller!((*self.0).setFlipped, flip_x as i32, flip_y as i32)
    }
}

static mut DISPLAY: Display = Display(ptr::null_mut());
