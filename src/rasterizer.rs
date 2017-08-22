use arrayvec::ArrayVec;
use rayon::prelude::*;

use geometry::{Rect, Line, ImageSize, Point, Size, Vec2d};
use filter::{Filter, Evaluate};

use std::sync::Mutex;

pub trait Curve: Sized {
    type ClipIter: Iterator<Item = Self>;

    fn bounding_box(&self) -> Rect;
    fn clip_to_rect(&self, rect: Rect) -> Self::ClipIter;
    fn offset(&self, offset: Vec2d) -> Self;
}

impl Curve for Line {
    type ClipIter = ::std::option::IntoIter<Self>;

    fn bounding_box(&self) -> Rect {
        Rect::new(
            self.start.x,
            self.start.y,
            self.end.x - self.start.x,
            self.end.y - self.start.y,
        )
    }

    // implementation of the Liang-Barski algorithm
    #[inline(always)]
    fn clip_to_rect(&self, rect: Rect) -> Self::ClipIter {
        let delta_x = self.end.x - self.start.x;
        let delta_y = self.end.y - self.start.y;
        let xmin = rect.origin.x;
        let xmax = rect.origin.x + rect.size.width;
        let ymin = rect.origin.y;
        let ymax = rect.origin.y + rect.size.height;

        let mut lt_zero: ArrayVec<[f32; 3]> = ArrayVec::new();
        let mut gt_zero: ArrayVec<[f32; 3]> = ArrayVec::new();
        let pq_iterator = (0..4).map(|i| match i {
            0 => (-delta_x, self.start.x - xmin),
            1 => (delta_x, xmax - self.start.x),
            2 => (-delta_y, self.start.y - ymin),
            3 => (delta_y, ymax - self.start.y),
            _ => unreachable!(),
        });

        for (p, q) in pq_iterator {
            if p < 0.0 {
                lt_zero.push(q / p);
                continue;
            }
            if p > 0.0 {
                gt_zero.push(q / p);
                continue;
            }
            if p == 0.0 && q < 0.0 {
                return None.into_iter();
            }
        }

        lt_zero.push(0.0);
        gt_zero.push(1.0);

        let t1 = *lt_zero
            .iter()
            .max_by(|a, b| a.partial_cmp(b).unwrap())
            .unwrap();
        let t2 = *gt_zero
            .iter()
            .min_by(|a, b| a.partial_cmp(b).unwrap())
            .unwrap();

        if t1 > t2 {
            return None.into_iter();
        }

        let line = Line {
            start: self.start + (self.end - self.start) * t1,
            end: self.start + (self.end - self.start) * t2,
        };

        if line.start.x == 1.0 && line.end.x == 1.0 || line.start.y == 1.0 && line.end.y == 1.0 {
            return None.into_iter();
        }

        Some(line).into_iter()
    }

    fn offset(&self, offset: Vec2d) -> Line {
        Line {
            start: self.start + offset,
            end: self.end + offset,
        }
    }
}

/// Stores the curves for each scanline
struct ScanlineTable<C> {
    /// Stores indices to curves in `curves` that start at each scanline.
    buckets: Vec<usize>,
    /// Heap storage for the curves.
    curves: Vec<C>,
}

// TODO: currently works only for lines
fn cut_curves<C>(viewport: Rect, curves: &[C]) -> Vec<Vec<C>>
where
    C: Curve + Send + Sync,
{
    const PIXEL_RECT: Rect = Rect {
        origin: Point { x: 0., y: 0. },
        size: Size {
            width: 1.,
            height: 1.,
        },
    };

    let size: ImageSize = viewport.size.into();
    (0..size.width * size.height)
        .into_par_iter()
        .map(move |index| {
            let row = index / size.width;
            let col = index % size.width;

            let pixel_origin = viewport.origin + Vec2d::new(col as f32, row as f32);
            curves
                .iter()
                .flat_map(|curve| {
                    curve.offset(-pixel_origin.vec_from_origin()).clip_to_rect(
                        PIXEL_RECT,
                    )
                })
                .collect()
        })
        .collect()
}

/// Rasterize the given `curves` using the filter `filter` and put the pixel values into `buffer`.
///
/// This uses rayon for parallelization where each filter piece and each scanline are evaluated in
/// parallel.
///
/// # Parameters
/// - `viewport`: The portion of the vector image that should be rendered
pub fn rasterize_parallel<Flt, C>(viewport: Rect, filter: &Flt, curves: &[C], buffer: &mut Vec<f32>)
where
    Flt: Filter + Evaluate<C> + Sync,
    C: Curve + Clone + Send + Sync + ::std::fmt::Debug,
{
    let viewport = viewport.normalize();
    let size: ImageSize = viewport.size.into();

    // prepare the image buffer
    buffer.clear();
    buffer.reserve(size.width * size.height);
    for _ in 0..(size.width * size.height) {
        buffer.push(0.0);
    }

    // Pixel Grid (pixels centers "+" are always in between whole coordinates)
    //      0.5 1.5
    //   0 |---|---|
    //     | + | + | 0.5
    //   1 |---|---|
    //     | + | + | 1.5
    //   2 |---|---|
    //     0   1   2

    let (support_x, support_y) = filter.support();
    let x_filt_pieces = (support_x.1 - support_x.0) as usize;
    let y_filt_pieces = (support_y.1 - support_y.0) as usize;

    println!("cutting curves");
    let curves_viewport = Rect {
        origin: viewport.origin +
            Vec2d {
                x: support_x.0 + 0.5,
                y: support_y.0 + 0.5,
            },
        size: Size {
            width: viewport.size.width + x_filt_pieces as f32 - 1.,
            height: viewport.size.height + y_filt_pieces as f32 - 1.,
        },
    };
    let curves = cut_curves(curves_viewport, curves);
    println!("done");

    // Create a Mutex for each scanline, so they can be filled independently.
    let scanline_buffers = buffer
        .chunks_mut(size.width)
        .map(Mutex::new)
        .collect::<Vec<_>>();

    (0..x_filt_pieces * y_filt_pieces)
        .into_par_iter()
        .map(|filter_index| {
            let filter_piece_x = filter_index / y_filt_pieces;
            let filter_piece_y = filter_index % y_filt_pieces;
            (filter_piece_x as u32, filter_piece_y as u32)
        })
        .flat_map(|fp| {
            scanline_buffers.par_iter().enumerate().map(
                move |(sl, ch)| {
                    (fp, sl, ch)
                },
            )
        })
        .for_each(|(filter_piece, scanline, chunk)| {
            let mut accumulator = 0.0;
            let mut chunk = chunk.lock().unwrap();

            // inner rendering loop
            for column in (0..size.width).rev() {
                let curve_index_x = column + filter_piece.0 as usize;
                let curve_index_y = scanline + filter_piece.1 as usize;
                let curve_index = curve_index_y * (size.width + x_filt_pieces - 1) + curve_index_x;

                let mut pixel_value = accumulator;
                for (pv, acc) in curves[curve_index].iter().cloned().map(|curve| {
                    filter.eval(curve, filter_piece)
                })
                {
                    pixel_value += pv;
                    accumulator += acc;
                }

                chunk[column] += pixel_value;
            }
        });
}
