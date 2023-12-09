//! Filesystem-related functionality
#![warn(missing_docs)]
use {
    crate::{log_to_console, pd_func_caller, pd_func_caller_log},
    alloc::{boxed::Box, format, string::String, vec::Vec},
    anyhow::{ensure, Error},
    core::ptr,
    crankstart_sys::{ctypes::c_void, FileOptions, PDButtons, SDFile},
    cstr_core::CStr,
    cstr_core::CString,
};

pub use crankstart_sys::FileStat;

/// Internal helper function that handles getting the human-readable error from a filesystem call
fn ensure_filesystem_success(result: i32, function_name: &str) -> Result<(), Error> {
    if result < 0 {
        let file_sys = FileSystem::get();
        let err_result = pd_func_caller!((*file_sys.0).geterr)?;
        let err_string = unsafe { CStr::from_ptr(err_result) };

        Err(Error::msg(format!(
            "Error {} from {}: {:?}",
            result, function_name, err_string
        )))
    } else {
        Ok(())
    }
}

/// Global FileSystem handle
#[derive(Clone, Debug)]
pub struct FileSystem(*const crankstart_sys::playdate_file);

/// Internal function that gets passed to the C `listfiles()` call.
/// 
/// Used in [FileSystem::listfiles].
extern "C" fn list_files_callback(
    filename: *const crankstart_sys::ctypes::c_char,
    userdata: *mut core::ffi::c_void,
) {
    unsafe {
        let path = CStr::from_ptr(filename).to_string_lossy().into_owned();
        let files_ptr: *mut Vec<String> = userdata as *mut Vec<String>;
        (*files_ptr).push(path);
    }
}

impl FileSystem {
    pub(crate) fn new(file: *const crankstart_sys::playdate_file) {
        unsafe {
            FILE_SYSTEM = FileSystem(file);
        }
    }

    /// Return a copy of the global FileSystem object to access
    pub fn get() -> Self {
        unsafe { FILE_SYSTEM.clone() }
    }

    /// Returns a list of every file at `path`. Subfolders are indicated by a trailing slash `'/'`
    /// in filename. listfiles() does not recurse into subfolders. If `show_hidden` is set, files
    /// beginning with a period will be included; otherwise, they are skipped.
    /// 
    /// [Playdate SDK Reference](https://sdk.play.date/inside-playdate-with-c/#f-file.listfiles)
    pub fn listfiles(&self, path: &str, show_hidden: bool) -> Result<Vec<String>, Error> {
        let mut files: Box<Vec<String>> = Box::default();
        let files_ptr: *mut Vec<String> = &mut *files;
        let c_path = CString::new(path).map_err(Error::msg)?;
        let result = pd_func_caller!(
            (*self.0).listfiles,
            c_path.as_ptr(),
            Some(list_files_callback),
            files_ptr as *mut core::ffi::c_void,
            if show_hidden { 1 } else { 0 }
        )?;
        ensure_filesystem_success(result, "listfiles")?;
        Ok(*files)
    }

    /// Get information on a file, including whether it is a directory, the size (in bytes), and its last modified time.
    /// 
    /// [Playdate SDK Reference](https://sdk.play.date/inside-playdate-with-c/#f-file.stat)
    pub fn stat(&self, path: &str) -> Result<FileStat, Error> {
        let c_path = CString::new(path).map_err(Error::msg)?;
        let mut file_stat = FileStat::default();
        let result = pd_func_caller!((*self.0).stat, c_path.as_ptr(), &mut file_stat)?;
        ensure_filesystem_success(result, "stat")?;
        Ok(file_stat)
    }

    /// Creates the given path in the Data/<gameid> folder. It does not create intermediate folders.
    /// 
    /// Returns nothing on success.
    /// 
    /// [Playdate SDK Reference](https://sdk.play.date/inside-playdate-with-c/#f-file.mkdir)
    pub fn mkdir(&self, path: &str) -> Result<(), Error> {
        let c_path = CString::new(path).map_err(Error::msg)?;
        let result = pd_func_caller!((*self.0).mkdir, c_path.as_ptr())?;
        ensure_filesystem_success(result, "mkdir")?;
        Ok(())
    }

    /// Deletes the file at path. Returns nothing on success.
    /// 
    /// If recursive is `true` and the target path is a folder, this deletes everything inside the
    /// folder (including folders, folders inside those, and so on) as well as the folder itself.
    /// 
    /// [Playdate SDK Reference](https://sdk.play.date/inside-playdate-with-c/#f-file.unlink)
    pub fn unlink(&self, path: &str, recursive: bool) -> Result<(), Error> {
        let c_path = CString::new(path).map_err(Error::msg)?;
        let result = pd_func_caller!((*self.0).unlink, c_path.as_ptr(), recursive as i32)?;
        ensure_filesystem_success(result, "unlink")?;
        Ok(())
    }

    /// Renames the file at `from_path` to `to_path`. It will overwrite the file at `to_path` without confirmation.
    /// It does not create intermediate folders. Returns nothing on success.
    /// 
    /// [Playdate SDK Reference](https://sdk.play.date/inside-playdate-with-c/#f-file.rename)
    pub fn rename(&self, from_path: &str, to_path: &str) -> Result<(), Error> {
        let c_from_path = CString::new(from_path).map_err(Error::msg)?;
        let c_to_path = CString::new(to_path).map_err(Error::msg)?;
        let result = pd_func_caller!((*self.0).rename, c_from_path.as_ptr(), c_to_path.as_ptr())?;
        ensure_filesystem_success(result, "rename")?;
        Ok(())
    }

    /// Opens a [File] for the file at path. [FileOptions] is a bitmask.
    /// 
    /// The `kFileRead` mode opens a file in the game's pdx, while `kFileReadData` searches the game’s data folder;
    /// to search the data folder first then fall back on the game pdx,
    /// use the bitwise combination `kFileRead|kFileReadData`.
    /// `kFileWrite` and `kFileAppend` always write to the data folder.
    /// 
    /// The function will error if the file cannot be opened.
    /// 
    /// The filesystem has a limit of 64 simultaneous open files.
    /// 
    /// [Playdate SDK Reference](https://sdk.play.date/inside-playdate-with-c/#f-file.open)
    pub fn open(&self, path: &str, options: FileOptions) -> Result<File, Error> {
        let c_path = CString::new(path).map_err(Error::msg)?;
        
        let raw_file = pd_func_caller!((*self.0).open, c_path.as_ptr(), options)?;
        ensure!(
            !raw_file.is_null(),
            "Failed to open file at {} with options {:?}",
            path,
            options
        );
        Ok(File(raw_file))
    }

    /// Open the file at `path` and read it completely into a Rust [String]
    /// 
    /// This is a convenience function and not from the original Playdate C API
    pub fn read_file_as_string(&self, path: &str) -> Result<String, Error> {
        let stat = self.stat(path)?;
        let mut buffer = alloc::vec![0; stat.size as usize];
        let sd_file = self.open(path, FileOptions::kFileRead | FileOptions::kFileReadData)?;
        sd_file.read(&mut buffer)?;
        String::from_utf8(buffer).map_err(Error::msg)
    }
}

static mut FILE_SYSTEM: FileSystem = FileSystem(ptr::null_mut());

#[repr(i32)]
#[derive(Debug, Clone, Copy)]
/// How to seek in a file, used by [seek()][self::File::seek]
pub enum Whence {
    /// Seek relative to the beginning of the file
    Set = crankstart_sys::SEEK_SET as i32,
    /// Seek relative to the current position of the file pointer
    Cur = crankstart_sys::SEEK_CUR as i32,
    /// Seek relative to the end of the file.
    End = crankstart_sys::SEEK_END as i32,
}

/// An open file handle on the Playdate console.
/// 
/// Calls [close()](https://sdk.play.date/inside-playdate-with-c/#f-file.close) on the file handle when dropped.
#[derive(Debug)]
pub struct File(*mut SDFile);

impl File {
    /// Reads up to `len` bytes from the file into the buffer buf. Returns the number of bytes read.
    /// 
    /// [Playdate SDK Reference](https://sdk.play.date/inside-playdate-with-c/#f-file.read)

    pub fn read(&self, buf: &mut [u8]) -> Result<usize, Error> {
        let file_sys = FileSystem::get();
        let sd_file = self.0;
        let result = pd_func_caller!(
            (*file_sys.0).read,
            sd_file,
            buf.as_mut_ptr() as *mut core::ffi::c_void,
            buf.len() as u32
        )?;
        ensure_filesystem_success(result, "read")?;
        Ok(result as usize)
    }

    /// Writes the buffer of bytes `buf` to the file. Returns the number of bytes written.
    /// 
    /// [Playdate SDK Reference](https://sdk.play.date/inside-playdate-with-c/#f-file.write
    pub fn write(&self, buf: &[u8]) -> Result<usize, Error> {
        let file_sys = FileSystem::get();
        let sd_file = self.0;
        let result = pd_func_caller!(
            (*file_sys.0).write,
            sd_file,
            buf.as_ptr() as *mut core::ffi::c_void,
            buf.len() as u32
        )?;
        ensure_filesystem_success(result, "write")?;
        Ok(result as usize)
    }

    /// Flushes the output buffer of file immediately. Returns the number of bytes written.
    /// 
    /// [Playdate SDK Reference](https://sdk.play.date/inside-playdate-with-c/#f-file.flush)
    pub fn flush(&self) -> Result<(), Error> {
        let file_sys = FileSystem::get();
        let sd_file = self.0;
        let result = pd_func_caller!((*file_sys.0).flush, sd_file)?;
        ensure_filesystem_success(result, "flush")?;
        Ok(())
    }

    /// Returns the current read/write offset in the given file handle.
    /// 
    /// [Playdate SDK Reference](https://sdk.play.date/inside-playdate-with-c/#f-file.tell)
    pub fn tell(&self) -> Result<i32, Error> {
        let file_sys = FileSystem::get();
        let sd_file = self.0;
        let result = pd_func_caller!((*file_sys.0).tell, sd_file)?;
        ensure_filesystem_success(result, "tell")?;
        Ok(result)
    }

    /// Sets the read/write offset in the file handle to `pos`, relative to [Whence].
    /// 
    /// [Playdate SDK Reference](https://sdk.play.date/inside-playdate-with-c/#f-file.seek)
    pub fn seek(&self, pos: i32, whence: Whence) -> Result<(), Error> {
        let file_sys = FileSystem::get();
        let sd_file = self.0;
        let result = pd_func_caller!((*file_sys.0).seek, sd_file, pos, whence as i32)?;
        ensure_filesystem_success(result, "seek")?;
        Ok(())
    }
}

impl Drop for File {
    fn drop(&mut self) {
        let file_sys = FileSystem::get();
        let sd_file = self.0;
        pd_func_caller_log!((*file_sys.0).close, sd_file);
    }
}
