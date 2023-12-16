use {
    crate::{
        geometry::{ScreenPoint, ScreenRect, ScreenSize, ScreenVector},
        log_to_console, pd_func_caller, pd_func_caller_log,
        system::System,
    },
    alloc::{format, rc::Rc, vec::Vec},
    anyhow::{anyhow, ensure, Error},
    core::{cell::RefCell, ops::RangeInclusive, ptr, slice},
    crankstart_sys::{ctypes::c_int, LCDBitmapTable, LCDPattern},
    cstr_core::{CStr, CString},
    euclid::default::{Point2D, Vector2D},
    hashbrown::HashMap,
};

pub use crankstart_sys::{
    LCDBitmapDrawMode, LCDBitmapFlip, LCDLineCapStyle, LCDPolygonFillRule, LCDRect, LCDSolidColor,
    PDRect, PDStringEncoding, LCD_COLUMNS, LCD_ROWS, LCD_ROWSIZE,
};

pub fn rect_make(x: f32, y: f32, width: f32, height: f32) -> PDRect {
    PDRect {
        x,
        y,
        width,
        height,
    }
}

#[derive(Clone, Debug)]
pub enum LCDColor {
    Solid(LCDSolidColor),
    Pattern(LCDPattern),
}

impl From<LCDColor> for usize {
    fn from(color: LCDColor) -> Self {
        match color {
            LCDColor::Solid(solid_color) => solid_color as usize,
            LCDColor::Pattern(pattern) => {
                let pattern_ptr = &pattern as *const u8;
                pattern_ptr as usize
            }
        }
    }
}

#[derive(Debug)]
pub struct BitmapData {
    pub width: c_int,
    pub height: c_int,
    pub rowbytes: c_int,
    pub hasmask: bool,
}

#[derive(Debug)]
pub struct BitmapInner {
    pub(crate) raw_bitmap: *mut crankstart_sys::LCDBitmap,
    owned: bool,
}

impl BitmapInner {
    pub fn get_data(&self) -> Result<BitmapData, Error> {
        let mut width = 0;
        let mut height = 0;
        let mut rowbytes = 0;
        let mut mask_ptr = ptr::null_mut();
        pd_func_caller!(
            (*Graphics::get_ptr()).getBitmapData,
            self.raw_bitmap,
            &mut width,
            &mut height,
            &mut rowbytes,
            &mut mask_ptr,
            ptr::null_mut(),
        )?;
        Ok(BitmapData {
            width,
            height,
            rowbytes,
            hasmask: !mask_ptr.is_null(),
        })
    }

    pub fn draw(&self, location: ScreenPoint, flip: LCDBitmapFlip) -> Result<(), Error> {
        pd_func_caller!(
            (*Graphics::get_ptr()).drawBitmap,
            self.raw_bitmap,
            location.x,
            location.y,
            flip,
        )?;
        Ok(())
    }

    pub fn draw_scaled(&self, location: ScreenPoint, scale: Vector2D<f32>) -> Result<(), Error> {
        pd_func_caller!(
            (*Graphics::get_ptr()).drawScaledBitmap,
            self.raw_bitmap,
            location.x,
            location.y,
            scale.x,
            scale.y,
        )
    }

    pub fn draw_rotated(
        &self,
        location: ScreenPoint,
        degrees: f32,
        center: Vector2D<f32>,
        scale: Vector2D<f32>,
    ) -> Result<(), Error> {
        pd_func_caller!(
            (*Graphics::get_ptr()).drawRotatedBitmap,
            self.raw_bitmap,
            location.x,
            location.y,
            degrees,
            center.x,
            center.y,
            scale.x,
            scale.y,
        )
    }

    pub fn rotated(&self, degrees: f32, scale: Vector2D<f32>) -> Result<Self, Error> {
        let raw_bitmap = pd_func_caller!(
            (*Graphics::get_ptr()).rotatedBitmap,
            self.raw_bitmap,
            degrees,
            scale.x,
            scale.y,
            // No documentation on this anywhere, but null works in testing.
            ptr::null_mut(), // allocedSize
        )?;
        Ok(Self {
            raw_bitmap,
            owned: true,
        })
    }

    pub fn tile(
        &self,
        location: ScreenPoint,
        size: ScreenSize,
        flip: LCDBitmapFlip,
    ) -> Result<(), Error> {
        pd_func_caller!(
            (*Graphics::get_ptr()).tileBitmap,
            self.raw_bitmap,
            location.x,
            location.y,
            size.width,
            size.height,
            flip,
        )?;
        Ok(())
    }

    pub fn clear(&self, color: LCDColor) -> Result<(), Error> {
        pd_func_caller!(
            (*Graphics::get_ptr()).clearBitmap,
            self.raw_bitmap,
            color.into()
        )
    }

    pub fn duplicate(&self) -> Result<Self, Error> {
        let raw_bitmap = pd_func_caller!((*Graphics::get_ptr()).copyBitmap, self.raw_bitmap)?;

        Ok(Self {
            raw_bitmap,
            owned: self.owned,
        })
    }

    pub fn transform(&self, rotation: f32, scale: Vector2D<f32>) -> Result<Self, Error> {
        // let raw_bitmap = pd_func_caller!(
        //     (*Graphics::get_ptr()).transformedBitmap,
        //     self.raw_bitmap,
        //     rotation,
        //     scale.x,
        //     scale.y,
        //     core::ptr::null_mut(),
        // )?;
        // Ok(Self { raw_bitmap })
        todo!();
    }

    pub fn into_color(&self, bitmap: Bitmap, top_left: Point2D<i32>) -> Result<LCDColor, Error> {
        let mut pattern = LCDPattern::default();
        let pattern_ptr = pattern.as_mut_ptr();
        let mut pattern_val = pattern_ptr as usize;
        let graphics = Graphics::get();
        pd_func_caller!(
            (*graphics.0).setColorToPattern,
            &mut pattern_val,
            self.raw_bitmap,
            top_left.x,
            top_left.y
        )?;
        Ok(LCDColor::Pattern(pattern))
    }

    pub fn load(&self, path: &str) -> Result<(), Error> {
        let c_path = CString::new(path).map_err(Error::msg)?;
        let mut out_err: *const crankstart_sys::ctypes::c_char = ptr::null_mut();
        let graphics = Graphics::get();
        pd_func_caller!(
            (*graphics.0).loadIntoBitmap,
            c_path.as_ptr(),
            self.raw_bitmap,
            &mut out_err
        )?;
        if !out_err.is_null() {
            let err_msg = unsafe { CStr::from_ptr(out_err).to_string_lossy().into_owned() };
            Err(anyhow!(err_msg))
        } else {
            Ok(())
        }
    }

    pub fn check_mask_collision(
        &self,
        my_location: ScreenPoint,
        my_flip: LCDBitmapFlip,
        other: Bitmap,
        other_location: ScreenPoint,
        other_flip: LCDBitmapFlip,
        rect: ScreenRect,
    ) -> Result<bool, Error> {
        let graphics = Graphics::get();
        let other_raw = other.inner.borrow().raw_bitmap;
        let lcd_rect: LCDRect = rect.to_untyped().into();
        let pixels_covered = pd_func_caller!(
            (*graphics.0).checkMaskCollision,
            self.raw_bitmap,
            my_location.x,
            my_location.y,
            my_flip,
            other_raw,
            other_location.x,
            other_location.y,
            other_flip,
            lcd_rect,
        )?;
        Ok(pixels_covered != 0)
    }
}

impl Drop for BitmapInner {
    fn drop(&mut self) {
        if self.owned {
            pd_func_caller_log!((*Graphics::get_ptr()).freeBitmap, self.raw_bitmap);
        }
    }
}

pub type BitmapInnerPtr = Rc<RefCell<BitmapInner>>;

#[derive(Clone, Debug)]
pub struct Bitmap {
    pub(crate) inner: BitmapInnerPtr,
}

impl Bitmap {
    fn new(raw_bitmap: *mut crankstart_sys::LCDBitmap, owned: bool) -> Self {
        Bitmap {
            inner: Rc::new(RefCell::new(BitmapInner { raw_bitmap, owned })),
        }
    }

    pub fn get_data(&self) -> Result<BitmapData, Error> {
        self.inner.borrow().get_data()
    }

    pub fn draw(&self, location: ScreenPoint, flip: LCDBitmapFlip) -> Result<(), Error> {
        self.inner.borrow().draw(location, flip)
    }

    pub fn draw_scaled(&self, location: ScreenPoint, scale: Vector2D<f32>) -> Result<(), Error> {
        self.inner.borrow().draw_scaled(location, scale)
    }

    /// Draw the `Bitmap` to the given `location`, rotated `degrees` about the `center` point,
    /// scaled up or down in size by `scale`.  `center` is given by two numbers between 0.0 and
    /// 1.0, where (0, 0) is the top left and (0.5, 0.5) is the center point.
    pub fn draw_rotated(
        &self,
        location: ScreenPoint,
        degrees: f32,
        center: Vector2D<f32>,
        scale: Vector2D<f32>,
    ) -> Result<(), Error> {
        self.inner
            .borrow()
            .draw_rotated(location, degrees, center, scale)
    }

    /// Return a copy of self, rotated by `degrees` and scaled up or down in size by `scale`.
    pub fn rotated(&self, degrees: f32, scale: Vector2D<f32>) -> Result<Bitmap, Error> {
        let raw_bitmap = self.inner.borrow().rotated(degrees, scale)?;
        Ok(Self {
            inner: Rc::new(RefCell::new(raw_bitmap)),
        })
    }

    pub fn tile(
        &self,
        location: ScreenPoint,
        size: ScreenSize,
        flip: LCDBitmapFlip,
    ) -> Result<(), Error> {
        self.inner.borrow().tile(location, size, flip)
    }

    pub fn clear(&self, color: LCDColor) -> Result<(), Error> {
        self.inner.borrow().clear(color)
    }

    pub fn transform(&self, rotation: f32, scale: Vector2D<f32>) -> Result<Bitmap, Error> {
        let inner = self.inner.borrow().transform(rotation, scale)?;
        Ok(Self {
            inner: Rc::new(RefCell::new(inner)),
        })
    }

    pub fn into_color(&self, bitmap: Bitmap, top_left: Point2D<i32>) -> Result<LCDColor, Error> {
        self.inner.borrow().into_color(bitmap, top_left)
    }

    pub fn load(&self, path: &str) -> Result<(), Error> {
        self.inner.borrow().load(path)
    }

    pub fn check_mask_collision(
        &self,
        my_location: ScreenPoint,
        my_flip: LCDBitmapFlip,
        other: Bitmap,
        other_location: ScreenPoint,
        other_flip: LCDBitmapFlip,
        rect: ScreenRect,
    ) -> Result<bool, Error> {
        self.inner.borrow().check_mask_collision(
            my_location,
            my_flip,
            other,
            other_location,
            other_flip,
            rect,
        )
    }
}

type OptionalBitmap<'a> = Option<&'a mut Bitmap>;

fn raw_bitmap(bitmap: OptionalBitmap<'_>) -> *mut crankstart_sys::LCDBitmap {
    if let Some(bitmap) = bitmap {
        bitmap.inner.borrow().raw_bitmap
    } else {
        ptr::null_mut()
    }
}

pub struct Font(*mut crankstart_sys::LCDFont);

impl Font {
    pub fn new(font: *mut crankstart_sys::LCDFont) -> Result<Self, Error> {
        anyhow::ensure!(!font.is_null(), "Null pointer passed to Font::new");
        Ok(Self(font))
    }
}

impl Drop for Font {
    fn drop(&mut self) {
        // the C API is currently missing a freeFont command
        // but all it'd be is calling realloc(font, 0)
        // https://devforum.play.date/t/how-do-i-release-font-in-c-api/12190/2
        unsafe {
            System::get().realloc(self.0 as *mut core::ffi::c_void, 0);
        }
    }
}

#[derive(Debug)]
struct BitmapTableInner {
    raw_bitmap_table: *mut LCDBitmapTable,
    bitmaps: HashMap<usize, Bitmap>,
}

impl BitmapTableInner {
    fn get_bitmap(&mut self, index: usize) -> Result<Bitmap, Error> {
        if let Some(bitmap) = self.bitmaps.get(&index) {
            Ok(bitmap.clone())
        } else {
            let raw_bitmap = pd_func_caller!(
                (*Graphics::get_ptr()).getTableBitmap,
                self.raw_bitmap_table,
                index as c_int
            )?;
            ensure!(
                !raw_bitmap.is_null(),
                "Failed to load bitmap {} from table {:?}",
                index,
                self.raw_bitmap_table
            );
            let bitmap = Bitmap::new(raw_bitmap, true);
            self.bitmaps.insert(index, bitmap.clone());
            Ok(bitmap)
        }
    }

    fn load(&mut self, path: &str) -> Result<(), Error> {
        let c_path = CString::new(path).map_err(Error::msg)?;
        let mut out_err: *const crankstart_sys::ctypes::c_char = ptr::null_mut();
        let graphics = Graphics::get();
        pd_func_caller!(
            (*graphics.0).loadIntoBitmapTable,
            c_path.as_ptr(),
            self.raw_bitmap_table,
            &mut out_err
        )?;
        if !out_err.is_null() {
            let err_msg = unsafe { CStr::from_ptr(out_err).to_string_lossy().into_owned() };
            Err(anyhow!(err_msg))
        } else {
            Ok(())
        }
    }
}

impl Drop for BitmapTableInner {
    fn drop(&mut self) {
        pd_func_caller_log!(
            (*Graphics::get_ptr()).freeBitmapTable,
            self.raw_bitmap_table
        );
    }
}

type BitmapTableInnerPtr = Rc<RefCell<BitmapTableInner>>;

/// An array of [Bitmap]s of equal dimensions
#[derive(Clone, Debug)]
pub struct BitmapTable {
    inner: BitmapTableInnerPtr,
}

impl BitmapTable {
    pub fn new(raw_bitmap_table: *mut LCDBitmapTable) -> Self {
        Self {
            inner: Rc::new(RefCell::new(BitmapTableInner {
                raw_bitmap_table,
                bitmaps: HashMap::new(),
            })),
        }
    }

    /// Loads the imagetable at `path` into the existing table.
    /// 
    /// [Playdate SDK Reference](https://sdk.play.date/inside-playdate-with-c/#f-graphics.loadIntoBitmapTable)
    pub fn load(&self, path: &str) -> Result<(), Error> {
        self.inner.borrow_mut().load(path)
    }

    /// Get the [Bitmap] stored in the table at `index`.
    /// 
    /// [Playdate SDK Reference](https://sdk.play.date/inside-playdate-with-c/#f-graphics.getTableBitmap)
    pub fn get_bitmap(&self, index: usize) -> Result<Bitmap, Error> {
        self.inner.borrow_mut().get_bitmap(index)
    }
}

#[repr(u32)]
#[derive(Debug, Copy, Clone, Hash, PartialEq, Eq)]
/// Drawing mode to set on images
///
/// The draw mode applies to images and fonts (which are technically images).
/// The draw mode does not apply to primitive shapes such as lines or rectangles. 
///
/// [Playdate SDK Reference](https://sdk.play.date/2.1.1/Inside%20Playdate%20with%20C.html#f-graphics.setDrawMode)
///
/// [Playdate Lua SDK Reference (with example images)](https://sdk.play.date/2.1.1/Inside%20Playdate.html#f-graphics.setImageDrawMode)
pub enum BitmapDrawMode {
    /// Images are drawn exactly as they are
    /// (black pixels are drawn black and white pixels are drawn white)
    Copy = LCDBitmapDrawMode::kDrawModeCopy as u32,
    /// Any white portions of an image are drawn transparent
    /// (black pixels are drawn black and white pixels are drawn transparent)
    WhiteTransparent = LCDBitmapDrawMode::kDrawModeWhiteTransparent as u32,
    /// Any black portions of an image are drawn transparent
    /// (black pixels are drawn transparent and white pixels are drawn white)
    BlackTransparent = LCDBitmapDrawMode::kDrawModeBlackTransparent as u32,
    /// All non-transparent pixels are drawn white
    /// (black pixels are drawn white and white pixels are drawn white)
    FillWhite = LCDBitmapDrawMode::kDrawModeFillWhite as u32,
    /// All non-transparent pixels are drawn black
    /// (black pixels are drawn black and white pixels are drawn black)
    FillBlack = LCDBitmapDrawMode::kDrawModeFillBlack as u32,
    /// Pixels are drawn inverted on white backgrounds, creating an effect where
    /// any white pixels in the original image will always be visible, regardless
    /// of the background color, and any black pixels will appear transparent
    /// (on a white background, black pixels are drawn white and white pixels are drawn black)
    XOR = LCDBitmapDrawMode::kDrawModeXOR as u32,
    /// Pixels are drawn inverted on black backgrounds, creating an effect where
    /// any black pixels in the original image will always be visible, regardless
    /// of the background color, and any white pixels will appear transparent
    /// (on a black background, black pixels are drawn white and white pixels are drawn black)
    NXOR = LCDBitmapDrawMode::kDrawModeNXOR as u32,
    /// Pixels are drawn inverted
    /// (black pixels are drawn white and white pixels are drawn black)
    Inverted = LCDBitmapDrawMode::kDrawModeInverted as u32,
}

impl From<BitmapDrawMode> for LCDBitmapDrawMode {
    fn from(value: BitmapDrawMode) -> Self {
        // safety: the only possible values are the ones defined above
        // which are all `LCDBitmapDrawMode`s to begin with
        unsafe { core::mem::transmute(value) }
    }
}

static mut GRAPHICS: Graphics = Graphics(ptr::null_mut());

#[derive(Clone, Debug)]
pub struct Graphics(*const crankstart_sys::playdate_graphics);

impl Graphics {
    pub(crate) fn new(graphics: *const crankstart_sys::playdate_graphics) {
        unsafe {
            GRAPHICS = Self(graphics);
        }
    }

    pub fn get() -> Self {
        unsafe { GRAPHICS.clone() }
    }

    pub fn get_ptr() -> *const crankstart_sys::playdate_graphics {
        Self::get().0
    }

    /// Allows drawing directly into an image rather than the framebuffer, for example for
    /// drawing text into a sprite's image.
    pub fn with_context<F, T>(&self, bitmap: &Bitmap, f: F) -> Result<T, Error>
    where
        F: FnOnce() -> Result<T, Error>,
    {
        // Any calls in this context are directly modifying the bitmap, so borrow mutably
        // for safety.
        self.push_context(bitmap.inner.borrow_mut().raw_bitmap)?;
        let res = f();
        self.pop_context()?;
        res
    }

    /// Internal function; use `with_context`.
    fn push_context(&self, raw_bitmap: *mut crankstart_sys::LCDBitmap) -> Result<(), Error> {
        pd_func_caller!((*self.0).pushContext, raw_bitmap)
    }

    /// Internal function; use `with_context`.
    fn pop_context(&self) -> Result<(), Error> {
        pd_func_caller!((*self.0).popContext)
    }

    /// Returns the current display frame buffer.
    /// 
    /// Rows are 32-bit aligned, so the row stride is 52 bytes, with the extra 2 bytes per row ignored.
    /// Bytes are MSB-ordered; i.e., the pixel in column 0 is the 0x80 bit of the first byte of the row.
    /// 
    /// [Playdate SDK Reference](https://sdk.play.date/inside-playdate-with-c/#f-graphics.getFrame)
    pub fn get_frame(&self) -> Result<&'static mut [u8], Error> {
        let ptr = pd_func_caller!((*self.0).getFrame)?;
        anyhow::ensure!(!ptr.is_null(), "Null pointer returned from getFrame");
        let frame = unsafe { slice::from_raw_parts_mut(ptr, (LCD_ROWSIZE * LCD_ROWS) as usize) };
        Ok(frame)
    }

    /// Returns the raw bits in the display buffer, the last completed frame.
    /// 
    /// [Playdate SDK Reference](https://sdk.play.date/inside-playdate-with-c/#f-graphics.getDisplayFrame)
    pub fn get_display_frame(&self) -> Result<&'static mut [u8], Error> {
        let ptr = pd_func_caller!((*self.0).getDisplayFrame)?;
        anyhow::ensure!(!ptr.is_null(), "Null pointer returned from getDisplayFrame");
        let frame = unsafe { slice::from_raw_parts_mut(ptr, (LCD_ROWSIZE * LCD_ROWS) as usize) };
        Ok(frame)
    }

    /// Only valid in the Simulator, returns the debug framebuffer as a bitmap.
    /// 
    /// Function will error on device.
    /// 
    /// [Playdate SDK Reference](https://sdk.play.date/inside-playdate-with-c/#f-graphics.getDebugBitmap)
    pub fn get_debug_bitmap(&self) -> Result<Bitmap, Error> {
        let raw_bitmap = pd_func_caller!((*self.0).getDebugBitmap)?;
        anyhow::ensure!(
            !raw_bitmap.is_null(),
            "Null pointer returned from getDebugImage"
        );
        Ok(Bitmap::new(raw_bitmap, false))
    }

    /// Returns a copy the contents of the working frame buffer as a bitmap.
    /// 
    /// [Playdate SDK Reference](https://sdk.play.date/2.1.1/Inside%20Playdate%20with%20C.html#f-graphics.copyFrameBufferBitmap)
    pub fn get_framebuffer_bitmap(&self) -> Result<Bitmap, Error> {
        let raw_bitmap = pd_func_caller!((*self.0).copyFrameBufferBitmap)?;
        anyhow::ensure!(
            !raw_bitmap.is_null(),
            "Null pointer returned from copyFrameBufferBitmap"
        );
        Ok(Bitmap::new(raw_bitmap, true))
    }

    /// Returns a bitmap containing the contents of the display buffer.
    /// 
    /// This is the active display buffer bitmap, not a copy.
    /// For a copy you can do what you want with, see [Graphics::get_framebuffer_bitmap].
    /// 
    /// [Playdate SDK Reference](https://sdk.play.date/2.1.1/Inside%20Playdate%20with%20C.html#f-graphics.getDisplayBufferBitmap)
    pub fn get_display_buffer_bitmap(&self) -> Result<Bitmap, Error> {
        let raw_bitmap = pd_func_caller!((*self.0).getDisplayBufferBitmap)?;
        anyhow::ensure!(
            !raw_bitmap.is_null(),
            "Null pointer returned from getDisplayBufferBitmap"
        );
        Ok(Bitmap::new(raw_bitmap, false))
    }

    /// Sets the background color shown when the display is [offset][crate::Display::set_offset] or for clearing dirty areas in the sprite system.
    /// 
    /// [Playdate SDK Reference](https://sdk.play.date/2.1.1/Inside%20Playdate%20with%20C.html#f-graphics.setBackgroundColor)
    pub fn set_background_color(&self, color: LCDSolidColor) -> Result<(), Error> {
        pd_func_caller!((*self.0).setBackgroundColor, color)
    }

    /// Sets the mode used for drawing bitmaps.
    /// Note that text drawing uses bitmaps, so this affects how fonts are displayed as well.
    ///
    /// [Playdate SDK Reference](https://sdk.play.date/2.1.1/Inside%20Playdate%20with%20C.html#f-graphics.setDrawMode)
    ///
    /// [Playdate Lua SDK Reference (with example images)](https://sdk.play.date/2.1.1/Inside%20Playdate.html#f-graphics.setImageDrawMode)
    pub fn set_draw_mode(&self, mode: BitmapDrawMode) -> Result<(), Error> {
        pd_func_caller!((*self.0).setDrawMode, mode.into())
    }

    /// After updating pixels in the buffer returned by getFrame(), you must tell the graphics system
    /// which rows were updated. This function marks a contiguous range of rows as updated
    /// (e.g., markUpdatedRows(0,LCD_ROWS-1) tells the system to update the entire display).
    /// Both “start” and “end” are included in the range.
    /// 
    /// [Playdate SDK Reference](https://sdk.play.date/inside-playdate-with-c/#f-graphics.markUpdatedRows)
    pub fn mark_updated_rows(&self, range: RangeInclusive<i32>) -> Result<(), Error> {
        let (start, end) = range.into_inner();
        pd_func_caller!((*self.0).markUpdatedRows, start, end)
    }

    /// Manually flushes the current frame buffer out to the display.
    /// 
    /// This function is automatically called after each pass through the run loop,
    /// so there shouldn’t be any need to call it yourself.
    /// 
    /// [Playdate SDK Reference](https://sdk.play.date/inside-playdate-with-c/#f-graphics.display)
    pub fn display(&self) -> Result<(), Error> {
        pd_func_caller!((*self.0).display)
    }

    /// Offsets the origin point for all drawing calls to x, y (can be negative).
    /// 
    /// This is useful, for example, for centering a "camera" on a sprite that is
    /// moving around a world larger than the screen.
    /// 
    /// [Playdate SDK Reference](https://sdk.play.date/inside-playdate-with-c/#f-graphics.setDrawOffset)
    pub fn set_draw_offset(&self, offset: ScreenVector) -> Result<(), Error> {
        pd_func_caller!((*self.0).setDrawOffset, offset.x, offset.y)
    }

    /// Allocates and returns a new [Bitmap] of [`size`][ScreenSize] dimensions filled with `bg_color`.
    /// 
    /// [Playdate SDK Reference](https://sdk.play.date/inside-playdate-with-c/#f-graphics.newBitmap)
    pub fn new_bitmap(&self, size: ScreenSize, bg_color: LCDColor) -> Result<Bitmap, Error> {
        let raw_bitmap = pd_func_caller!(
            (*self.0).newBitmap,
            size.width,
            size.height,
            bg_color.into()
        )?;
        anyhow::ensure!(
            !raw_bitmap.is_null(),
            "Null pointer returned from new_bitmap"
        );
        Ok(Bitmap::new(raw_bitmap, true))
    }

    /// Allocates and returns a new [Bitmap] from the file at `path`.
    /// If there is no file at path, the function will error.
    /// 
    /// [Playdate SDK Reference](https://sdk.play.date/inside-playdate-with-c/#f-graphics.newBitmap)
    pub fn load_bitmap(&self, path: &str) -> Result<Bitmap, Error> {
        let c_path = CString::new(path).map_err(Error::msg)?;
        let mut out_err: *const crankstart_sys::ctypes::c_char = ptr::null_mut();
        let raw_bitmap = pd_func_caller!((*self.0).loadBitmap, c_path.as_ptr(), &mut out_err)?;
        if raw_bitmap.is_null() {
            if !out_err.is_null() {
                let err_msg = unsafe { CStr::from_ptr(out_err).to_string_lossy().into_owned() };
                Err(anyhow!(err_msg))
            } else {
                Err(anyhow!(
                    "load_bitmap failed without providing an error message"
                ))
            }
        } else {
            Ok(Bitmap::new(raw_bitmap, true))
        }
    }

    /// Allocates and returns a new [BitmapTable] that can hold `count` [Bitmap]s of size `size`.
    /// 
    /// [Playdate SDK Reference](https://sdk.play.date/inside-playdate-with-c/#f-graphics.newBitmapTable)
    pub fn new_bitmap_table(&self, count: usize, size: ScreenSize) -> Result<BitmapTable, Error> {
        let raw_bitmap_table = pd_func_caller!(
            (*self.0).newBitmapTable,
            count as i32,
            size.width,
            size.height
        )?;

        Ok(BitmapTable::new(raw_bitmap_table))
    }

    /// Allocates and returns a new [BitmapTable] from the file at `path`.
    /// If there is no file at path, the function will error.
    /// 
    /// [Playdate SDK Reference](https://sdk.play.date/inside-playdate-with-c/#f-graphics.loadBitmapTable)
    pub fn load_bitmap_table(&self, path: &str) -> Result<BitmapTable, Error> {
        let c_path = CString::new(path).map_err(Error::msg)?;
        let mut out_err: *const crankstart_sys::ctypes::c_char = ptr::null_mut();
        let raw_bitmap_table =
            pd_func_caller!((*self.0).loadBitmapTable, c_path.as_ptr(), &mut out_err)?;
        if raw_bitmap_table.is_null() {
            if !out_err.is_null() {
                let err_msg = unsafe { CStr::from_ptr(out_err).to_string_lossy().into_owned() };
                Err(anyhow!(err_msg))
            } else {
                Err(anyhow!(
                    "load_bitmap_table failed without providing an error message"
                ))
            }
        } else {
            Ok(BitmapTable::new(raw_bitmap_table))
        }
    }

    /// Clears the entire display, filling it with [`color`][LCDColor].
    ///
    /// [Playdate SDK Reference](https://sdk.play.date/inside-playdate-with-c/#f-graphics.clear)
    pub fn clear(&self, color: LCDColor) -> Result<(), Error> {
        pd_func_caller!((*self.0).clear, color.into())
    }

    /// Draws a line from `p1` to `p2` with a stroke width of `width` and the provided [`color`][LCDColor].
    /// 
    /// [Playdate SDK Reference](https://sdk.play.date/inside-playdate-with-c/#f-graphics.drawLine)
    pub fn draw_line(
        &self,
        p1: ScreenPoint,
        p2: ScreenPoint,
        width: i32,
        color: LCDColor,
    ) -> Result<(), Error> {
        pd_func_caller!(
            (*self.0).drawLine,
            p1.x,
            p1.y,
            p2.x,
            p2.y,
            width,
            color.into(),
        )
    }

    /// Fills the polygon with vertices at the given coordinates using the given color and fill, or winding, rule.
    /// 
    /// [Wikipedia: Nonzero-rule](https://en.wikipedia.org/wiki/Nonzero-rule)
    /// 
    /// [Playdate SDK Reference](https://sdk.play.date/2.1.1/Inside%20Playdate%20with%20C.html#f-graphics.fillPolygon)
    pub fn fill_polygon(
        &self,
        coords: &[ScreenPoint],
        color: LCDColor,
        fillrule: LCDPolygonFillRule,
    ) -> Result<(), Error> {
        let n_pts = coords.len();
        let mut coords_seq = coords
            .iter()
            .flat_map(|pt| [pt.x, pt.y])
            .collect::<alloc::vec::Vec<_>>();

        pd_func_caller!(
            (*self.0).fillPolygon,
            n_pts as i32,
            coords_seq.as_mut_ptr(),
            color.into(),
            fillrule
        )?;

        Ok(())
    }

    /// Draws a filled triangle with points at `p1`, `p2`, and `p3` with the provided [`color`][LCDColor].
    /// 
    /// [Playdate SDK Reference](https://sdk.play.date/inside-playdate-with-c/#f-graphics.fillTriangle)
    pub fn fill_triangle(
        &self,
        p1: ScreenPoint,
        p2: ScreenPoint,
        p3: ScreenPoint,
        color: LCDColor,
    ) -> Result<(), Error> {
        pd_func_caller!(
            (*self.0).fillTriangle,
            p1.x,
            p1.y,
            p2.x,
            p2.y,
            p3.x,
            p3.y,
            color.into(),
        )
    }

    /// Draws a hollow [ScreenRect] rectangle on the screen with the provided [`color`][LCDColor].
    /// 
    /// [Playdate SDK Reference](https://sdk.play.date/inside-playdate-with-c/#f-graphics.drawRect)
    pub fn draw_rect(&self, rect: ScreenRect, color: LCDColor) -> Result<(), Error> {
        pd_func_caller!(
            (*self.0).drawRect,
            rect.origin.x,
            rect.origin.y,
            rect.size.width,
            rect.size.height,
            color.into(),
        )
    }

    /// Draws a filled [ScreenRect] rectangle on the screen with the provided [`color`][LCDColor].
    /// 
    /// [Playdate SDK Reference](https://sdk.play.date/inside-playdate-with-c/#f-graphics.fillRect)
    pub fn fill_rect(&self, rect: ScreenRect, color: LCDColor) -> Result<(), Error> {
        pd_func_caller!(
            (*self.0).fillRect,
            rect.origin.x,
            rect.origin.y,
            rect.size.width,
            rect.size.height,
            color.into(),
        )
    }

    /// Draws a filled ellipse inside the rectangle `size` at position `origin`
    /// 
    /// * The ellipse will be drawn inset within the rectangle bounds.
    /// * The line will be drawn in the provided `line_width` and [`color`][LCDColor]
    /// * If `start_angle == end_angle`, this draws a complete ellipse.
    /// * If `start_angle != end_angle`, this draws an arc between the given angles.
    /// * Angles are given in degrees, clockwise from due north.
    /// 
    /// [Playdate SDK Reference](https://sdk.play.date/inside-playdate-with-c/#f-graphics.drawEllipse)
    pub fn draw_ellipse(
        &self,
        origin: ScreenPoint,
        size: ScreenSize,
        line_width: i32,
        start_angle: f32,
        end_angle: f32,
        color: LCDColor,
    ) -> Result<(), Error> {
        pd_func_caller!(
            (*self.0).drawEllipse,
            origin.x,
            origin.y,
            size.width,
            size.height,
            line_width,
            start_angle,
            end_angle,
            color.into(),
        )
    }

    /// Draws a solid ellipse inside the rectangle `size` at position `origin`
    /// 
    /// * The ellipse will be drawn inset within the rectangle bounds.
    /// * The line will be drawn in the provided `line_width` and [`color`][LCDColor]
    /// * If `start_angle == end_angle`, this draws a complete ellipse.
    /// * If `start_angle != end_angle`, this draws an wedge (or "pacman") shape between the given angles.
    /// * Angles are given in degrees, clockwise from due north.
    /// 
    /// [Playdate SDK Reference](https://sdk.play.date/inside-playdate-with-c/#f-graphics.drawEllipse)
    pub fn fill_ellipse(
        &self,
        target: OptionalBitmap,
        stencil: OptionalBitmap,
        origin: ScreenPoint,
        size: ScreenSize,
        line_width: i32,
        start_angle: f32,
        end_angle: f32,
        color: LCDColor,
        clip: LCDRect,
    ) -> Result<(), Error> {
        pd_func_caller!(
            (*self.0).fillEllipse,
            origin.x,
            origin.y,
            size.width,
            size.height,
            start_angle,
            end_angle,
            color.into(),
        )
    }

    /// Load the font at `path` into a [Font] object.
    /// 
    /// [Playdate SDK Reference](https://sdk.play.date/inside-playdate-with-c/#f-graphics.loadFont)
    pub fn load_font(&self, path: &str) -> Result<Font, Error> {
        let c_path = CString::new(path).map_err(Error::msg)?;
        let mut out_err: *const crankstart_sys::ctypes::c_char = ptr::null_mut();
        let font = pd_func_caller!((*self.0).loadFont, c_path.as_ptr(), &mut out_err)?;
        if font.is_null() {
            if !out_err.is_null() {
                let err_msg = unsafe { CStr::from_ptr(out_err).to_string_lossy().into_owned() };
                Err(anyhow!(err_msg))
            } else {
                Err(anyhow!(
                    "load_font failed without providing an error message"
                ))
            }
        } else {
            Font::new(font)
        }
    }

    /// Sets the [font][Font] to use in subsequent [Graphics::draw_text()] calls.
    /// 
    /// [Playdate SDK Reference](https://sdk.play.date/inside-playdate-with-c/#f-graphics.loadFont)
    pub fn set_font(&self, font: &Font) -> Result<(), Error> {
        pd_func_caller_log!((*self.0).setFont, font.0);
        Ok(())
    }

    /// Draws the given text using the provided options.
    /// 
    /// If no font has been set with [Graphics::set_font()],
    /// the default system font `Asheville Sans 14 Light` is used.
    /// 
    /// [Playdate SDK Reference](https://sdk.play.date/inside-playdate-with-c/#f-graphics.drawText)
    pub fn draw_text(&self, text: &str, position: ScreenPoint) -> Result<i32, Error> {
        let c_text = CString::new(text).map_err(Error::msg)?;
        pd_func_caller!(
            (*self.0).drawText,
            c_text.as_ptr() as *const core::ffi::c_void,
            text.len(),
            PDStringEncoding::kUTF8Encoding,
            position.x,
            position.y,
        )
    }

    /// Returns the width of the given `text` in the given [font][Font].
    /// 
    /// [Playdate SDK Reference](https://sdk.play.date/inside-playdate-with-c/#f-graphics.getTextWidth)
    pub fn get_text_width(&self, font: &Font, text: &str, tracking: i32) -> Result<i32, Error> {
        let c_text = CString::new(text).map_err(Error::msg)?;
        pd_func_caller!(
            (*self.0).getTextWidth,
            font.0,
            c_text.as_ptr() as *const core::ffi::c_void,
            text.len(),
            PDStringEncoding::kUTF8Encoding,
            tracking,
        )
    }

    /// Returns the height of the given [font][Font].
    /// 
    /// [Playdate SDK Reference](https://sdk.play.date/inside-playdate-with-c/#f-graphics.getFontHeight)
    pub fn get_font_height(&self, font: &Font) -> Result<u8, Error> {
        pd_func_caller!((*self.0).getFontHeight, font.0)
    }

    /// Returns the height of the system's default font.
    /// 
    /// This isn't a real API call, the system's default font is `Asheville Sans 14 Light`,
    /// a 14 point font.
    /// 
    /// This function just returns `14`, and is here for your convenience.
    pub fn get_system_font_height(&self) -> u8 {
        14
    }

    /// Returns the width of the given `text` in the system's default font
    /// 
    /// This is a convenience function to provide a safe way of calling
    /// [Graphics::get_text_width()] with a null pointer as the font.
    pub fn get_system_text_width(&self, text: &str, tracking: i32) -> Result<i32, Error> {
        let c_text = CString::new(text).map_err(Error::msg)?;
        pd_func_caller!(
            (*self.0).getTextWidth,
            ptr::null_mut(),
            c_text.as_ptr() as *const core::ffi::c_void,
            text.len(),
            PDStringEncoding::kUTF8Encoding,
            tracking,
        )
    }
}
