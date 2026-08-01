use super::provider::OverlayProvider;
use super::state::OverlayState;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use windows::{
    Win32::{
        Foundation::{COLORREF, HINSTANCE, HWND, LPARAM, LRESULT, RECT, WPARAM},
        Graphics::Gdi::{
            BeginPaint, CLIP_DEFAULT_PRECIS, CreateFontA, CreateSolidBrush, DEFAULT_PITCH,
            DEFAULT_QUALITY, DT_CENTER, DT_VCENTER, DT_WORDBREAK, DrawTextA, EndPaint, FF_DONTCARE,
            FW_BOLD, FillRect, InvalidateRect, OUT_DEFAULT_PRECIS, PAINTSTRUCT, SelectObject,
            SetBkMode, SetTextColor, TRANSPARENT,
        },
        UI::WindowsAndMessaging::{
            CS_HREDRAW, CS_VREDRAW, CW_USEDEFAULT, CreateWindowExA, DefWindowProcA, DestroyWindow,
            DispatchMessageA, GWL_EXSTYLE, GWL_STYLE, GetClientRect, GetWindowLongA, GetWindowRect,
            HMENU, HTCAPTION, HTCLIENT, LWA_ALPHA, LWA_COLORKEY, MSG, PM_REMOVE, PeekMessageA,
            PostQuitMessage, RegisterClassA, SW_HIDE, SW_SHOW, SWP_FRAMECHANGED, SWP_NOMOVE,
            SWP_NOSIZE, SWP_NOZORDER, SetLayeredWindowAttributes, SetWindowLongA, SetWindowPos,
            ShowWindow, TranslateMessage, WM_DESTROY, WM_LBUTTONDOWN, WM_NCHITTEST, WNDCLASSA,
            WS_EX_LAYERED, WS_EX_TOPMOST, WS_EX_TRANSPARENT, WS_POPUP, WS_THICKFRAME,
        },
    },
    core::PCSTR,
};

static WINDOW_CREATED: AtomicBool = AtomicBool::new(false);
static CURRENT_TAB: AtomicUsize = AtomicUsize::new(0);

pub struct NativeWindowProvider {
    hwnd: HWND,
    is_standalone: bool,
    is_unlocked: bool,
}

impl NativeWindowProvider {
    pub fn new(is_standalone: bool) -> Self {
        Self {
            hwnd: HWND::default(),
            is_standalone,
            is_unlocked: false,
        }
    }
}

unsafe extern "system" fn window_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    match msg {
        WM_DESTROY => {
            unsafe { PostQuitMessage(0) };
            LRESULT(0)
        }
        WM_LBUTTONDOWN => {
            let y = (((lparam.0 as usize) >> 16) & 0xFFFF) as i16 as i32;
            let x = ((lparam.0 as usize) & 0xFFFF) as i16 as i32;

            if y <= 40 {
                let mut rect = RECT::default();
                let _ = unsafe { GetClientRect(hwnd, &mut rect) };
                if x < rect.right / 2 {
                    CURRENT_TAB.store(0, Ordering::SeqCst);
                } else {
                    CURRENT_TAB.store(1, Ordering::SeqCst);
                }
            }
            LRESULT(0)
        }
        WM_NCHITTEST => {
            let res = unsafe { DefWindowProcA(hwnd, msg, wparam, lparam) };
            if res.0 == HTCLIENT as isize {
                let screen_y = (((lparam.0 as usize) >> 16) & 0xFFFF) as i16 as i32;
                let mut rect = RECT::default();
                let _ = unsafe { GetWindowRect(hwnd, &mut rect) };

                let client_y = screen_y - rect.top;

                if client_y > 40 {
                    return LRESULT(HTCAPTION as isize);
                }
            }
            LRESULT(res.0)
        }
        _ => unsafe { DefWindowProcA(hwnd, msg, wparam, lparam) },
    }
}

impl OverlayProvider for NativeWindowProvider {
    fn init(&mut self) -> Result<(), String> {
        unsafe {
            let class_name = b"AcProOverlayClass\0";

            let window_name_ptr = if self.is_standalone {
                c"AcProOverlay TEST MODE".as_ptr().cast()
            } else {
                c"AcProOverlay".as_ptr().cast()
            };

            if !WINDOW_CREATED.load(Ordering::SeqCst) {
                let wc = WNDCLASSA {
                    style: CS_HREDRAW | CS_VREDRAW,
                    lpfnWndProc: Some(window_proc),
                    cbClsExtra: 0,
                    cbWndExtra: 0,
                    hInstance: HINSTANCE::default(),
                    hIcon: Default::default(),
                    hCursor: Default::default(),
                    hbrBackground: Default::default(),
                    lpszMenuName: PCSTR::null(),
                    lpszClassName: PCSTR::from_raw(class_name.as_ptr()),
                };
                RegisterClassA(&wc);
                WINDOW_CREATED.store(true, Ordering::SeqCst);
            }

            let mut ex_style = WS_EX_LAYERED | WS_EX_TOPMOST;
            if !self.is_standalone {
                ex_style |= WS_EX_TRANSPARENT;
            }

            let mut style = WS_POPUP;
            if self.is_standalone {
                style |= WS_THICKFRAME;
            }

            self.hwnd = CreateWindowExA(
                ex_style,
                PCSTR::from_raw(class_name.as_ptr()),
                PCSTR::from_raw(window_name_ptr),
                style,
                CW_USEDEFAULT,
                CW_USEDEFAULT,
                500,
                250,
                HWND::default(),
                HMENU::default(),
                HINSTANCE::default(),
                None,
            );

            if self.is_standalone {
                let _ = SetLayeredWindowAttributes(self.hwnd, COLORREF(0), 235, LWA_ALPHA);
            } else {
                let _ = SetLayeredWindowAttributes(
                    self.hwnd,
                    COLORREF(0x00000000),
                    255,
                    LWA_COLORKEY | LWA_ALPHA,
                );
            }
        }
        Ok(())
    }

    fn set_visible(&mut self, visible: bool) {
        unsafe {
            if visible {
                ShowWindow(self.hwnd, SW_SHOW);
            } else {
                ShowWindow(self.hwnd, SW_HIDE);
            }
        }
    }

    fn set_unlocked(&mut self, unlocked: bool) {
        self.is_unlocked = unlocked;
        if self.is_standalone {
            return;
        }

        unsafe {
            let mut ex_style = GetWindowLongA(self.hwnd, GWL_EXSTYLE) as u32;
            let mut style = GetWindowLongA(self.hwnd, GWL_STYLE) as u32;

            if unlocked {
                ex_style &= !WS_EX_TRANSPARENT.0;
                style |= WS_THICKFRAME.0;
                let _ = SetLayeredWindowAttributes(self.hwnd, COLORREF(0), 235, LWA_ALPHA);
            } else {
                ex_style |= WS_EX_TRANSPARENT.0;
                style &= !WS_THICKFRAME.0;
                let _ = SetLayeredWindowAttributes(
                    self.hwnd,
                    COLORREF(0x00000000),
                    255,
                    LWA_COLORKEY | LWA_ALPHA,
                );
            }

            SetWindowLongA(self.hwnd, GWL_EXSTYLE, ex_style as i32);
            SetWindowLongA(self.hwnd, GWL_STYLE, style as i32);
            let _ = SetWindowPos(
                self.hwnd,
                HWND::default(),
                0,
                0,
                0,
                0,
                SWP_NOMOVE | SWP_NOSIZE | SWP_NOZORDER | SWP_FRAMECHANGED,
            );
        }
    }

    fn render(&mut self, state: &OverlayState) {
        unsafe {
            let mut msg = MSG::default();
            while PeekMessageA(&mut msg, HWND::default(), 0, 0, PM_REMOVE).into() {
                TranslateMessage(&msg);
                DispatchMessageA(&msg);
            }

            InvalidateRect(self.hwnd, None, true);

            let mut ps = PAINTSTRUCT::default();
            let hdc = BeginPaint(self.hwnd, &mut ps);

            let mut rect = RECT::default();
            let _ = GetClientRect(self.hwnd, &mut rect);

            let is_edit_mode = self.is_standalone || self.is_unlocked;

            let bg_color = if is_edit_mode {
                COLORREF(0x00111111)
            } else {
                COLORREF(0x00000000)
            };
            let bg_brush = CreateSolidBrush(bg_color);
            FillRect(hdc, &rect, bg_brush);
            windows::Win32::Graphics::Gdi::DeleteObject(bg_brush);

            let mut content_rect = rect;

            if is_edit_mode {
                let mut tab_rect = rect;
                tab_rect.bottom = 40;
                let tab_bg = CreateSolidBrush(COLORREF(0x00222222));
                FillRect(hdc, &tab_rect, tab_bg);
                windows::Win32::Graphics::Gdi::DeleteObject(tab_bg);

                SetBkMode(hdc, TRANSPARENT);
                let tab_font = CreateFontA(
                    24,
                    0,
                    0,
                    0,
                    FW_BOLD.0 as i32,
                    0,
                    0,
                    0,
                    0,
                    OUT_DEFAULT_PRECIS.0 as u32,
                    CLIP_DEFAULT_PRECIS.0 as u32,
                    DEFAULT_QUALITY.0 as u32,
                    (DEFAULT_PITCH.0 | FF_DONTCARE.0) as u32,
                    PCSTR::from_raw(c"Consolas".as_ptr().cast()),
                );
                let old_font = SelectObject(hdc, tab_font);

                let current_tab = CURRENT_TAB.load(Ordering::SeqCst);

                let mut t1_rect = tab_rect;
                t1_rect.right = rect.right / 2;
                SetTextColor(
                    hdc,
                    if current_tab == 0 {
                        COLORREF(0x0000FF00)
                    } else {
                        COLORREF(0x00888888)
                    },
                );
                DrawTextA(
                    hdc,
                    &mut b"TELEMETRY\0".to_vec(),
                    &mut t1_rect,
                    DT_CENTER | DT_VCENTER,
                );

                let mut t2_rect = tab_rect;
                t2_rect.left = rect.right / 2;
                SetTextColor(
                    hdc,
                    if current_tab == 1 {
                        COLORREF(0x0000FF00)
                    } else {
                        COLORREF(0x00888888)
                    },
                );
                DrawTextA(
                    hdc,
                    &mut b"ENGINEER\0".to_vec(),
                    &mut t2_rect,
                    DT_CENTER | DT_VCENTER,
                );

                SelectObject(hdc, old_font);
                windows::Win32::Graphics::Gdi::DeleteObject(tab_font);

                content_rect.top = 45;
            } else {
                SetBkMode(hdc, TRANSPARENT);
                content_rect.top = 10;
            }

            content_rect.left = 15;
            content_rect.right -= 15;

            let current_tab = CURRENT_TAB.load(Ordering::SeqCst);
            let mut display_text = String::new();

            if current_tab == 0 {
                if state.show_telemetry {
                    let is_redline = state.rpm > (state.max_rpm - 500) && state.rpm > 1000;
                    SetTextColor(
                        hdc,
                        if is_redline {
                            COLORREF(0x000000FF)
                        } else {
                            COLORREF(0x00FFFFFF)
                        },
                    );

                    let font_size = (rect.bottom / 4).max(25);
                    let font = CreateFontA(
                        font_size,
                        0,
                        0,
                        0,
                        FW_BOLD.0 as i32,
                        0,
                        0,
                        0,
                        0,
                        OUT_DEFAULT_PRECIS.0 as u32,
                        CLIP_DEFAULT_PRECIS.0 as u32,
                        DEFAULT_QUALITY.0 as u32,
                        (DEFAULT_PITCH.0 | FF_DONTCARE.0) as u32,
                        PCSTR::from_raw(c"Consolas".as_ptr().cast()),
                    );
                    let old_font = SelectObject(hdc, font);

                    let gear_str = match state.gear {
                        -1 => "R".to_string(),
                        0 => "N".to_string(),
                        g => g.to_string(),
                    };
                    display_text = format!(
                        "GEAR: {}\nSPD: {} KMH\nRPM: {}\0",
                        gear_str, state.speed_kmh, state.rpm
                    );
                    DrawTextA(
                        hdc,
                        &mut display_text.clone().into_bytes(),
                        &mut content_rect,
                        DT_WORDBREAK,
                    );
                    SelectObject(hdc, old_font);
                    windows::Win32::Graphics::Gdi::DeleteObject(font);
                }
            } else if current_tab == 1 && state.show_engineer {
                SetTextColor(hdc, COLORREF(0x0000FFFF));
                let font = CreateFontA(
                    22,
                    0,
                    0,
                    0,
                    FW_BOLD.0 as i32,
                    0,
                    0,
                    0,
                    0,
                    OUT_DEFAULT_PRECIS.0 as u32,
                    CLIP_DEFAULT_PRECIS.0 as u32,
                    DEFAULT_QUALITY.0 as u32,
                    (DEFAULT_PITCH.0 | FF_DONTCARE.0) as u32,
                    PCSTR::from_raw(c"Consolas".as_ptr().cast()),
                );
                let old_font = SelectObject(hdc, font);

                if state.engineer_messages.is_empty() {
                    display_text = "SYSTEMS NOMINAL: No advice.\0".to_string();
                } else {
                    display_text = String::from("ENGINEER ADVICE:\n");
                    for (i, msg) in state.engineer_messages.iter().enumerate() {
                        display_text.push_str(&format!("> {}\n", msg));
                        if i >= 4 {
                            break;
                        }
                    }
                    display_text.push('\0');
                }
                DrawTextA(
                    hdc,
                    &mut display_text.clone().into_bytes(),
                    &mut content_rect,
                    DT_WORDBREAK,
                );
                SelectObject(hdc, old_font);
                windows::Win32::Graphics::Gdi::DeleteObject(font);
            }

            if display_text.is_empty() && is_edit_mode {
                let fallback_text = String::from("CONTENT HIDDEN IN SETTINGS\0");
                DrawTextA(
                    hdc,
                    &mut fallback_text.into_bytes(),
                    &mut content_rect,
                    DT_WORDBREAK,
                );
            }

            let _ = EndPaint(self.hwnd, &ps);
        }
    }

    fn shutdown(&mut self) {
        unsafe {
            let _ = DestroyWindow(self.hwnd);
        }
    }
}
