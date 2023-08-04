use crate::lerp::{inv_lerp, lerp};
use num::Complex;
use rodio::Source;
use rustfft::Fft;
use std::{
  cell::UnsafeCell,
  sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
  },
};

use crate::lerp::lerp_as;
const CZERO: Complex<f32> = Complex { re: 0.0, im: 0.0 };

#[derive(Debug, Clone, Copy)]
pub enum NoteState {
  Silent,
  Attack(f32),
  Decay(f32),
  Sustain(f32),
  Release(f32),
}

pub struct AdsrParams {
  attack_level: f32,
  sustain_level: f32,
  attack_dur: f32,
  decay_dur: f32,
  release_dur: f32,
  sustain_dur: f32,
}

impl NoteState {
  #[inline(always)]
  pub fn next(&mut self, adsr: &AdsrParams, dt: f32, sustain: bool) -> f32 {
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
      Decay(_) => (
        Sustain(if sustain {
          adsr.sustain_dur
        } else {
          adsr.sustain_dur / 4.0
        }),
        adsr.sustain_level,
      ),
      Sustain(t) if t > 0.0 => (Sustain(t - dt), adsr.sustain_level),
      Sustain(_) => (Release(adsr.release_dur), adsr.sustain_level),
      Release(t) if t > 0.0 => (
        Release(t - dt),
        lerp_as(t, adsr.release_dur, 0.0, adsr.sustain_level, 0.0),
      ),
      Release(_) => (Silent, 0.0),
    };
    val
  }
  #[inline(always)]
  pub fn peek(&self, adsr: &AdsrParams) -> f32 {
    use NoteState::*;
    match *self {
      Silent => 0.0,
      Attack(t) if t > 0.0 => lerp_as(t, adsr.attack_dur, 0.0, 0.0, adsr.attack_level),
      Attack(_) => adsr.attack_level,
      Decay(t) if t > 0.0 => lerp_as(
        t,
        adsr.decay_dur,
        0.0,
        adsr.attack_level,
        adsr.sustain_level,
      ),
      Decay(_) => adsr.sustain_level,
      Sustain(_) => adsr.sustain_level,
      Release(t) if t > 0.0 => lerp_as(t, adsr.release_dur, 0.0, adsr.sustain_level, 0.0),
      Release(_) => 0.0,
    }
  }
}

pub struct WavesControl {
  pub ss: UnsafeCell<Box<[NoteState]>>,
  pub sustain: AtomicBool,
  pub adsr: AdsrParams,
}

unsafe impl Send for WavesControl {}
unsafe impl Sync for WavesControl {}

impl WavesControl {
  pub fn hit(&self, note: usize) {
    let ss = unsafe { &mut *self.ss.get() };
    use NoteState::*;
    let f = 2.0f32.powf(note as f32 / 12.0) * 16.35;
    // println!("hit freq: {f}");
    let index = f as usize - 16;
    // println!("hit index: {index}");
    if index < ss.len() {
      let note = ss[index + 1];
      let v = note.peek(&self.adsr);
      let v = inv_lerp(v, 0.0, self.adsr.attack_level);
      let v = lerp(v, self.adsr.attack_dur, 0.0);
      ss[index + 1] = Attack(v);
    }
  }
  pub fn get_state(&self, freqs: &mut [f32]) {
    let ss = unsafe { &mut *self.ss.get() };
    for (o, f) in freqs.iter_mut().zip(ss.iter()) {
      *o = f.peek(&self.adsr);
    }
  }
  pub fn max_note(&self) -> usize {
    let ss = unsafe { &*self.ss.get() };
    let max_freq = ss.len();
    ((max_freq as f32 / 16.35).log2() * 12.0) as usize
  }
}
pub struct Waves {
  fft: Arc<dyn Fft<f32>>,
  window: Box<[Complex<f32>]>,
  buf: Box<[Complex<f32>]>,
  wp: usize,
  control: Arc<WavesControl>,
}

impl Waves {
  pub fn new(notes: usize) -> Self {
    let notes = notes.next_power_of_two();
    let mut planner = rustfft::FftPlanner::<f32>::new();
    let fft = planner.plan_fft_forward(notes);
    let buf = vec![CZERO; notes].into_boxed_slice();
    let ss = UnsafeCell::new(vec![NoteState::Silent; notes / 2 - 2].into_boxed_slice());
    let control = Arc::new(WavesControl {
      ss,
      sustain: AtomicBool::new(false),
      adsr: AdsrParams {
        attack_level: 0.5,
        sustain_level: 0.3,
        attack_dur: 0.1,
        decay_dur: 0.04,
        release_dur: 0.15,
        sustain_dur: 0.2,
      },
    });
    Self {
      fft,
      window: buf.clone(),
      buf,
      wp: 0,
      control,
    }
  }
  pub fn shallow_clone(&self) -> Self {
    let buf = vec![CZERO; self.buf.len()].into_boxed_slice();
    Self {
      fft: Arc::clone(&self.fft),
      window: buf.clone(),
      buf,
      wp: 0,
      control: self.control(),
    }
  }
  pub fn control(&self) -> Arc<WavesControl> {
    Arc::clone(&self.control)
  }
}

impl Iterator for Waves {
  type Item = f32;

  fn next(&mut self) -> Option<Self::Item> {
    if self.wp == self.window.len() {
      let ss = unsafe { &mut *self.control.ss.get() };
      let dt = 1.0 / self.window.len() as f32 * 16.0 * 2.0;
      let sustain = self.control.sustain.load(Ordering::Relaxed);
      let n = self.buf.len();
      self.window.fill(CZERO);
      let mut fsum = 0.0;
      for (i, b) in ss.iter_mut().enumerate() {
        let s = b.next(&self.control.adsr, dt, sustain);
        let s = s / (i as f32 + 1.0) * 5.0;
        fsum += s;
        let v = Complex::new(0f32, s);
        self.window[i + 1] = -v;
        self.window[n - i - 1] = v;
      }
      if fsum > 1.0 {
        for i in 1..n / 2 {
          self.window[i + 1] /= fsum;
          self.window[n - i - 1] /= fsum;
        }
      }
      self.fft.process_with_scratch(&mut self.window, &mut self.buf);

      self.wp = 0;
    }
    let v = self.window[self.wp].re;
    self.wp += 1;
    Some(v)
  }
}
impl Source for Waves {
  fn current_frame_len(&self) -> Option<usize> {
    None
  }

  fn channels(&self) -> u16 {
    1
  }

  fn sample_rate(&self) -> u32 {
    // 44100
    self.window.len() as u32 * 16
  }

  fn total_duration(&self) -> Option<std::time::Duration> {
    None
  }
}
