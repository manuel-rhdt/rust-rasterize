mod box_filter;
mod dynamic_filter;

pub use self::box_filter::BoxFilter;
pub use self::dynamic_filter::DynamicFilter;

pub trait Filter {
    /// Returns the support of the filter
    fn support(&self) -> ((f32, f32), (f32, f32));
}

pub trait Evaluate<C> {
    // second return value is accumulator
    fn eval(&self, curve: C, filter_piece: (u32, u32)) -> (f32, f32);
}