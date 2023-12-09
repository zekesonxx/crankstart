use {
    crate::pd_func_caller, alloc::format, anyhow::Error, core::ptr, crankstart_sys::ctypes::c_void,
    cstr_core::CString,
};

use crankstart_sys::ctypes::c_int;
pub use crankstart_sys::PDButtons;
use crankstart_sys::{PDDateTime, PDLanguage, PDPeripherals};

static mut SYSTEM: System = System(ptr::null_mut());

/// Playdate System functions
#[derive(Clone, Debug)]
pub struct System(*const crankstart_sys::playdate_sys);

impl System {
    pub(crate) fn new(system: *const crankstart_sys::playdate_sys) {
        unsafe {
            SYSTEM = Self(system);
        }
    }

    pub fn get() -> Self {
        unsafe { SYSTEM.clone() }
    }

    /// Allocates heap space if ptr is NULL, else reallocates the given pointer.
    /// If size is zero, frees the given pointer.
    /// 
    /// You shouldn't need to use this, since crankstart already includes a Rust allocator.
    /// 
    /// [Playdate SDK Reference](https://sdk.play.date/inside-playdate-with-c/#f-system.realloc)

    pub(crate) fn realloc(&self, ptr: *mut c_void, size: usize) -> *mut c_void {
        unsafe {
            let realloc_fn = (*self.0).realloc.expect("realloc");
            realloc_fn(ptr, size)
        }
    }

    /// Replaces the default Lua run loop function with a custom update function.
    /// The update function should return a non-zero number to tell the system to update the
    /// display, or zero if update isn’t needed.
    /// 
    /// [Playdate SDK Reference](https://sdk.play.date/inside-playdate-with-c/#f-system.setUpdateCallback)
    pub fn set_update_callback(&self, f: crankstart_sys::PDCallbackFunction) -> Result<(), Error> {
        pd_func_caller!((*self.0).setUpdateCallback, f, ptr::null_mut())
    }

    /// `(current, pushed, released)`
    /// 
    /// Sets the value pointed to by current to a bitmask indicating which buttons are currently down.
    /// 
    /// `pushed` and `released` reflect which buttons were pushed or released over the previous
    /// update cycle—at the nominal frame rate of 50 ms, fast button presses can be missed if you
    /// just poll the instantaneous state.
    /// 
    /// [Playdate SDK Reference](https://sdk.play.date/inside-playdate-with-c/#f-system.getAccelerometer)
    pub fn get_button_state(&self) -> Result<(PDButtons, PDButtons, PDButtons), Error> {
        let mut current: PDButtons = PDButtons(0);
        let mut pushed: PDButtons = PDButtons(0);
        let mut released: PDButtons = PDButtons(0);
        pd_func_caller!(
            (*self.0).getButtonState,
            &mut current,
            &mut pushed,
            &mut released
        )?;
        Ok((current, pushed, released))
    }

    /// By default, the accelerometer is disabled to save (a small amount of) power.
    /// To use a peripheral, it must first be enabled via this function.
    /// Accelerometer data is not available until the next update cycle after it’s enabled.
    /// 
    /// [Playdate SDK Reference](https://sdk.play.date/inside-playdate-with-c/#f-system.getAccelerometer)
    pub fn set_peripherals_enabled(&self, peripherals: PDPeripherals) -> Result<(), Error> {
        pd_func_caller!((*self.0).setPeripheralsEnabled, peripherals)
    }

    /// Returns the last-read accelerometer data.
    /// 
    /// [Playdate SDK Reference](https://sdk.play.date/inside-playdate-with-c/#f-system.getAccelerometer)
    pub fn get_accelerometer(&self) -> Result<(f32, f32, f32), Error> {
        let mut outx = 0.0;
        let mut outy = 0.0;
        let mut outz = 0.0;
        pd_func_caller!((*self.0).getAccelerometer, &mut outx, &mut outy, &mut outz)?;
        Ok((outx, outy, outz))
    }

    /// Returns 1 or 0 indicating whether or not the crank is folded into the unit.
    ///
    /// [Playdate SDK Reference](https://sdk.play.date/inside-playdate-with-c/#f-system.isCrankDocked)
    pub fn is_crank_docked(&self) -> Result<bool, Error> {
        let docked: bool = pd_func_caller!((*self.0).isCrankDocked)? != 0;
        Ok(docked)
    }
    /// Returns the current position of the crank, in the range 0-360. Zero is pointing up, and the value increases as the crank moves clockwise, as viewed from the right side of the device.
    /// 
    /// [Playdate SDK Reference](https://sdk.play.date/2.1.1/Inside%20Playdate%20with%20C.html#f-system.getCrankAngle)
    pub fn get_crank_angle(&self) -> Result<f32, Error> {
        pd_func_caller!((*self.0).getCrankAngle,)
    }

    /// Returns the angle change of the crank since the last time this function was called. Negative values are anti-clockwise.
    /// 
    /// [Playdate SDK Reference](https://sdk.play.date/2.1.1/Inside%20Playdate%20with%20C.html#f-system.getCrankChange)
    pub fn get_crank_change(&self) -> Result<f32, Error> {
        pd_func_caller!((*self.0).getCrankChange,)
    }

    /// Disable the sound played when the crank is folded into or out of the unit.
    /// Returns the last value of the setting
    /// 
    /// [Playdate SDK Reference](https://sdk.play.date/inside-playdate-with-c/#f-system.setCrankSoundsDisabled)
    pub fn set_crank_sound_disabled(&self, disable: bool) -> Result<bool, Error> {
        let last = pd_func_caller!((*self.0).setCrankSoundsDisabled, disable as i32)?;
        Ok(last != 0)
    }

    /// Disables or enables the 60 second auto lock feature. When called, the timer is reset to 60 seconds.
    ///
    /// [Playdate SDK Reference](https://sdk.play.date/inside-playdate-with-c/#f-system.setAutoLockDisabled)
    pub fn set_auto_lock_disabled(&self, disable: bool) -> Result<(), Error> {
        pd_func_caller!((*self.0).setAutoLockDisabled, disable as i32)
    }

    /// Calls the log function.
    /// 
    /// [Playdate SDK Reference](https://sdk.play.date/inside-playdate-with-c/#f-system.logToConsole)
    pub fn log_to_console(text: &str) {
        unsafe {
            if !SYSTEM.0.is_null() {
                if let Ok(c_text) = CString::new(text) {
                    let log_to_console_fn = (*SYSTEM.0).logToConsole.expect("logToConsole");
                    log_to_console_fn(c_text.as_ptr() as *mut crankstart_sys::ctypes::c_char);
                }
            }
        }
    }

    pub fn log_to_console_raw(text: &str) {
        unsafe {
            if !SYSTEM.0.is_null() {
                let log_to_console_fn = (*SYSTEM.0).logToConsole.expect("logToConsole");
                log_to_console_fn(text.as_ptr() as *mut crankstart_sys::ctypes::c_char);
            }
        }
    }

    /// Calls the log function, outputting an error in red to the console, then pauses execution.
    /// 
    /// [Playdate SDK Reference](https://sdk.play.date/inside-playdate-with-c/#f-system.error)
    pub fn error(text: &str) {
        unsafe {
            if !SYSTEM.0.is_null() {
                if let Ok(c_text) = CString::new(text) {
                    let error_fn = (*SYSTEM.0).error.expect("error");
                    error_fn(c_text.as_ptr() as *mut crankstart_sys::ctypes::c_char);
                }
            }
        }
    }

    pub fn error_raw(text: &str) {
        unsafe {
            if !SYSTEM.0.is_null() {
                let error_fn = (*SYSTEM.0).error.expect("error");
                error_fn(text.as_ptr() as *mut crankstart_sys::ctypes::c_char);
            }
        }
    }

    /// Returns the number of seconds and milliseconds elapsed since midnight (hour 0), January 1, 2000.
    /// 
    /// [Playdate SDK Reference](https://sdk.play.date/inside-playdate-with-c/#f-system.getSecondsSinceEpoch)
    pub fn get_seconds_since_epoch(&self) -> Result<(usize, usize), Error> {
        let mut miliseconds = 0;
        let seconds = pd_func_caller!((*self.0).getSecondsSinceEpoch, &mut miliseconds)?;
        Ok((seconds as usize, miliseconds as usize))
    }

    /// Returns the number of milliseconds since…​some arbitrary point in time.
    /// This should present a consistent timebase while a game is running, but
    /// the counter will be disabled when the device is sleeping.
    /// 
    /// [Playdate SDK Reference](https://sdk.play.date/inside-playdate-with-c/#f-system.getCurrentTimeMilliseconds)
    pub fn get_current_time_milliseconds(&self) -> Result<usize, Error> {
        Ok(pd_func_caller!((*self.0).getCurrentTimeMilliseconds)? as usize)
    }

    /// Returns the system timezone offset from GMT, in seconds.
    /// [Playdate SDK Reference](https://sdk.play.date/inside-playdate-with-c/#f-system.getTimezoneOffset)
    pub fn get_timezone_offset(&self) -> Result<i32, Error> {
        pd_func_caller!((*self.0).getTimezoneOffset)
    }

    /// Converts the given epoch time to a PDDateTime.
    ///
    /// [Playdate SDK Reference](https://sdk.play.date/inside-playdate-with-c/#f-system.convertEpochToDateTime)
    pub fn convert_epoch_to_datetime(&self, epoch: u32) -> Result<PDDateTime, Error> {
        let mut datetime = PDDateTime::default();
        pd_func_caller!((*self.0).convertEpochToDateTime, epoch, &mut datetime)?;
        Ok(datetime)
    }

    /// Converts the given PDDateTime to an epoch time.
    /// 
    /// [Playdate SDK Reference](https://sdk.play.date/inside-playdate-with-c/#f-system.convertDateTimeToEpoch)
    pub fn convert_datetime_to_epoch(&self, datetime: &mut PDDateTime) -> Result<usize, Error> {
        Ok(pd_func_caller!((*self.0).convertDateTimeToEpoch, datetime)? as usize)
    }

    /// Returns whether the user has set the 24-Hour Time preference in the Settings program.
    /// 
    /// [Playdate SDK Reference](https://sdk.play.date/inside-playdate-with-c/#f-system.shouldDisplay24HourTime)
    pub fn should_display_24_hour_time(&self) -> Result<bool, Error> {
        Ok(pd_func_caller!((*self.0).shouldDisplay24HourTime)? != 0)
    }

    /// Resets the high-resolution timer.
    /// 
    /// [Playdate SDK Reference](https://sdk.play.date/inside-playdate-with-c/#f-system.resetElapsedTime)
    pub fn reset_elapsed_time(&self) -> Result<(), Error> {
        pd_func_caller!((*self.0).resetElapsedTime)
    }

    /// Returns the number of seconds since reset_elapsed_time() was called.
    /// The value is a floating-point number with microsecond accuracy.
    /// 
    /// [Playdate SDK Reference](https://sdk.play.date/inside-playdate-with-c/#f-system.getElapsedTime)
    pub fn get_elapsed_time(&self) -> Result<f32, Error> {
        pd_func_caller!((*self.0).getElapsedTime)
    }

    /// Returns whether the global "flipped" system setting is set.
    /// 
    /// [Playdate SDK Reference](https://sdk.play.date/inside-playdate-with-c/#f-system.getFlipped)
    pub fn get_flipped(&self) -> Result<bool, Error> {
        Ok(pd_func_caller!((*self.0).getFlipped)? != 0)
    }

    /// Returns whether the global "reduce flashing" system setting is set
    /// 
    /// [Playdate SDK Reference](https://sdk.play.date/inside-playdate-with-c/#f-system.getReduceFlashing)
    pub fn get_reduced_flashing(&self) -> Result<bool, Error> {
        Ok(pd_func_caller!((*self.0).getReduceFlashing)? != 0)
    }

    /// Calculates the current frames per second and draws that value at x, y.
    /// 
    /// [Playdate SDK Reference](https://sdk.play.date/inside-playdate-with-c/#f-system.drawFPS)
    pub fn draw_fps(&self, x: i32, y: i32) -> Result<(), Error> {
        pd_func_caller!((*self.0).drawFPS, x, y)
    }

    /// Returns a value from 0-100 denoting the current level of battery charge. 0 = empty; 100 = full.
    /// 
    /// [Playdate SDK Reference](https://sdk.play.date/inside-playdate-with-c/#f-system.getBatteryPercentage)
    pub fn get_battery_percentage(&self) -> Result<f32, Error> {
        pd_func_caller!((*self.0).getBatteryPercentage)
    }

    /// Returns the battery’s current voltage level.
    ///
    /// [Playdate SDK Reference](https://sdk.play.date/inside-playdate-with-c/#f-system.getBatteryVoltage)
    pub fn get_battery_voltage(&self) -> Result<f32, Error> {
        pd_func_caller!((*self.0).getBatteryVoltage)
    }

    /// Returns the current language of the system.
    /// 
    /// [Playdate SDK Reference](https://sdk.play.date/inside-playdate-with-c/#f-system.getLanguage)
    pub fn get_language(&self) -> Result<PDLanguage, Error> {
        pd_func_caller!((*self.0).getLanguage)
    }
}
