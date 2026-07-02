#[cfg(windows)]
/// https://github.com/sunshowers-code/enable-ansi-support
pub fn enable_ansi_support() -> Result<(), std::io::Error> {
    use std::{ ffi::OsStr, iter::once, os::windows::ffi::OsStrExt };

    use windows_sys::Win32::{
        Foundation::{ CloseHandle, INVALID_HANDLE_VALUE },
        Storage::FileSystem::{
            CreateFileW,
            FILE_GENERIC_READ,
            FILE_GENERIC_WRITE,
            FILE_SHARE_READ,
            FILE_SHARE_WRITE,
            OPEN_EXISTING,
        },
        System::Console::{ ENABLE_VIRTUAL_TERMINAL_PROCESSING, GetConsoleMode, SetConsoleMode },
    };

    unsafe {
        let console_out_name: Vec<u16> = OsStr::new("CONOUT$")
            .encode_wide()
            .chain(once(0))
            .collect();

        let console_handle = CreateFileW(
            console_out_name.as_ptr(),
            FILE_GENERIC_READ | FILE_GENERIC_WRITE,
            FILE_SHARE_READ | FILE_SHARE_WRITE,
            std::ptr::null(),
            OPEN_EXISTING,
            0,
            std::ptr::null_mut()
        );

        if console_handle == INVALID_HANDLE_VALUE {
            return Err(std::io::Error::last_os_error());
        }

        let result = {
            let mut console_mode = 0;

            if GetConsoleMode(console_handle, &mut console_mode) == 0 {
                Err(std::io::Error::last_os_error())
            } else if (console_mode & ENABLE_VIRTUAL_TERMINAL_PROCESSING) == 0 {
                let new_mode = console_mode | ENABLE_VIRTUAL_TERMINAL_PROCESSING;

                if SetConsoleMode(console_handle, new_mode) == 0 {
                    Err(std::io::Error::last_os_error())
                } else {
                    Ok(())
                }
            } else {
                Ok(())
            }
        };

        CloseHandle(console_handle);

        result
    }
}

#[cfg(not(windows))]
#[inline]
pub fn enable_ansi_support() -> Result<(), std::io::Error> {
    Ok(())
}
