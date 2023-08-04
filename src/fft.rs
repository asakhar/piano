use std::f32::consts::PI;

const I: num::Complex<f32> = num::Complex { re: 0.0, im: 1.0 };

// pub fn ifft_re(input: &mut [f32], output: &mut [f32]) {
//   debug_assert!(input.len().is_power_of_two());
//   fn ifft_inner(
//     buf_a: &mut [f32],
//     buf_b: &mut [f32],
//     n: usize,    // total length of the input array
//     step: usize, // precalculated values for t
//   ) {
//     if step >= n {
//       return;
//     }

//     ifft_inner(buf_b, buf_a, n, step * 2);
//     ifft_inner(&mut buf_b[step..], &mut buf_a[step..], n, step * 2);
//     // create a slice for each half of buf_a:
//     let (left, right) = buf_a.split_at_mut(n / 2);

//     for i in (0..n).step_by(step * 2) {
//       let t = (I * PI * (i as f32) / (n as f32)).exp() * buf_b[i + step];
//       let t = t.norm();
//       left[i / 2] = buf_b[i] + t;
//       right[i / 2] = buf_b[i] - t;
//     }
//   }
//   output.copy_from_slice(input);
//   ifft_inner(output, input, input.len(), 1);
// }

pub fn fft(input: &mut [num::Complex<f32>], output: &mut [num::Complex<f32>]) {
  debug_assert!(input.len().is_power_of_two());
  fn fft_inner(
    buf_a: &mut [num::Complex<f32>],
    buf_b: &mut [num::Complex<f32>],
    n: usize,    // total length of the input array
    step: usize, // precalculated values for t
  ) {
    if step >= n {
      return;
    }

    fft_inner(buf_b, buf_a, n, step * 2);
    fft_inner(&mut buf_b[step..], &mut buf_a[step..], n, step * 2);
    // create a slice for each half of buf_a:
    let (left, right) = buf_a.split_at_mut(n / 2);

    for i in (0..n).step_by(step * 2) {
      let t = (-I * PI * (i as f32) / (n as f32)).exp() * buf_b[i + step];
      left[i / 2] = buf_b[i] + t;
      right[i / 2] = buf_b[i] - t;
    }
  }
  output.copy_from_slice(input);
  fft_inner(output, input, input.len(), 1);
  output
    .iter_mut()
    .for_each(|c| *c = c.scale(1.0f32 / input.len() as f32))
}

pub fn ifft(input: &mut [num::Complex<f32>], output: &mut [num::Complex<f32>]) {
  debug_assert!(input.len().is_power_of_two());
  debug_assert_eq!(input.len(), output.len());
  fn ifft_inner(
    buf_a: &mut [num::Complex<f32>],
    buf_b: &mut [num::Complex<f32>],
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
      let t = (I * PI * (i as f32) / (n as f32)).exp() * buf_b[i + step];
      left[i / 2] = buf_b[i] + t;
      right[i / 2] = buf_b[i] - t;
    }
  }
  output.copy_from_slice(input);
  ifft_inner(output, input, input.len(), 1);
}