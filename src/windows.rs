use crate::waves::{NoteMode, WavesControl};
use cutils::csizeof;
use plotters::{backend::BGRXPixel, prelude::*};
use std::{
  cell::UnsafeCell,
  ffi::OsStr,
  os::windows::prelude::OsStrExt,
  ptr::null_mut,
  sync::{atomic::Ordering, Arc},
};
use winapi::{
  shared::{
    minwindef::{HINSTANCE, LPARAM, LRESULT, UINT, WPARAM},
    windef::{HBRUSH, HDC, HWND, POINT, RECT},
  },
  um::{
    libloaderapi::GetModuleHandleW,
    wingdi::{
      GetStockObject, StretchDIBits, BITMAPINFO, BITMAPINFOHEADER, BI_RGB, DIB_RGB_COLORS, RGBQUAD,
      WHITE_BRUSH,
    },
    winuser::{
      CreateWindowExW, DefWindowProcW, DispatchMessageW, GetClientRect, GetDC, LoadCursorW,
      PeekMessageW, PostQuitMessage, RegisterClassExW, ReleaseDC, ShowWindow, CS_DBLCLKS,
      CS_HREDRAW, CS_VREDRAW, IDC_ARROW, MK_CONTROL, MK_LBUTTON, MK_MBUTTON, MK_RBUTTON, MK_SHIFT,
      MK_XBUTTON1, MK_XBUTTON2, MSG, PM_REMOVE, SW_SHOWMAXIMIZED, VK_OEM_1, VK_OEM_4, VK_OEM_5,
      VK_OEM_6, VK_OEM_7, WM_DESTROY, WM_KEYDOWN, WM_KEYUP, WM_MOUSEMOVE, WM_QUIT, WNDCLASSEXW,
      WS_OVERLAPPEDWINDOW,
    },
  },
};

pub struct WindowState {
  pub mouse: MouseMoveEvent,
}
pub struct WindowUpdater(WindowBackend, pub WindowState);
pub struct WindowBackend(Arc<UnsafeCell<WindowBackendInner>>);
struct WindowBackendInner {
  hwnd: HWND,
  size: (u32, u32),
  bm_buffer: Vec<u8>,
  bm_info: BITMAPINFO,
  msg: MSG,
  sound_key_vks: Vec<i32>,
  sound_key_states: Vec<bool>,
  special_key_vks: Vec<i32>,
  special_key_states: Vec<bool>,
  control: Arc<WavesControl>,
}
#[derive(Debug)]
pub struct DrawingError;
impl std::fmt::Display for DrawingError {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    f.write_fmt(format_args!("{self:?}"))
  }
}

impl WindowBackend {
  fn inner(&self) -> &mut WindowBackendInner {
    unsafe { &mut *self.0.get() }
  }
}
impl std::error::Error for DrawingError {}

impl WindowBackend {
  pub fn into_backend(&self) -> (WindowUpdater, BitMapBackend<BGRXPixel>) {
    let inner = unsafe { &mut *self.0.get() };
    let size = inner.size;
    (
      WindowUpdater(Self(Arc::clone(&self.0)), WindowState { mouse: Default::default() }),
      BitMapBackend::with_buffer_and_format(inner.bm_buffer.as_mut_slice(), size).unwrap(),
    )
  }
}
impl WindowUpdater {
  pub fn present(&mut self) {
    let inner = self.0.inner();
    unsafe {
      let device_context: HDC = GetDC(inner.hwnd);
      let mut client_rect = RECT {
        left: 0,
        top: 0,
        right: 0,
        bottom: 0,
      };
      GetClientRect(inner.hwnd, &mut client_rect);
      inner.draw(device_context, client_rect);
      ReleaseDC(inner.hwnd, device_context);
    }
  }
  pub fn update(&mut self) -> bool {
    let inner = self.0.inner();
    loop {
      let res = unsafe { PeekMessageW(&mut inner.msg, std::ptr::null_mut(), 0, 0, PM_REMOVE) };
      if res == 0 {
        break true;
      }
      if inner.msg.message == WM_QUIT {
        break false;
      }
      // if inner.msg.message == WM_SIZE {
      //   self.init();
      // }
      unsafe { DispatchMessageW(&inner.msg) };
      if let Some((i, pressed)) = process_keyboard(
        inner.msg.message,
        inner.msg.wParam as i32,
        &mut inner.sound_key_states,
        &inner.sound_key_vks,
      ) {
        if pressed {
          inner.control.hit(i);
        }
        continue;
      }
      if let Some((_, pressed)) = process_keyboard(
        inner.msg.message,
        inner.msg.wParam as i32,
        &mut inner.special_key_states,
        &inner.special_key_vks,
      ) {
        let code = inner.msg.wParam as u8;
        match code {
          b' ' => {
            inner.control.sustain.store(pressed, Ordering::Relaxed);
          }
          b'1' | b'2' | b'3' | b'4' => {
            let mode = match code {
              b'1' => NoteMode::Sine,
              b'2' => NoteMode::Saw,
              b'3' => NoteMode::Square,
              b'4' => NoteMode::Triangle,
              _ => unreachable!(),
            };
            unsafe { *inner.control.mode.get() = mode };
          }
          _ => (),
        }
        continue;
      }
      if let Some(event) = process_mouse(inner.msg.message, inner.msg.wParam, inner.msg.lParam as usize) {
        self.1.mouse = event;
        println!("mouse={event:?}");
      }
    }
  }
}
impl WindowBackend {
  pub fn new(size: (u32, u32), control: Arc<WavesControl>) -> Self {
    Self(Arc::new(UnsafeCell::new(WindowBackendInner::new(
      size, control,
    ))))
  }
}
impl WindowBackendInner {
  fn new(size: (u32, u32), control: Arc<WavesControl>) -> Self {
    let hwnd = create_window((size.0 as i32, size.1 as i32));
    let msg = MSG {
      hwnd: std::ptr::null_mut(),
      message: 0,
      wParam: 0,
      lParam: 0,
      time: 0,
      pt: POINT { x: 0, y: 0 },
    };
    let sound_key_vks = {
      let mut key_vks = vec![];
      key_vks.extend(b"QWERTYUIOP".map(|c| c as i32));
      key_vks.extend([VK_OEM_4, VK_OEM_6]);
      key_vks.extend(b"ASDFGHJKL".map(|c| c as i32));
      key_vks.extend([VK_OEM_1, VK_OEM_7, VK_OEM_5]);
      key_vks
    };
    let sound_key_states = vec![false; sound_key_vks.len()];
    let special_key_vks = {
      let mut key_vks = vec![];
      key_vks.extend(b" 1234".map(|c| c as i32));
      key_vks
    };
    let special_key_states = vec![false; special_key_vks.len()];
    Self {
      hwnd,
      size,
      bm_info: BITMAPINFO {
        bmiHeader: BITMAPINFOHEADER {
          biSize: csizeof!(BITMAPINFOHEADER),
          biWidth: size.0 as i32,
          biHeight: size.1 as i32,
          biPlanes: 1,
          biBitCount: 32,
          biCompression: BI_RGB,
          biSizeImage: 0,
          biXPelsPerMeter: 0,
          biYPelsPerMeter: 0,
          biClrUsed: 0,
          biClrImportant: 0,
        },
        bmiColors: [RGBQUAD {
          rgbBlue: 0,
          rgbGreen: 0,
          rgbRed: 0,
          rgbReserved: 0,
        }],
      },
      bm_buffer: vec![0; size.0 as usize * size.1 as usize * 4],
      msg,
      sound_key_vks,
      sound_key_states,
      special_key_vks,
      special_key_states,
      control,
    }
  }
  fn draw(&mut self, device_context: HDC, window_rect: RECT) {
    // update memory state bitmap to window
    // this is a rect to rect copy
    let window_width = window_rect.right - window_rect.left;
    let window_height = window_rect.bottom - window_rect.top;

    unsafe {
      StretchDIBits(
        device_context,
        0,
        window_height + 1,
        window_width,
        -window_height,
        0,
        0,
        self.bm_info.bmiHeader.biWidth,
        self.bm_info.bmiHeader.biHeight,
        self.bm_buffer.as_ptr().cast(),
        &self.bm_info,
        DIB_RGB_COLORS,
        winapi::um::wingdi::SRCCOPY,
      )
    };
  }
}

fn process_keyboard(
  message: u32,
  key: i32,
  key_states: &mut [bool],
  key_vks: &[i32],
) -> Option<(usize, bool)> {
  let pressed = match message {
    WM_KEYDOWN => true,
    WM_KEYUP => false,
    _ => return None,
  };
  // println!("key {} is {}", key, pressed);
  let Some((i, _)) = key_vks.iter().copied().enumerate().find(|(_, c)|*c==key as i32) else {
    return None;
  };
  let old_state = key_states[i];
  key_states[i] = pressed;
  if old_state != pressed {
    Some((i, pressed))
  } else {
    None
  }
}

#[derive(Debug, Default, Clone, Copy)]
pub struct MouseMoveEvent {
  pub x: i32,
  pub y: i32,
  pub ctrl: bool,
  pub left: bool,
  pub right: bool,
  pub middle: bool,
  pub shift: bool,
  pub x1: bool,
  pub x2: bool,
}

fn process_mouse(message: u32, mouse_vk: usize, mouse_xy: usize) -> Option<MouseMoveEvent> {
  if message != WM_MOUSEMOVE {
    return None;
  }
  let x = ((mouse_xy >> 0) & 0xFFFF) as i32;
  let y = ((mouse_xy >> 16) & 0xFFFF) as i32;
  let ctrl = mouse_vk & MK_CONTROL != 0;
  let left = mouse_vk & MK_LBUTTON != 0;
  let right = mouse_vk & MK_RBUTTON != 0;
  let middle = mouse_vk & MK_MBUTTON != 0;
  let shift = mouse_vk & MK_SHIFT != 0;
  let x1 = mouse_vk & MK_XBUTTON1 != 0;
  let x2 = mouse_vk & MK_XBUTTON2 != 0;
  Some(MouseMoveEvent {
    x,
    y,
    ctrl,
    left,
    right,
    middle,
    shift,
    x1,
    x2,
  })
}

fn to_wstring(s: &str) -> Vec<u16> {
  OsStr::new(s)
    .encode_wide()
    .chain(std::iter::once(0))
    .collect()
}

pub unsafe extern "system" fn window_proc(
  hwnd: HWND,
  msg: UINT,
  wparam: WPARAM,
  lparam: LPARAM,
) -> LRESULT {
  if msg == WM_DESTROY {
    PostQuitMessage(0);
    return 0;
  }
  return DefWindowProcW(hwnd, msg, wparam, lparam);
}

fn create_window(size: (i32, i32)) -> HWND {
  unsafe {
    let wc = WNDCLASSEXW {
      cbSize: csizeof!(WNDCLASSEXW),
      style: CS_HREDRAW | CS_VREDRAW | CS_DBLCLKS,
      lpfnWndProc: Some(window_proc),
      cbClsExtra: 0,
      cbWndExtra: 0,
      hInstance: GetModuleHandleW(null_mut()) as HINSTANCE,
      hIcon: null_mut(),
      hCursor: LoadCursorW(null_mut(), IDC_ARROW),
      hbrBackground: GetStockObject(WHITE_BRUSH as i32) as HBRUSH,
      lpszMenuName: null_mut(),
      lpszClassName: to_wstring("rust_window_class").as_ptr(),
      hIconSm: null_mut(),
    };
    if RegisterClassExW(&wc) == 0 {
      panic!("RegisterClassEx failed");
    }

    let hwnd = CreateWindowExW(
      0,
      wc.lpszClassName,
      to_wstring("Piano").as_ptr(),
      WS_OVERLAPPEDWINDOW,
      0,
      0,
      size.0,
      size.1,
      std::ptr::null_mut(),
      std::ptr::null_mut(),
      wc.hInstance,
      std::ptr::null_mut(),
    );
    if hwnd == std::ptr::null_mut() {
      panic!("CreateWindowEx failed");
    }
    ShowWindow(hwnd, SW_SHOWMAXIMIZED);
    hwnd
  }
}
