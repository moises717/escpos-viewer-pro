#[cfg(target_os = "windows")]
mod imp {
    use std::sync::{atomic::{AtomicIsize, Ordering}, Arc};

    use raw_window_handle::{HasWindowHandle, RawWindowHandle};
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        BringWindowToTop, GetWindowLongPtrW, SetForegroundWindow,
        SetWindowLongPtrW, SetWindowPos, ShowWindow,
        GWL_EXSTYLE, SWP_FRAMECHANGED, SWP_NOMOVE, SWP_NOSIZE, SWP_NOZORDER, SW_HIDE, SW_RESTORE,
        SW_SHOW, SWP_SHOWWINDOW, WS_EX_APPWINDOW, WS_EX_TOOLWINDOW, HWND_NOTOPMOST, HWND_TOPMOST,
    };

    use windows_sys::Win32::Foundation::RECT;
    use windows_sys::Win32::Graphics::Gdi::{
        GetMonitorInfoW, MonitorFromWindow, MONITORINFO, MONITOR_DEFAULTTONEAREST,
    };
    use windows_sys::Win32::UI::WindowsAndMessaging::GetWindowRect;

    #[derive(Clone, Default)]
    pub struct WindowControl {
        hwnd: Arc<AtomicIsize>,
    }

    impl WindowControl {
        pub fn try_update_from_frame(&self, frame: &mut eframe::Frame) {
            let Ok(window_handle) = frame.window_handle() else {
                return;
            };
            let RawWindowHandle::Win32(handle) = window_handle.as_raw() else {
                return;
            };
            let hwnd = handle.hwnd.get() as isize;
            if hwnd != 0 {
                self.hwnd.store(hwnd, Ordering::Relaxed);
            }
        }

        fn hwnd_ptr(&self) -> *mut core::ffi::c_void {
            let hwnd = self.hwnd.load(Ordering::Relaxed);
            hwnd as *mut core::ffi::c_void
        }

        fn set_taskbar_visible(&self, visible: bool) {
            let hwnd = self.hwnd_ptr();
            if hwnd.is_null() {
                return;
            }

            unsafe {
                let mut ex = GetWindowLongPtrW(hwnd, GWL_EXSTYLE) as isize;
                if visible {
                    ex &= !(WS_EX_TOOLWINDOW as isize);
                    ex |= WS_EX_APPWINDOW as isize;
                } else {
                    ex |= WS_EX_TOOLWINDOW as isize;
                    ex &= !(WS_EX_APPWINDOW as isize);
                }
                let _ = SetWindowLongPtrW(hwnd, GWL_EXSTYLE, ex);
                let _ = SetWindowPos(
                    hwnd,
                    core::ptr::null_mut(),
                    0,
                    0,
                    0,
                    0,
                    SWP_NOMOVE | SWP_NOSIZE | SWP_NOZORDER | SWP_FRAMECHANGED,
                );
            }
        }

        pub fn hide_to_tray(&self) {
            let hwnd = self.hwnd_ptr();
            if hwnd.is_null() {
                return;
            }
            self.set_taskbar_visible(false);
            unsafe {
                let _ = ShowWindow(hwnd, SW_HIDE);
            }
        }

        pub fn show_and_focus(&self) {
            let hwnd = self.hwnd_ptr();
            if hwnd.is_null() {
                return;
            }
            self.set_taskbar_visible(true);
            unsafe {
                let _ = ShowWindow(hwnd, SW_SHOW);
                let _ = ShowWindow(hwnd, SW_RESTORE);

                // Truco común en Windows para forzar que suba al frente:
                // poner TOPMOST y luego volver a NOTOPMOST (no queda siempre encima).
                let _ = SetWindowPos(
                    hwnd,
                    HWND_TOPMOST,
                    0,
                    0,
                    0,
                    0,
                    SWP_NOMOVE | SWP_NOSIZE | SWP_SHOWWINDOW,
                );
                let _ = SetWindowPos(
                    hwnd,
                    HWND_NOTOPMOST,
                    0,
                    0,
                    0,
                    0,
                    SWP_NOMOVE | SWP_NOSIZE | SWP_SHOWWINDOW,
                );

                let _ = BringWindowToTop(hwnd);
                let _ = SetForegroundWindow(hwnd);
            }
        }

        pub fn snap_near_right(&self, margin_px: i32) {
            let hwnd = self.hwnd_ptr();
            if hwnd.is_null() {
                return;
            }

            unsafe {
                let mut rect: RECT = core::mem::zeroed();
                if GetWindowRect(hwnd, &mut rect) == 0 {
                    return;
                }

                let w = (rect.right - rect.left).max(1);
                let h = (rect.bottom - rect.top).max(1);

                let monitor = MonitorFromWindow(hwnd, MONITOR_DEFAULTTONEAREST);
                if monitor.is_null() {
                    return;
                }

                let mut mi: MONITORINFO = core::mem::zeroed();
                mi.cbSize = core::mem::size_of::<MONITORINFO>() as u32;
                if GetMonitorInfoW(monitor, &mut mi) == 0 {
                    return;
                }

                // Usar work area (sin taskbar) y alinear abajo a la derecha.
                let work = mi.rcWork;
                let margin = margin_px.max(0);
                let mut x = work.right - w - margin;
                let mut y = work.bottom - h - margin;

                // Clamp básico para no salir del work area.
                x = x.max(work.left + margin).min(work.right - w - margin);
                y = y.max(work.top + margin).min(work.bottom - h - margin);

                let _ = SetWindowPos(
                    hwnd,
                    core::ptr::null_mut(),
                    x,
                    y,
                    0,
                    0,
                    SWP_NOSIZE | SWP_NOZORDER | SWP_SHOWWINDOW,
                );
            }
        }

        pub fn center_on_screen(&self) {
            let hwnd = self.hwnd_ptr();
            if hwnd.is_null() {
                return;
            }

            unsafe {
                let mut rect: RECT = core::mem::zeroed();
                if GetWindowRect(hwnd, &mut rect) == 0 {
                    return;
                }

                let w = (rect.right - rect.left).max(1);
                let h = (rect.bottom - rect.top).max(1);

                let monitor = MonitorFromWindow(hwnd, MONITOR_DEFAULTTONEAREST);
                if monitor.is_null() {
                    return;
                }

                let mut mi: MONITORINFO = core::mem::zeroed();
                mi.cbSize = core::mem::size_of::<MONITORINFO>() as u32;
                if GetMonitorInfoW(monitor, &mut mi) == 0 {
                    return;
                }

                let work = mi.rcWork;
                let work_w = (work.right - work.left).max(1);
                let work_h = (work.bottom - work.top).max(1);

                let x = work.left + ((work_w - w) / 2);
                let y = work.top + ((work_h - h) / 2);

                let _ = SetWindowPos(
                    hwnd,
                    core::ptr::null_mut(),
                    x,
                    y,
                    0,
                    0,
                    SWP_NOSIZE | SWP_NOZORDER | SWP_SHOWWINDOW,
                );
            }
        }
    }

    pub use WindowControl as WindowControlExport;
}

#[cfg(not(target_os = "windows"))]
mod imp {
    #[derive(Clone, Default)]
    pub struct WindowControl;

    impl WindowControl {
        pub fn try_update_from_frame(&self, _frame: &mut eframe::Frame) {}
        pub fn hide_to_tray(&self) {}
        pub fn show_and_focus(&self) {}
        pub fn snap_near_right(&self, _margin_px: i32) {}
        pub fn center_on_screen(&self) {}
    }

    pub use WindowControl as WindowControlExport;
}

pub use imp::WindowControlExport as WindowControl;
