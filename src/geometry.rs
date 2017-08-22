use std::ops::{Add, Sub, Mul, Div, Neg};
use std::convert::From;

#[derive(Debug, Copy, Clone, PartialEq, Default)]
pub struct Point {
    pub x: f32,
    pub y: f32,
}

impl Sub<Point> for Point {
    type Output = Vec2d;
    fn sub(self, rhs: Point) -> Vec2d {
        Vec2d {
            x: self.x - rhs.x,
            y: self.y - rhs.y,
        }
    }
}

impl Mul<f32> for Point {
    type Output = Point;
    fn mul(self, rhs: f32) -> Point {
        Point {
            x: self.x * rhs,
            y: self.y * rhs,
        }
    }
}

impl Point {
    pub fn origin() -> Point {
        Point::new(0., 0.)
    }

    pub fn new(x: f32, y: f32) -> Point {
        Point { x: x, y: y }
    }

    pub fn vec_from_origin(self) -> Vec2d {
        self - Point::origin()
    }

    pub fn offset(self, delta_x: f32, delta_y: f32) -> Point {
        let delta_vec = Vec2d::new(delta_x, delta_y);
        self + delta_vec
    }
}

#[derive(Debug, Copy, Clone, Default, PartialEq)]
pub struct Vec2d {
    pub x: f32,
    pub y: f32,
}

impl Vec2d {
    pub fn new(x: f32, y: f32) -> Vec2d {
        Vec2d { x: x, y: y }
    }

    pub fn orth(self) -> Vec2d {
        Vec2d::new(self.y, -self.x)
    }

    pub fn norm(self) -> f32 {
        (self.x * self.x + self.y * self.y).sqrt()
    }
}

impl Add<Vec2d> for Vec2d {
    type Output = Vec2d;
    fn add(self, rhs: Vec2d) -> Vec2d {
        Vec2d {
            x: self.x + rhs.x,
            y: self.y + rhs.y,
        }
    }
}

impl Sub<Vec2d> for Vec2d {
    type Output = Vec2d;
    fn sub(self, rhs: Vec2d) -> Vec2d {
        Vec2d {
            x: self.x - rhs.x,
            y: self.y - rhs.y,
        }
    }
}

impl Add<Vec2d> for Point {
    type Output = Point;
    fn add(self, rhs: Vec2d) -> Point {
        Point {
            x: self.x + rhs.x,
            y: self.y + rhs.y,
        }
    }
}

impl Sub<Vec2d> for Point {
    type Output = Point;
    fn sub(self, rhs: Vec2d) -> Point {
        Point {
            x: self.x - rhs.x,
            y: self.y - rhs.y,
        }
    }
}

impl Neg for Vec2d {
    type Output = Vec2d;
    fn neg(self) -> Vec2d {
        Vec2d {
            x: -self.x,
            y: -self.y,
        }
    }
}

impl Mul<f32> for Vec2d {
    type Output = Vec2d;
    fn mul(self, rhs: f32) -> Vec2d {
        Vec2d {
            x: self.x * rhs,
            y: self.y * rhs,
        }
    }
}

impl Div<f32> for Vec2d {
    type Output = Vec2d;
    fn div(self, rhs: f32) -> Vec2d {
        Vec2d {
            x: self.x / rhs,
            y: self.y / rhs,
        }
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Default)]
pub struct Line {
    pub start: Point,
    pub end: Point,
}

impl Line {
    pub fn new(start: Point, end: Point) -> Line {
        Line {
            start: start,
            end: end,
        }
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Default)]
pub struct QuadraticBezier {
    pub start: Point,
    pub control: Point,
    pub end: Point,
}

impl QuadraticBezier {
    pub fn new(start: Point, control: Point, end: Point) -> QuadraticBezier {
        QuadraticBezier {
            start: start,
            control: control,
            end: end,
        }
    }
}

#[derive(Debug, Copy, Clone, PartialEq)]
pub struct Size {
    pub width: f32,
    pub height: f32,
}

#[derive(Debug, Copy, Clone, PartialEq)]
pub struct Rect {
    pub origin: Point,
    pub size: Size,
}

impl Rect {
    pub fn new(x: f32, y: f32, width: f32, height: f32) -> Rect {
        Rect {
            origin: Point { x: x, y: y },
            size: Size {
                width: width,
                height: height,
            },
        }
    }

    /// Normalizes a rect to contain no negative width/height.
    ///
    /// A normalized rect always has its origin at the top left.
    ///
    /// # Examples
    ///
    /// ```
    /// use rasterization::geometry::Rect;
    ///
    /// // A rect with positive width and height remains unchanged:
    /// let rect = Rect::new(0., 1., 2., 3.);
    /// assert_eq!(rect, rect.normalize());
    ///
    /// // Same rect expressed with a negative width:
    /// let rect2 = Rect::new(2., 1., -2., 3.);
    /// assert_eq!(rect, rect2.normalize());
    ///
    /// // negative height:
    /// let rect3 = Rect::new(0., 3., 2., -3.);
    /// assert_eq!(rect, rect3.normalize());
    ///
    /// // both width and height negative:
    /// let rect4 = Rect::new(2., 3., -2., -3.);
    /// assert_eq!(rect, rect4.normalize());
    /// ```
    ///
    pub fn normalize(self) -> Rect {
        let mut rect = self;
        if rect.size.width < 0.0 {
            rect.origin.x += rect.size.width;
            rect.size.width = -rect.size.width;
        }

        if rect.size.height < 0.0 {
            rect.origin.y += rect.size.height;
            rect.size.height = -rect.size.height;
        }

        rect
    }

    pub fn top_left(self) -> Point {
        self.normalize().origin
    }

    pub fn top_right(self) -> Point {
        let rect = self.normalize();
        rect.origin.offset(rect.size.width, 0.0)
    }

    pub fn bottom_left(self) -> Point {
        let rect = self.normalize();
        rect.origin.offset(0.0, rect.size.height)
    }

    pub fn bottom_right(self) -> Point {
        let rect = self.normalize();
        rect.origin.offset(rect.size.width, rect.size.height)
    }

    pub fn is_inside(self, point: Point) -> bool {
        let rect = self.normalize();
        point.x >= rect.top_left().x && point.x <= rect.top_right().x &&
            point.y >= rect.top_left().y && point.y <= rect.bottom_left().y
    }

    pub fn intersects(self, other: Rect) -> bool {
        self.is_inside(other.top_left()) || self.is_inside(other.top_right()) ||
            self.is_inside(other.bottom_left()) || self.is_inside(other.bottom_right())
    }

    pub fn intersects_pixel(self, pixel: (usize, usize)) -> bool {
        let other = Rect::new(pixel.0 as f32, pixel.1 as f32, 1.0, 1.0);
        self.intersects(other)
    }
}

pub struct ImageSize {
    pub width: usize,
    pub height: usize,
}

impl From<Size> for ImageSize {
    fn from(size: Size) -> ImageSize {
        ImageSize {
            width: size.width as usize,
            height: size.height as usize,
        }
    }
}