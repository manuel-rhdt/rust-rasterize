use serde::Deserialize;
use simd::f32x4;

use std::ops::MulAssign;

use geometry::Line;
use super::{Filter, Evaluate};

trait PowerLookup: Copy + Clone + MulAssign<Self> {
    type Index: Copy + Clone + Default + ::std::fmt::Debug;
    type Output: EvaluateMultinomial;

    fn identity() -> Self;
    fn power_lookup_4x(table: &[Self], index: [Self::Index; 4]) -> Self::Output;
}

trait EvaluateMultinomial {
    fn fold(self, coeffs: f32x4) -> f32x4;
}

#[derive(Copy, Clone, PartialEq, Debug)]
struct ParametricLine {
    origin: [f32; 2],
    vector: [f32; 2],
}

impl MulAssign<ParametricLine> for ParametricLine {
    fn mul_assign(&mut self, other: ParametricLine) {
        self.origin[0] *= other.origin[0];
        self.origin[1] *= other.origin[1];
        self.vector[0] *= other.vector[0];
        self.vector[1] *= other.vector[1];
    }
}

impl PowerLookup for ParametricLine {
    type Index = [u8; 4];
    type Output = [[f32; 4]; 4];

    fn identity() -> Self {
        ParametricLine {
            origin: [1., 1.],
            vector: [1., 1.],
        }
    }

    fn power_lookup_4x(table: &[Self], index_mat: [Self::Index; 4]) -> Self::Output {
        let mut result: [[f32; 4]; 4] = [[0.; 4]; 4];

        for i in 0..4 {
            let mut row: [f32; 4] = [0.; 4];
            for (j, index_vec) in index_mat.iter().enumerate() {
                let index = index_vec[i] as usize;
                row[j] = match i {
                    0 => table[index].origin[0],
                    1 => table[index].origin[1],
                    2 => table[index].vector[0],
                    3 => table[index].vector[1],
                    _ => unreachable!(),
                };
            }

            result[i] = row;
        }

        result
    }
}

impl EvaluateMultinomial for [[f32; 4]; 4] {
    #[inline(always)]
    fn fold(self, mut coeffs: f32x4) -> f32x4 {
        for i in 0..4 {
            let simd_vec = f32x4::load(&self[i], 0);
            coeffs = coeffs * simd_vec;
        }
        coeffs
    }
}

#[derive(Debug, Deserialize, Clone)]
pub struct DynamicFilter {
    name: String,
    support: ((f32, f32), (f32, f32)),
    normalization: f32,
    line_tiles: Option<TileSet<ParametricLine>>,
}

#[serde(bound = "Tile<T>: Deserialize<'de>")]
#[derive(Debug, Deserialize, Clone)]
struct TileSet<T: PowerLookup>(Vec<Vec<Tile<T>>>);

impl<T: PowerLookup> TileSet<T> {
    /// Evaluates the specified tile using supplied values.
    fn evaluate_tile(&self, tile: (u32, u32), values: T) -> f32 {
        let tile = &self.0[tile.1 as usize][tile.0 as usize];

        let lookup_4x_table = PowersLookupTable::new(values, tile.max_pow);
        tile.evaluate(&lookup_4x_table)
    }
}

#[serde(bound = "T::Index: Deserialize<'de>")]
#[derive(Debug, Deserialize, Clone)]
struct Tile<T: PowerLookup> {
    coefficients: Vec<f32>,
    powers: Vec<T::Index>,
    max_pow: u8,
}

impl<T: PowerLookup> Tile<T> {
    fn evaluate(&self, lookup_4x_table: &PowersLookupTable<T>) -> f32 {
        let mut result = f32x4::splat(0.0);

        let iter = self.coefficients.chunks(4).zip(self.powers.chunks(4));
        for (coeff_vec, powers_mat) in iter {
            let pmat;
            let cvec;
            if coeff_vec.len() < 4 {
                pmat = [
                    powers_mat.get(0).cloned().unwrap_or_default(),
                    powers_mat.get(1).cloned().unwrap_or_default(),
                    powers_mat.get(2).cloned().unwrap_or_default(),
                    powers_mat.get(3).cloned().unwrap_or_default(),
                ];
                let cvec_arr = [
                    coeff_vec.get(0).cloned().unwrap_or_default(),
                    coeff_vec.get(1).cloned().unwrap_or_default(),
                    coeff_vec.get(2).cloned().unwrap_or_default(),
                    coeff_vec.get(3).cloned().unwrap_or_default(),
                ];
                cvec = f32x4::load(&cvec_arr, 0);
            } else {
                pmat = [powers_mat[0], powers_mat[1], powers_mat[2], powers_mat[3]];
                // cvec = [coeff_vec[0], coeff_vec[1], coeff_vec[2], coeff_vec[3]];
                cvec = f32x4::load(&coeff_vec, 0);
            }
            let lookup_4x_mat = lookup_4x_table.lookup_4x(pmat);
            result = result + lookup_4x_mat.fold(cvec);
        }

        let mut sum = result.extract(0);
        for i in 1..4 {
            sum += result.extract(i);
        }
        sum
    }
}

#[derive(Debug, Clone)]
struct PowersLookupTable<T> {
    table: Vec<T>,
}

impl<T: PowerLookup> PowersLookupTable<T> {
    fn new(values: T, up_to_power: u8) -> Self {
        let mut table = Vec::with_capacity((up_to_power + 1) as usize);
        table.push(T::identity());
        let mut accumulator = values;
        table.push(accumulator);
        for _ in 0..up_to_power - 1 {
            accumulator *= values;
            table.push(accumulator);
        }
        PowersLookupTable { table: table }
    }

    #[inline(always)]
    fn lookup_4x(&self, pow: [T::Index; 4]) -> T::Output {
        T::power_lookup_4x(&self.table, pow)
    }
}

impl Filter for DynamicFilter {
    fn support(&self) -> ((f32, f32), (f32, f32)) {
        self.support
    }
}

impl Evaluate<Line> for DynamicFilter {
    fn eval(&self, line: Line, piece: (u32, u32)) -> (f32, f32) {
        let line_tileset = self.line_tiles.as_ref().expect(
            "This filter cannot rasterize Lines.",
        );

        let mut par_line = ParametricLine {
            origin: [line.start.x, line.start.y],
            vector: [(line.end.x - line.start.x), (line.end.y - line.start.y)],
        };
        let pixel_value = line_tileset.evaluate_tile(piece, par_line);
        par_line.origin[0] = 1.0;
        par_line.vector[0] = 0.0;
        let accumulator = line_tileset.evaluate_tile(piece, par_line);

        (
            pixel_value * self.normalization,
            accumulator * self.normalization,
        )
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use geometry::Point;

    use rmps;

    const EPS: f32 = 1.0e-5;

    // #[test]
    // fn test_tile_eval() {
    //     let data = include_bytes!("../../lanczos.filter");
    //     let lanczos: DynamicFilter = rmps::from_slice(data).unwrap();

    //     let line_tileset = lanczos.line_tiles.unwrap();
    //     let result = line_tileset.evaluate_tile((4, 2), &[1.5, -1., 0., 1.]);
    //     assert!((result - 0.4474546287339809).abs() < EPS);
    // }

    // #[test]
    // fn test_curve_integral() {
    //     let line = Line {
    //         start: Point::new(0.5, 0.),
    //         end: Point::new(0.5, 1.),
    //     };

    //     let data = include_bytes!("../../lanczos.filter");
    //     let lanczos: DynamicFilter = rmps::from_slice(data).unwrap();

    //     let (pv, acc) = lanczos.eval(line, (4, 2));
    //     println!("{:?}", (pv, acc));
    //     assert!((pv - 0.44745462873398173).abs() < EPS);
    //     assert!((acc - 0.42379986695582544).abs() < EPS);


    //     let line = Line {
    //         start: Point::new(0.5, 1.),
    //         end: Point::new(0.5, 0.),
    //     };

    //     let (pv, acc) = lanczos.eval(line, (4, 2));
    //     println!("{:?}", (pv, acc));
    //     assert!((pv + 0.44745462873398173).abs() < EPS);
    //     assert!((acc + 0.42379986695582544).abs() < EPS);
    // }

    use itertools::Itertools;

    #[test]
    fn test_power_table() {
        let line = ParametricLine {
            origin: [1.0, 2.0],
            vector: [3.0, 4.0],
        };

        let pow_tab = PowersLookupTable::new(line, 5);

        let index_mat = [[0, 1, 2, 3], [3, 2, 1, 0], [3, 2, 4, 5], [5, 4, 2, 3]];

        let powers_mat = pow_tab.lookup_4x(index_mat);

        println!("{:?}", pow_tab);
        println!("{:?}", powers_mat);

        for i in 0..index_mat.len() {
            println!("i = {:?}", i);
            assert!((line.origin[0].powi(index_mat[i][0] as i32) - powers_mat[0][i]) < EPS);
            assert!((line.origin[1].powi(index_mat[i][1] as i32) - powers_mat[1][i]) < EPS);
            assert!((line.vector[0].powi(index_mat[i][2] as i32) - powers_mat[2][i]) < EPS);
            assert!((line.vector[1].powi(index_mat[i][3] as i32) - powers_mat[3][i]) < EPS);
        }
    }
}
