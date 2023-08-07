#[inline(always)]
pub fn inv_lerp(t: f32, min: f32, max: f32) -> f32 {
  (t - min) / (max - min)
}
#[inline(always)]
pub fn lerp(t: f32, min: f32, max: f32) -> f32 {
  t * (max - min) + min
}
#[inline(always)]
pub fn lerp_as(t: f32, tmin: f32, tmax: f32, min: f32, max: f32) -> f32 {
  lerp(inv_lerp(t, tmin, tmax), min, max)
}
#[inline(always)]
pub fn quadratic_interpolate_as(t: f32, tmin: f32, tmax: f32, min: f32, max: f32) -> f32 {
  lerp(inv_lerp(t, tmin, tmax).powi(2), min, max)
}

#[test]
fn test_lerp_as() {
  use plotters::prelude::*;
  const LEN: usize = 32;
  let root = BitMapBackend::new("output.png", (1024, 800)).into_drawing_area();
  root.fill(&WHITE).unwrap();
  let mut chart = ChartBuilder::on(&root)
    .x_label_area_size(35)
    .y_label_area_size(40)
    .margin(5)
    .build_cartesian_2d(0f32..LEN as f32 + 1f32, 0f32..5f32)
    .unwrap();
  chart.configure_mesh().y_desc("Amplitude").x_labels(7*5).x_max_light_lines(2).draw().unwrap();
  let space = (0..LEN).map(|i| i as f32);
  // ------------------------

  let mut buf = [0f32; LEN];
  let mut out = [0f32; LEN];
  let max = LEN as f32 / 10.0;
  for (i, v) in buf.iter_mut().enumerate() {
    *v = -(i as f32) * 0.1 + LEN as f32 / 10.0;
  }
  
  for (o, b) in out.iter_mut().zip(buf) {
    *o = lerp_as(b, max, 0.0, 2.0, 3.0);
  }
  chart
    .draw_series(LineSeries::new(
      space.clone().zip(buf),
      GREEN,
    ))
    .unwrap()
    .label(format!("y = 2.3x+1.2"))
    .legend(|(x, y)| PathElement::new(vec![(x, y), (x + 20, y)], GREEN));
  chart
    .draw_series(LineSeries::new(space.clone().zip(out), BLUE))
    .unwrap()
    .label(format!("y = x+2"))
    .legend(|(x, y)| PathElement::new(vec![(x, y), (x + 20, y)], BLUE));

  chart
  .configure_series_labels()
  .background_style(&RGBColor(128, 128, 128))
  .draw().unwrap();
  root.present().expect("Unable to write result to file, please make sure 'plotters-doc-data' dir exists under current dir");
}