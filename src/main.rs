use std::{
  cell::UnsafeCell,
  io::stdout,
  sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
  },
  time::Instant,
};

use crossterm::{
  cursor,
  event::{read, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers},
  execute,
  style::Print,
  terminal::{disable_raw_mode, enable_raw_mode, Clear, ClearType, EnterAlternateScreen, LeaveAlternateScreen},
};
use rodio::{OutputStream, Sink, Source};

#[derive(Debug, Clone, Copy)]
enum NoteState {
  Silent,
  Attack(f32),
  Decay(f32),
  Sustain(f32),
  Release(f32),
}

struct AdsrParams {
  attack_level: f32,
  sustain_level: f32,
  attack_dur: f32,
  decay_dur: f32,
  release_dur: f32,
  sustain_dur: f32,
}

#[inline(always)]
fn inv_lerp(t: f32, min: f32, max: f32) -> f32 {
  (t - min) / (max - min)
}
#[inline(always)]
fn lerp(t: f32, min: f32, max: f32) -> f32 {
  t * (max - min) + min
}
#[inline(always)]
fn lerp_as(t: f32, tmin: f32, tmax: f32, min: f32, max: f32) -> f32 {
  lerp(inv_lerp(t, tmin, tmax), min, max)
}

impl NoteState {
  fn next(&mut self, adsr: &AdsrParams, dt: f32, sustain: bool) -> f32 {
    use NoteState::*;
    let val;
    (*self, val) = match *self {
      Silent => (Silent, 0.0),
      Attack(t) if t > 0.0 => (
        Attack(t - dt),
        lerp_as(t, adsr.attack_dur, 0.0, 0.0, adsr.attack_level),
      ),
      Attack(_) => (Decay(adsr.decay_dur), adsr.attack_level),
      Decay(t) if t > 0.0 => (
        Decay(t - dt),
        lerp_as(
          t,
          adsr.decay_dur,
          0.0,
          adsr.attack_level,
          adsr.sustain_level,
        ),
      ),
      Decay(_) => (Sustain(adsr.sustain_dur), adsr.sustain_level),
      Sustain(t) if t > 0.0 && sustain => (Sustain(t - dt), adsr.sustain_level),
      Sustain(_) => (Release(adsr.release_dur), adsr.sustain_level),
      Release(t) if t > 0.0 => (
        Release(t - dt),
        lerp_as(t, adsr.release_dur, 0.0, adsr.sustain_level, 0.0),
      ),
      Release(_) => (Silent, 0.0),
    };
    val
  }
}

struct WavesControl {
  ss: UnsafeCell<Box<[NoteState]>>,
  sustain: AtomicBool,
  adsr: AdsrParams,
}

unsafe impl Send for WavesControl {}
unsafe impl Sync for WavesControl {}

impl WavesControl {
  fn hit(&self, note: usize) {
    let ss = unsafe { &mut *self.ss.get() };
    use NoteState::*;
    match ss[note] {
      Silent => ss[note] = Attack(self.adsr.attack_dur),
      Release(_) => ss[note] = Sustain(self.adsr.sustain_dur),
      _ => ()
    }
  }
  fn max_note(&self) -> usize {
    todo!()
  }
}
struct Waves {
  pt: Instant,
  window: Box<[f32]>,
  buf: Box<[f32]>,
  wp: usize,
  control: Arc<WavesControl>,
}

impl Waves {
  fn new(notes: usize) -> Self {
    let notes = notes.next_power_of_two();
    let buf = vec![0f32; notes].into_boxed_slice();
    let ss = UnsafeCell::new(vec![NoteState::Silent; notes].into_boxed_slice());
    let control = Arc::new(WavesControl {
      ss,
      sustain: AtomicBool::new(false),
      adsr: AdsrParams {
        attack_level: 0.9,
        sustain_level: 0.7,
        attack_dur: 0.04,
        decay_dur: 0.04,
        release_dur: 0.2,
        sustain_dur: 0.1,
      },
    });
    Self {
      pt: Instant::now(),
      window: buf.clone(),
      buf,
      wp: 0,
      control,
    }
  }
  fn control(&self) -> Arc<WavesControl> {
    Arc::clone(&self.control)
  }
}

fn ifft(input: &mut [f32], output: &mut [f32]) {
  debug_assert!(input.len().is_power_of_two());
  fn ifft_inner(
    buf_a: &mut [f32],
    buf_b: &mut [f32],
    n: usize,    // total length of the input array
    step: usize, // precalculated values for t
  ) {
    if step >= n {
      return;
    }

    ifft_inner(buf_b, buf_a, n, step * 2);
    ifft_inner(&mut buf_b[step..], &mut buf_a[step..], n, step * 2);
    // create a slice for each half of buf_a:
    let (left, right) = buf_a.split_at_mut(n / 2);

    for i in (0..n).step_by(step * 2) {
      let t = (std::f32::consts::PI * (i as f32) / (n as f32)).sin() * buf_b[i + step];
      left[i / 2] = buf_b[i] + t;
      right[i / 2] = buf_b[i] - t;
    }
  }
  output.copy_from_slice(input);
  ifft_inner(output, input, input.len(), 1);
}

impl Iterator for Waves {
  type Item = f32;

  fn next(&mut self) -> Option<Self::Item> {
    if self.wp == self.window.len() {
      let ss = unsafe { &mut *self.control.ss.get() };
      let ct = Instant::now();
      let dt = ct.duration_since(self.pt).as_secs_f32();
      let sustain = self.control.sustain.load(Ordering::Relaxed);
      for (i, o) in ss.iter_mut().zip(self.buf.iter_mut()) {
        *o = i.next(&self.control.adsr, dt, sustain);
      }
      self.pt = ct;
      ifft(&mut self.buf, &mut self.window);
      self.wp = 0;
    }
    let v = self.window[self.wp];
    self.wp += 1;
    Some(v)
  }
}
impl Source for Waves {
  fn current_frame_len(&self) -> Option<usize> {
    Some(self.window.len())
  }

  fn channels(&self) -> u16 {
    1
  }

  fn sample_rate(&self) -> u32 {
    44100
  }

  fn total_duration(&self) -> Option<std::time::Duration> {
    None
  }
}

fn main() {
  let waves = Waves::new(1024);
  let control = waves.control();
  let (_stream, stream_handle) = OutputStream::try_default().unwrap();
  let sink = Sink::try_new(&stream_handle).unwrap();
  sink.append(waves);
  let mut stdout = stdout();
  execute!(stdout, EnterAlternateScreen).unwrap();
  enable_raw_mode().unwrap();
  // let stdin = std::io::stdin();
  // let handle = stdin.as_handle();
  // let mut mode = ConsoleMode(0);
  // assert_eq!(unsafe { GetConsoleMode(handle, &mut mode) }, 1);
  execute!(
    stdout,
    Clear(ClearType::All),
    cursor::MoveTo(0, 0),
    Print(r#"ctrl + c to exit"#),
    // cursor::MoveTo(0, 2),
    // Print(&format!(r#"mode={mode:?}"#)),
  )
  .unwrap();
  
  loop {
    //going to top left corner
    execute!(stdout, cursor::MoveTo(0, 0)).unwrap();

    //matching the key
    match read().unwrap() {
      Event::Key(KeyEvent {
        code: KeyCode::Char('c'),
        modifiers: KeyModifiers::CONTROL,
        ..
      }) => break,
      Event::Key(KeyEvent {
        code: KeyCode::Char(' '),
        modifiers: KeyModifiers::NONE,
        kind: KeyEventKind::Press,
        ..
      }) => {
        let prev = control.sustain.load(Ordering::Relaxed);
        control.sustain.store(!prev, Ordering::Relaxed);
        execute!(
          stdout,
          cursor::MoveTo(0, 1),
          Print(format!(
            "sustain is {}   ",
            if !prev { "on" } else { "off" }
          ))
        )
        .unwrap();
      }
      Event::Key(KeyEvent {
        code: KeyCode::Char(c),
        modifiers: KeyModifiers::NONE,
        kind: KeyEventKind::Press | KeyEventKind::Repeat,
        ..
      }) => {
        let c = c as u8;
        const KEYS: &[u8] = b"qwertyuiop[]";
        let Some((i, _)) = KEYS.iter().enumerate().find(|(_, item)|**item==c) else {
          continue;
        };
        control.hit(i + 1);
      }
      _ => (),
    }
  }

  //disabling raw mode
  disable_raw_mode().unwrap();
  execute!(stdout, LeaveAlternateScreen).unwrap();
}

// pub const ENABLE_PROCESSED_INPUT: u32 = 0x0001;
// pub const ENABLE_LINE_INPUT: u32 = 0x0002;
// pub const ENABLE_ECHO_INPUT: u32 = 0x0004;
// pub const ENABLE_WINDOW_INPUT: u32 = 0x0008;
// pub const ENABLE_MOUSE_INPUT: u32 = 0x0010;
// pub const ENABLE_INSERT_MODE: u32 = 0x0020;
// pub const ENABLE_QUICK_EDIT_MODE: u32 = 0x0040;
// pub const ENABLE_EXTENDED_FLAGS: u32 = 0x0080;
// pub const ENABLE_AUTO_POSITION: u32 = 0x0100;
// pub const ENABLE_VIRTUAL_TERMINAL_INPUT: u32 = 0x0200;
// pub const ENABLE_PROCESSED_OUTPUT: u32 = 0x0001;
// pub const ENABLE_WRAP_AT_EOL_OUTPUT: u32 = 0x0002;
// pub const ENABLE_VIRTUAL_TERMINAL_PROCESSING: u32 = 0x0004;
// pub const DISABLE_NEWLINE_AUTO_RETURN: u32 = 0x0008;
// pub const ENABLE_LVB_GRID_WORLDWIDE: u32 = 0x0010;
// pub const CONSOLE_FULLSCREEN_MODE: u32 = 1;
// pub const CONSOLE_WINDOWED_MODE: u32 = 2;
// extern "system" {
//   fn GetConsoleMode(console_handle: BorrowedHandle, mode: &mut ConsoleMode) -> u32;
//   fn SetConsoleMode(console_handle: BorrowedHandle, mode: ConsoleMode) -> u32;
//   fn SetConsoleDisplayMode(
//     handle: BorrowedHandle,
//     flags: ConsoleModeFlags,
//     new_screen_buffer_dimentions: Option<&mut Coord>,
//   ) -> u32;
// }

// #[repr(transparent)]
// struct ConsoleMode(u32);
// #[repr(transparent)]
// struct ConsoleModeFlags(u32);

// #[derive(Debug)]
// #[repr(C)]
// struct Coord {
//   x: u16,
//   y: u16,
// }

// impl std::fmt::Debug for ConsoleModeFlags {
//   fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
//     const CONSTS: [(u32, &str); 2] = [
//       (CONSOLE_FULLSCREEN_MODE, "CONSOLE_FULLSCREEN_MODE"),
//       (CONSOLE_WINDOWED_MODE, "CONSOLE_WINDOWED_MODE"),
//     ];
//     let mut first = true;
//     for (val, str) in CONSTS {
//       if self.0 & val != 0 {
//         if first {
//           first = false;
//         } else {
//           f.write_str(" | ")?;
//         }
//         f.write_str(str)?;
//       }
//     }
//     if first {
//       f.write_str("None")?;
//     }
//     f.write_fmt(format_args!(" ({})", self.0))
//   }
// }

// impl std::fmt::Debug for ConsoleMode {
//   fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
//     const CONSTS: [(u32, &str); 15] = [
//       (ENABLE_PROCESSED_INPUT, "ENABLE_PROCESSED_INPUT"),
//       (ENABLE_LINE_INPUT, "ENABLE_LINE_INPUT"),
//       (ENABLE_ECHO_INPUT, "ENABLE_ECHO_INPUT"),
//       (ENABLE_WINDOW_INPUT, "ENABLE_WINDOW_INPUT"),
//       (ENABLE_MOUSE_INPUT, "ENABLE_MOUSE_INPUT"),
//       (ENABLE_INSERT_MODE, "ENABLE_INSERT_MODE"),
//       (ENABLE_QUICK_EDIT_MODE, "ENABLE_QUICK_EDIT_MODE"),
//       (ENABLE_EXTENDED_FLAGS, "ENABLE_EXTENDED_FLAGS"),
//       (ENABLE_AUTO_POSITION, "ENABLE_AUTO_POSITION"),
//       (
//         ENABLE_VIRTUAL_TERMINAL_INPUT,
//         "ENABLE_VIRTUAL_TERMINAL_INPUT",
//       ),
//       (ENABLE_PROCESSED_OUTPUT, "ENABLE_PROCESSED_OUTPUT"),
//       (ENABLE_WRAP_AT_EOL_OUTPUT, "ENABLE_WRAP_AT_EOL_OUTPUT"),
//       (
//         ENABLE_VIRTUAL_TERMINAL_PROCESSING,
//         "ENABLE_VIRTUAL_TERMINAL_PROCESSING",
//       ),
//       (DISABLE_NEWLINE_AUTO_RETURN, "DISABLE_NEWLINE_AUTO_RETURN"),
//       (ENABLE_LVB_GRID_WORLDWIDE, "ENABLE_LVB_GRID_WORLDWIDE"),
//     ];
//     let mut first = true;
//     for (val, str) in CONSTS {
//       if self.0 & val != 0 {
//         if first {
//           first = false;
//         } else {
//           f.write_str(" | ")?;
//         }
//         f.write_str(str)?;
//       }
//     }
//     if first {
//       f.write_str("None")?;
//     }
//     f.write_fmt(format_args!(" ({})", self.0))
//   }
// }
