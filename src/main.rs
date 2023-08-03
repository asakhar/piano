use cutils::csizeof;
use plotters_backend::{BackendColor, BackendCoord, DrawingErrorKind};
use rodio::{OutputStream, Sink};
use std::{
  cell::UnsafeCell,
  ffi::OsStr,
  os::windows::prelude::OsStrExt,
  sync::{atomic::Ordering, Arc},
};
use waves::WavesControl;
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
      CS_HREDRAW, CS_VREDRAW, IDC_ARROW, MSG, PM_REMOVE, SW_SHOW, VK_OEM_4, VK_OEM_6, WM_DESTROY,
      WM_KEYDOWN, WM_KEYUP, WM_QUIT, WNDCLASSEXW, WS_OVERLAPPEDWINDOW,
    },
  },
};

use crate::waves::Waves;

pub mod fft;
pub mod lerp;
pub mod waves;

use plotters::{prelude::*, backend::BGRXPixel};
struct WindowBackend(Arc<UnsafeCell<WindowBackendInner>>);
struct WindowBackendInner {
  hwnd: HWND,
  size: (u32, u32),
  isize: (i32, i32),
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
struct DrawingError((i32, i32), BackendCoord, usize, usize);
impl std::fmt::Display for DrawingError {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    f.write_fmt(format_args!("{self:?}"))
  }
}

impl WindowBackend {
  fn inner(&self) -> &WindowBackendInner {
    unsafe { &*self.0.get() }
  }
  fn inner_mut(&self) -> &mut WindowBackendInner {
    unsafe { &mut *self.0.get() }
  }
}
impl std::error::Error for DrawingError {}

impl WindowBackend {
  pub fn get_backend(&self) -> BitMapBackend<BGRXPixel> {
    let inner = unsafe {&mut *self.0.get()};
    let size = inner.size;
    BitMapBackend::with_buffer_and_format(inner.bm_buffer.as_mut_slice(), size).unwrap()
  }
  pub fn present(&mut self) {
    let inner = self.inner_mut();
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
}
// impl DrawingBackend for WindowBackend {
//   type ErrorType = DrawingError;

//   fn get_size(&self) -> (u32, u32) {
//     let size = self.inner().size;
//     (size.0 - 1, size.1 - 1)
//   }

//   fn ensure_prepared(&mut self) -> Result<(), DrawingErrorKind<Self::ErrorType>> {
//     Ok(())
//   }

//   fn present(&mut self) -> Result<(), DrawingErrorKind<Self::ErrorType>> {
//     let inner = self.inner_mut();
//     unsafe {
//       let device_context: HDC = GetDC(inner.hwnd);
//       let mut client_rect = RECT {
//         left: 0,
//         top: 0,
//         right: 0,
//         bottom: 0,
//       };
//       GetClientRect(inner.hwnd, &mut client_rect);
//       inner.draw(device_context, client_rect);
//       ReleaseDC(inner.hwnd, device_context);
//     }
//     Ok(())
//   }

//   fn draw_pixel(
//     &mut self,
//     point: BackendCoord,
//     color: BackendColor,
//   ) -> Result<(), DrawingErrorKind<Self::ErrorType>> {
//     let inner = self.inner_mut();
//     let index = (point.0.clamp(0, inner.isize.0 - 1)
//       + point.1.clamp(0, inner.isize.1 - 1) * inner.isize.0) as usize;
//     let Some(px) = inner.bm_buffer.get_mut(index) else {
//       return Ok(());
//     };
//     let BackendColor {
//       alpha,
//       rgb: (r, g, b),
//     } = color;
//     let a = (alpha.clamp(0.0, 1.0) * 255.0) as u8;
//     *px = [b, g, r, a];
//     Ok(())
//   }
// }
impl WindowBackend {
  fn new(size: (i32, i32), control: Arc<WavesControl>) -> Self {
    Self(Arc::new(UnsafeCell::new(WindowBackendInner::new(
      size, control,
    ))))
  }
  fn clone(&self) -> WindowBackend {
    Self(Arc::clone(&self.0))
  }
  fn update(&mut self) -> bool {
    let inner = self.inner_mut();
    loop {
      let res = unsafe { PeekMessageW(&mut inner.msg, std::ptr::null_mut(), 0, 0, PM_REMOVE) };
      if res == 0 {
        break true;
      }
      if inner.msg.message == WM_QUIT {
        break false;
      }
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
      }
      if let Some((_, pressed)) = process_keyboard(
        inner.msg.message,
        inner.msg.wParam as i32,
        &mut inner.special_key_states,
        &inner.special_key_vks,
      ) {
        match inner.msg.wParam as u8 {
          b' ' => {
            inner.control.sustain.store(pressed, Ordering::Relaxed);
          }
          _ => (),
        }
      }
    }
  }
}
impl WindowBackendInner {
  fn new(size: (i32, i32), control: Arc<WavesControl>) -> Self {
    let hwnd = create_window(size);
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
      key_vks
    };
    let sound_key_states = vec![false; sound_key_vks.len()];
    let special_key_vks = {
      let mut key_vks = vec![];
      key_vks.extend(b" ".map(|c| c as i32));
      key_vks
    };
    let special_key_states = vec![false; special_key_vks.len()];
    Self {
      hwnd,
      size: (size.0 as u32, size.1 as u32),
      isize: size,
      bm_info: BITMAPINFO {
        bmiHeader: BITMAPINFOHEADER {
          biSize: csizeof!(BITMAPINFOHEADER),
          biWidth: size.0,
          biHeight: size.1,
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
        0,
        self.bm_info.bmiHeader.biWidth,
        self.bm_info.bmiHeader.biHeight,
        0,
        window_height + 1,
        window_width,
        -window_height,
        self.bm_buffer.as_ptr().cast(),
        &self.bm_info,
        DIB_RGB_COLORS,
        winapi::um::wingdi::SRCCOPY,
      )
    };
  }
}

fn main() {
  const LEN: usize = 256;
  let waves = Waves::new(LEN);
  const SIZE: (i32, i32) = (1024, 800);
  let mut backend = WindowBackend::new(SIZE, waves.control());
  let control = waves.control();
  let (_stream, stream_handle) = OutputStream::try_default().unwrap();
  let sink = Sink::try_new(&stream_handle).unwrap();
  let mut waves_clone = waves.shallow_clone();
  sink.append(waves);
  println!("max note: {}", control.max_note());
  let mut backend_copy = backend.clone();

  let root = backend_copy.get_backend().into_drawing_area();

  let space = (0..LEN).map(|i| i as f32);

  let mut buf = [0f32; LEN];
  let mut freq = [0f32; LEN / 2 - 2];
  loop {
    if !backend.update() {
      return;
    }
    buf.fill_with(|| waves_clone.next().unwrap());
    control.get_state(&mut freq);
    root.fill(&WHITE).unwrap();
    let mut chart = ChartBuilder::on(&root)
      .x_label_area_size(35)
      .y_label_area_size(40)
      .margin(5)
      .caption("Freqs vs Amps", ("sans-serif", 50.0).into_font())
      .build_cartesian_2d(0f32..LEN as f32 + 1f32, -2f32..2f32)
      .unwrap();
    chart
      .configure_mesh()
      .y_desc("Amplitude")
      .x_labels(7 * 5)
      .x_max_light_lines(2)
      .draw()
      .unwrap();
    chart
      .draw_series(LineSeries::new(space.clone().zip(buf), RED))
      .unwrap()
      .label("a")
      .legend(|(x, y)| PathElement::new(vec![(x, y), (x + 20, y)], RED));
    chart
      .draw_series(LineSeries::new(space.clone().zip(freq), BLUE))
      .unwrap()
      .label("f")
      .legend(|(x, y)| PathElement::new(vec![(x, y), (x + 20, y)], BLUE));

    chart
      .configure_series_labels()
      .background_style(&RGBColor(128, 128, 128))
      .draw()
      .unwrap();
    root.present().expect("Unable to write result to file, please make sure 'plotters-doc-data' dir exists under current dir");
    backend.present();
  }
}
#[test]
fn main_no_draw() {
  use winapi::um::winuser::GetMessageW;
  let waves = Waves::new(256);
  let control = waves.control();
  let (_stream, stream_handle) = OutputStream::try_default().unwrap();
  let sink = Sink::try_new(&stream_handle).unwrap();
  sink.append(waves);
  println!("max note: {}", control.max_note());

  create_window((800, 600));
  let mut msg = MSG {
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
    key_vks
  };
  let mut sound_key_states = vec![false; sound_key_vks.len()];
  let special_key_vks = {
    let mut key_vks = vec![];
    key_vks.extend(b" ".map(|c| c as i32));
    key_vks
  };
  let mut special_key_states = vec![false; special_key_vks.len()];
  loop {
    let res = unsafe { GetMessageW(&mut msg, std::ptr::null_mut(), 0, 0) };
    if res == 0 || res == -1 {
      break;
    }
    unsafe { DispatchMessageW(&msg) };
    if let Some((i, pressed)) = process_keyboard(
      msg.message,
      msg.wParam as i32,
      &mut sound_key_states,
      &sound_key_vks,
    ) {
      if pressed {
        control.hit(i);
      }
    }
    if let Some((_, pressed)) = process_keyboard(
      msg.message,
      msg.wParam as i32,
      &mut special_key_states,
      &special_key_vks,
    ) {
      match msg.wParam as u8 {
        b' ' => {
          control.sustain.store(pressed, Ordering::Relaxed);
        }
        _ => (),
      }
    }
  }
}

#[test]
fn main_test() {
  use fft::{fft, ifft};
  use std::f32::consts::PI;

  use plotters::prelude::*;
  const LEN: usize = 32;
  let root = BitMapBackend::new("output.png", (1024, 800)).into_drawing_area();
  root.fill(&WHITE).unwrap();
  let mut chart = ChartBuilder::on(&root)
    .x_label_area_size(35)
    .y_label_area_size(40)
    .margin(5)
    .caption("Freqs vs Amps", ("sans-serif", 50.0).into_font())
    .build_cartesian_2d(0f32..LEN as f32 + 1f32, -2f32..2f32)
    .unwrap();
  chart
    .configure_mesh()
    .y_desc("Amplitude")
    .x_labels(7 * 5)
    .x_max_light_lines(2)
    .draw()
    .unwrap();
  let space = (0..LEN).map(|i| i as f32);
  // ------------------------
  const FREQ: f32 = 2.0;
  const FREQ3: f32 = 5.0;
  const FREQ2: f32 = 3.0;
  const FREQ4: f32 = 4.0;

  let mut buf = [num::Complex::new(0f32, 0f32); LEN];
  let mut out = [num::Complex::new(0f32, 0f32); LEN];
  buf.iter_mut().enumerate().for_each(|(i, n)| {
    n.re = (2f32 * PI * i as f32 * FREQ / (LEN as f32)).cos()
      + (2f32 * PI * i as f32 * FREQ2 / (LEN as f32)).sin()
      + (2f32 * PI * i as f32 * FREQ3 / (LEN as f32)).cos()
      + (2f32 * PI * i as f32 * FREQ4 / (LEN as f32)).sin()
  });
  let init = buf;
  fft(&mut buf, &mut out);
  chart
    .draw_series(LineSeries::new(
      space.clone().zip(init.map(|c| c.re)),
      GREEN,
    ))
    .unwrap()
    .label(format!("y = cos(2π{FREQ}t)"))
    .legend(|(x, y)| PathElement::new(vec![(x, y), (x + 20, y)], GREEN));
  chart
    .draw_series(LineSeries::new(space.clone().zip(out.map(|c| c.re)), BLUE))
    .unwrap()
    .label(format!("y = FFT(cos(2π{FREQ}t)).re"))
    .legend(|(x, y)| PathElement::new(vec![(x, y), (x + 20, y)], BLUE));
  chart
    .draw_series(LineSeries::new(
      space.clone().zip(out.map(|c| c.im)),
      MAGENTA,
    ))
    .unwrap()
    .label(format!("y = FFT(cos(2π{FREQ}t)).im"))
    .legend(|(x, y)| PathElement::new(vec![(x, y), (x + 20, y)], MAGENTA));
  ifft(&mut out, &mut buf);
  chart
    .draw_series(LineSeries::new(space.clone().zip(buf.map(|c| c.re)), RED))
    .unwrap()
    .label(format!("y = IFFT(FFT(cos(2π{FREQ}t)))"))
    .legend(|(x, y)| PathElement::new(vec![(x, y), (x + 20, y)], RED));

  // ------------------------
  // let mut buf = [0f32; LEN];
  // let mut out = [0f32; LEN];
  // buf[9] = 0.5;
  // let init = buf;
  // ifft(&mut buf, &mut out);
  // chart.draw_series(LineSeries::new((0..LEN).map(|i| i as f32).zip(init), GREEN)).unwrap();
  // chart.draw_series(LineSeries::new((0..LEN).map(|i| i as f32).zip(out), BLUE)).unwrap();
  // ------------------------
  chart
    .configure_series_labels()
    .background_style(&RGBColor(128, 128, 128))
    .draw()
    .unwrap();
  root.present().expect("Unable to write result to file, please make sure 'plotters-doc-data' dir exists under current dir");
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
      cbSize: std::mem::size_of::<WNDCLASSEXW>() as UINT,
      style: CS_HREDRAW | CS_VREDRAW | CS_DBLCLKS,
      lpfnWndProc: Some(window_proc),
      cbClsExtra: 0,
      cbWndExtra: 0,
      hInstance: GetModuleHandleW(std::ptr::null_mut()) as HINSTANCE,
      hIcon: std::ptr::null_mut(),
      hCursor: LoadCursorW(std::ptr::null_mut(), IDC_ARROW),
      hbrBackground: GetStockObject(WHITE_BRUSH as i32) as HBRUSH,
      lpszMenuName: std::ptr::null_mut(),
      lpszClassName: to_wstring("rust_window_class").as_ptr(),
      hIconSm: std::ptr::null_mut(),
    };
    if RegisterClassExW(&wc) == 0 {
      panic!("RegisterClassEx failed");
    }

    let hwnd = CreateWindowExW(
      0,
      wc.lpszClassName,
      to_wstring("Rust Window").as_ptr(),
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

    ShowWindow(hwnd, SW_SHOW);
    hwnd
  }
}
