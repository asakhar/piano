use num::Complex;
use plotters::{
  element::{Drawable, PointCollection},
  prelude::*,
};
use plotters_backend::{BackendCoord, BackendStyle, DrawingErrorKind};

pub struct RoundedRect<Coord> {
  pub points: [Coord; 2],
  pub radius: i32,
  pub segments: u32,
  pub style: ShapeStyle,
  pub margin: (u32, u32, u32, u32),
}

impl<Coord> RoundedRect<Coord> {
  pub fn new(points: [Coord; 2], radius: i32, style: impl Into<ShapeStyle>) -> Self {
    Self {
      points,
      radius,
      segments: 100,
      style: style.into(),
      margin: (0, 0, 0, 0),
    }
  }
}

impl<'a, Coord> PointCollection<'a, Coord> for &'a RoundedRect<Coord> {
  type Point = &'a Coord;
  type IntoIter = &'a [Coord];
  fn point_iter(self) -> &'a [Coord] {
    &self.points
  }
}

impl<Coord, DB: DrawingBackend> Drawable<DB> for RoundedRect<Coord> {
  fn draw<I: Iterator<Item = BackendCoord>>(
    &self,
    mut points: I,
    backend: &mut DB,
    _: (u32, u32),
  ) -> Result<(), DrawingErrorKind<DB::ErrorType>> {
    match (points.next(), points.next()) {
      (Some(a), Some(b)) => {
        let (mut a, mut b) = ((a.0.min(b.0), a.1.min(b.1)), (a.0.max(b.0), a.1.max(b.1)));
        a.1 += self.margin.0 as i32;
        b.1 -= self.margin.1 as i32;
        a.0 += self.margin.2 as i32;
        b.0 -= self.margin.3 as i32;
        let ara = (a.0 + self.radius, a.1);
        let bra = (b.0 - self.radius, a.1);
        let arb = (a.0 + self.radius, b.1);
        let brb = (b.0 - self.radius, b.1);
        let aar = (a.0, a.1 + self.radius);
        let abr = (a.0, b.1 - self.radius);
        let bar = (b.0, a.1 + self.radius);
        let bbr = (b.0, b.1 - self.radius);
        backend.draw_line(ara, bra, &self.style)?;
        backend.draw_line(arb, brb, &self.style)?;
        backend.draw_line(aar, abr, &self.style)?;
        backend.draw_line(bar, bbr, &self.style)?;
        let aa = (ara.0 + 1, aar.1);
        let ba = (bra.0, aar.1);
        let ab = (ara.0 + 1, abr.1);
        let bb = (bra.0, abr.1);

        draw_arc(backend, self.segments, aa, self.radius, 2, &self.style)?;
        draw_arc(backend, self.segments, ba, self.radius, 3, &self.style)?;
        draw_arc(backend, self.segments, ab, self.radius, 1, &self.style)?;
        draw_arc(backend, self.segments, bb, self.radius, 0, &self.style)?;
        Ok(())
      }
      _ => Ok(()),
    }
  }
}

pub struct SawIcon<Coord> {
  pub pos: [Coord; 1],
  pub size: i32,
  pub style: ShapeStyle,
}
impl<Coord> SawIcon<Coord> {
  pub fn new(pos: Coord, size: u32, style: impl Into<ShapeStyle>) -> Self {
    Self {
      pos: [pos],
      size: size as i32,
      style: style.into(),
    }
  }
}

impl<'a, Coord> PointCollection<'a, Coord> for &'a SawIcon<Coord> {
  type Point = &'a Coord;
  type IntoIter = &'a [Coord];
  fn point_iter(self) -> &'a [Coord] {
    &self.pos
  }
}

impl<Coord, DB: DrawingBackend> Drawable<DB> for SawIcon<Coord> {
  fn draw<I: Iterator<Item = BackendCoord>>(
    &self,
    mut points: I,
    backend: &mut DB,
    pd: (u32, u32),
  ) -> Result<(), DrawingErrorKind<DB::ErrorType>> {
    let Some(lt) = points.next() else {
      return Ok(());
    };
    let bbox = RoundedRect::new([lt, (lt.0 + self.size, lt.1 + self.size)], 5, self.style);
    bbox.draw(bbox.point_iter().iter().copied(), backend, pd)?;
    backend.draw_line(
      (lt.0 + self.size / 10, lt.1 + self.size / 10),
      (lt.0 + self.size / 10, lt.1 + 9 * self.size / 10),
      &self.style,
    )?;
    backend.draw_line(
      (lt.0 + self.size / 10, lt.1 + self.size / 10),
      (lt.0 + 5 * self.size / 10, lt.1 + 9 * self.size / 10),
      &self.style,
    )?;
    backend.draw_line(
      (lt.0 + 5 * self.size / 10, lt.1 + self.size / 10),
      (lt.0 + 5 * self.size / 10, lt.1 + 9 * self.size / 10),
      &self.style,
    )?;
    backend.draw_line(
      (lt.0 + 5 * self.size / 10, lt.1 + self.size / 10),
      (lt.0 + 9 * self.size / 10, lt.1 + 9 * self.size / 10),
      &self.style,
    )?;
    Ok(())
  }
}

pub struct TriangleIcon<Coord> {
  pub pos: [Coord; 1],
  pub size: i32,
  pub style: ShapeStyle,
}
impl<Coord> TriangleIcon<Coord> {
  pub fn new(pos: Coord, size: u32, style: impl Into<ShapeStyle>) -> Self {
    Self {
      pos: [pos],
      size: size as i32,
      style: style.into(),
    }
  }
}

impl<'a, Coord> PointCollection<'a, Coord> for &'a TriangleIcon<Coord> {
  type Point = &'a Coord;
  type IntoIter = &'a [Coord];
  fn point_iter(self) -> &'a [Coord] {
    &self.pos
  }
}

impl<Coord, DB: DrawingBackend> Drawable<DB> for TriangleIcon<Coord> {
  fn draw<I: Iterator<Item = BackendCoord>>(
    &self,
    mut points: I,
    backend: &mut DB,
    pd: (u32, u32),
  ) -> Result<(), DrawingErrorKind<DB::ErrorType>> {
    let Some(lt) = points.next() else {
      return Ok(());
    };
    let bbox = RoundedRect::new([lt, (lt.0 + self.size, lt.1 + self.size)], 5, self.style);
    bbox.draw(bbox.point_iter().iter().copied(), backend, pd)?;
    backend.draw_line(
      (lt.0 + 3 * self.size / 10, lt.1 + self.size / 10),
      (lt.0 + self.size / 10, lt.1 + 9 * self.size / 10),
      &self.style,
    )?;
    backend.draw_line(
      (lt.0 + 3 * self.size / 10, lt.1 + self.size / 10),
      (lt.0 + 5 * self.size / 10, lt.1 + 9 * self.size / 10),
      &self.style,
    )?;
    backend.draw_line(
      (lt.0 + 7 * self.size / 10, lt.1 + self.size / 10),
      (lt.0 + 5 * self.size / 10, lt.1 + 9 * self.size / 10),
      &self.style,
    )?;
    backend.draw_line(
      (lt.0 + 7 * self.size / 10, lt.1 + self.size / 10),
      (lt.0 + 9 * self.size / 10, lt.1 + 9 * self.size / 10),
      &self.style,
    )?;
    Ok(())
  }
}

pub struct SquareIcon<Coord> {
  pub pos: [Coord; 1],
  pub size: i32,
  pub style: ShapeStyle,
}
impl<Coord> SquareIcon<Coord> {
  pub fn new(pos: Coord, size: u32, style: impl Into<ShapeStyle>) -> Self {
    Self {
      pos: [pos],
      size: size as i32,
      style: style.into(),
    }
  }
}

impl<'a, Coord> PointCollection<'a, Coord> for &'a SquareIcon<Coord> {
  type Point = &'a Coord;
  type IntoIter = &'a [Coord];
  fn point_iter(self) -> &'a [Coord] {
    &self.pos
  }
}

impl<Coord, DB: DrawingBackend> Drawable<DB> for SquareIcon<Coord> {
  fn draw<I: Iterator<Item = BackendCoord>>(
    &self,
    mut points: I,
    backend: &mut DB,
    pd: (u32, u32),
  ) -> Result<(), DrawingErrorKind<DB::ErrorType>> {
    let Some(lt) = points.next() else {
      return Ok(());
    };
    let bbox = RoundedRect::new([lt, (lt.0 + self.size, lt.1 + self.size)], 5, self.style);
    bbox.draw(bbox.point_iter().iter().copied(), backend, pd)?;
    backend.draw_line(
      (lt.0 + 5 * self.size / 10, lt.1 + 9 * self.size / 10),
      (lt.0 + self.size / 10, lt.1 + 9 * self.size / 10),
      &self.style,
    )?;
    backend.draw_line(
      (lt.0 + 5 * self.size / 10, lt.1 + self.size / 10),
      (lt.0 + 5 * self.size / 10, lt.1 + 9 * self.size / 10),
      &self.style,
    )?;
    backend.draw_line(
      (lt.0 + 5 * self.size / 10, lt.1 + self.size / 10),
      (lt.0 + 9 * self.size / 10, lt.1 + self.size / 10),
      &self.style,
    )?;
    Ok(())
  }
}

pub struct SineIcon<Coord> {
  pub pos: [Coord; 1],
  pub size: i32,
  pub style: ShapeStyle,
}
impl<Coord> SineIcon<Coord> {
  pub fn new(pos: Coord, size: u32, style: impl Into<ShapeStyle>) -> Self {
    Self {
      pos: [pos],
      size: size as i32,
      style: style.into(),
    }
  }
}

impl<'a, Coord> PointCollection<'a, Coord> for &'a SineIcon<Coord> {
  type Point = &'a Coord;
  type IntoIter = &'a [Coord];
  fn point_iter(self) -> &'a [Coord] {
    &self.pos
  }
}

impl<Coord, DB: DrawingBackend> Drawable<DB> for SineIcon<Coord> {
  fn draw<I: Iterator<Item = BackendCoord>>(
    &self,
    mut points: I,
    backend: &mut DB,
    pd: (u32, u32),
  ) -> Result<(), DrawingErrorKind<DB::ErrorType>> {
    let Some(lt) = points.next() else {
      return Ok(());
    };
    let bbox = RoundedRect::new([lt, (lt.0 + self.size, lt.1 + self.size)], 5, self.style);
    bbox.draw(bbox.point_iter().iter().copied(), backend, pd)?;
    let segments = 100;
    let mut prev = 0;
    for i in 0..segments {
      let x = i as f32 / segments as f32 * 2.0 * std::f32::consts::PI;
      let h = ((x.sin() + 1.0) * self.size as f32 * 0.4) as i32;
      if i != 0 {
        backend.draw_line(
          (
            i * 8 * self.size / 10 / segments + lt.0 + self.size / 10,
            prev + lt.1 + self.size / 10,
          ),
          (
            (i + 1) * 8 * self.size / 10 / segments + lt.0 + self.size / 10,
            h + lt.1 + self.size / 10,
          ),
          &self.style,
        )?;
      }
      prev = h;
    }
    Ok(())
  }
}

fn draw_arc<DB: DrawingBackend>(
  backend: &mut DB,
  segments: u32,
  c: BackendCoord,
  r: i32,
  segment: i32,
  style: &impl BackendStyle,
) -> Result<(), DrawingErrorKind<DB::ErrorType>> {
  let r = r as f32;
  let c = Complex::new(c.0 as f32, c.1 as f32);
  for i in 0..segments {
    let prev =
      Complex::cis((i as f32 / segments as f32 + segment as f32) * std::f32::consts::PI / 2.0);
    let next = Complex::cis(
      ((i + 1) as f32 / segments as f32 + segment as f32) * std::f32::consts::PI / 2.0,
    );
    let prev_dn = prev * r + c;
    let next_dn = next * r + c;
    backend.draw_line(
      (prev_dn.re as i32, prev_dn.im as i32),
      (next_dn.re as i32, next_dn.im as i32),
      style,
    )?;
  }
  Ok(())
}
