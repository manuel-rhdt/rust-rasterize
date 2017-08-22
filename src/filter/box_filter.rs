use geometry::Line;
use super::{Filter, Evaluate};

#[derive(Debug, Copy, Clone, PartialEq)]
pub struct BoxFilter {
    support: ((f32, f32), (f32, f32)),
    area: f32,
}

impl BoxFilter {
    pub fn new(x: f32, y: f32) -> BoxFilter {
        let area = x * y;
        BoxFilter {
            support: ((-x/2.,x/2.), (-y/2.,y/2.)),
            area: area,
        }
    }
}

impl Filter for BoxFilter {
    fn support(&self) -> ((f32, f32), (f32, f32)) {
        self.support
    }
}

impl Evaluate<Line> for BoxFilter {
    fn eval(&self, line: Line, _: (u32, u32)) -> (f32, f32) {
        let accumulator = line.end.y - line.start.y;
        let pixel_value = 0.5 * accumulator * (line.end.x + line.start.x);
        (pixel_value / self.area, accumulator / self.area)
    }
}
