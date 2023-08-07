use plotters::{prelude::*, style::{WHITE, IntoFont}};
use rodio::{OutputStream, Sink};

use crate::waves::Waves;
use crate::windows::WindowBackend;

// pub mod fft;
pub mod lerp;
pub mod waves;
pub mod windows;

fn main() {
  const LEN: usize = 44100;
  let mut waves = Waves::new(LEN);
  const SIZE: (u32, u32) = (1920, 1080);
  let backend = WindowBackend::new(SIZE, waves.control());
  let control = waves.control();
  let (_stream, stream_handle) = OutputStream::try_default().unwrap();
  let sink = Sink::try_new(&stream_handle).unwrap();
  let waves_clone = waves.shallow_clone();
  sink.append(waves_clone);
  println!("max note: {}", control.max_note());

  let (mut updater, backend) = backend.into_backend();
  let root = backend.into_drawing_area();

  let space = (0..LEN).map(|i| i as f32);

  let mut buf = [0f32; LEN];
  let mut freq = [0f32; LEN / 2 - 2];
  loop {
    if !updater.update() {
      return;
    }
    buf.fill_with(|| waves.peek().unwrap());
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
    updater.present();
  }
}
// #[test]
// fn main_no_draw() {
//   use winapi::um::winuser::GetMessageW;
//   let waves = Waves::new(256);
//   let control = waves.control();
//   let (_stream, stream_handle) = OutputStream::try_default().unwrap();
//   let sink = Sink::try_new(&stream_handle).unwrap();
//   sink.append(waves);
//   println!("max note: {}", control.max_note());

//   create_window((800, 600));
//   let mut msg = MSG {
//     hwnd: std::ptr::null_mut(),
//     message: 0,
//     wParam: 0,
//     lParam: 0,
//     time: 0,
//     pt: POINT { x: 0, y: 0 },
//   };
//   let sound_key_vks = {
//     let mut key_vks = vec![];
//     key_vks.extend(b"QWERTYUIOP".map(|c| c as i32));
//     key_vks.extend([VK_OEM_4, VK_OEM_6]);
//     key_vks.extend(b"ASDFGHJKL".map(|c| c as i32));
//     key_vks
//   };
//   let mut sound_key_states = vec![false; sound_key_vks.len()];
//   let special_key_vks = {
//     let mut key_vks = vec![];
//     key_vks.extend(b" ".map(|c| c as i32));
//     key_vks
//   };
//   let mut special_key_states = vec![false; special_key_vks.len()];
//   loop {
//     let res = unsafe { GetMessageW(&mut msg, std::ptr::null_mut(), 0, 0) };
//     if res == 0 || res == -1 {
//       break;
//     }
//     unsafe { DispatchMessageW(&msg) };
//     if let Some((i, pressed)) = process_keyboard(
//       msg.message,
//       msg.wParam as i32,
//       &mut sound_key_states,
//       &sound_key_vks,
//     ) {
//       if pressed {
//         control.hit(i);
//       }
//     }
//     if let Some((_, pressed)) = process_keyboard(
//       msg.message,
//       msg.wParam as i32,
//       &mut special_key_states,
//       &special_key_vks,
//     ) {
//       match msg.wParam as u8 {
//         b' ' => {
//           control.sustain.store(pressed, Ordering::Relaxed);
//         }
//         _ => (),
//       }
//     }
//   }
// }

// #[test]
// fn main_test() {
//   use fft::{fft, ifft};
//   use std::f32::consts::PI;

//   use plotters::prelude::*;
//   const LEN: usize = 32;
//   let root = BitMapBackend::new("output.png", (1024, 800)).into_drawing_area();
//   root.fill(&WHITE).unwrap();
//   let mut chart = ChartBuilder::on(&root)
//     .x_label_area_size(35)
//     .y_label_area_size(40)
//     .margin(5)
//     .caption("Freqs vs Amps", ("sans-serif", 50.0).into_font())
//     .build_cartesian_2d(0f32..LEN as f32 + 1f32, -2f32..2f32)
//     .unwrap();
//   chart
//     .configure_mesh()
//     .y_desc("Amplitude")
//     .x_labels(7 * 5)
//     .x_max_light_lines(2)
//     .draw()
//     .unwrap();
//   let space = (0..LEN).map(|i| i as f32);
//   // ------------------------
//   const FREQ: f32 = 2.0;
//   const FREQ3: f32 = 5.0;
//   const FREQ2: f32 = 3.0;
//   const FREQ4: f32 = 4.0;

//   let mut buf = [num::Complex::new(0f32, 0f32); LEN];
//   let mut out = [num::Complex::new(0f32, 0f32); LEN];
//   buf.iter_mut().enumerate().for_each(|(i, n)| {
//     n.re = (2f32 * PI * i as f32 * FREQ / (LEN as f32)).cos()
//       + (2f32 * PI * i as f32 * FREQ2 / (LEN as f32)).sin()
//       + (2f32 * PI * i as f32 * FREQ3 / (LEN as f32)).cos()
//       + (2f32 * PI * i as f32 * FREQ4 / (LEN as f32)).sin()
//   });
//   let init = buf;
//   fft(&mut buf, &mut out);
//   chart
//     .draw_series(LineSeries::new(
//       space.clone().zip(init.map(|c| c.re)),
//       GREEN,
//     ))
//     .unwrap()
//     .label(format!("y = cos(2π{FREQ}t)"))
//     .legend(|(x, y)| PathElement::new(vec![(x, y), (x + 20, y)], GREEN));
//   chart
//     .draw_series(LineSeries::new(space.clone().zip(out.map(|c| c.re)), BLUE))
//     .unwrap()
//     .label(format!("y = FFT(cos(2π{FREQ}t)).re"))
//     .legend(|(x, y)| PathElement::new(vec![(x, y), (x + 20, y)], BLUE));
//   chart
//     .draw_series(LineSeries::new(
//       space.clone().zip(out.map(|c| c.im)),
//       MAGENTA,
//     ))
//     .unwrap()
//     .label(format!("y = FFT(cos(2π{FREQ}t)).im"))
//     .legend(|(x, y)| PathElement::new(vec![(x, y), (x + 20, y)], MAGENTA));
//   ifft(&mut out, &mut buf);
//   chart
//     .draw_series(LineSeries::new(space.clone().zip(buf.map(|c| c.re)), RED))
//     .unwrap()
//     .label(format!("y = IFFT(FFT(cos(2π{FREQ}t)))"))
//     .legend(|(x, y)| PathElement::new(vec![(x, y), (x + 20, y)], RED));

//   // ------------------------
//   // let mut buf = [0f32; LEN];
//   // let mut out = [0f32; LEN];
//   // buf[9] = 0.5;
//   // let init = buf;
//   // ifft(&mut buf, &mut out);
//   // chart.draw_series(LineSeries::new((0..LEN).map(|i| i as f32).zip(init), GREEN)).unwrap();
//   // chart.draw_series(LineSeries::new((0..LEN).map(|i| i as f32).zip(out), BLUE)).unwrap();
//   // ------------------------
//   chart
//     .configure_series_labels()
//     .background_style(&RGBColor(128, 128, 128))
//     .draw()
//     .unwrap();
//   root.present().expect("Unable to write result to file, please make sure 'plotters-doc-data' dir exists under current dir");
// }
