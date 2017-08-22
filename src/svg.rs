use svgparser;
use svgparser::{AttributeId, ElementId, Length, LengthUnit, Tokenize, TextFrame};
use svgparser::svg::{ElementEnd, Tokenizer, Token};

use geometry::{Line, Point, Vec2d};

#[derive(Debug, Default)]
pub struct VectorGraphic {
    pub paths: Vec<Path>,
    pub size: Option<(f32, f32)>,
}

#[derive(Debug, Default)]
pub struct Path {
    pub lines: Vec<Line>,
}

#[derive(Debug, Default)]
struct SvgRootMachine {
    width: Option<f32>,
    height: Option<f32>,
}

enum AttributeValue<'a> {
    Number(f32),
    NumberList(&'a mut Iterator<Item = f32>),
    Other(svgparser::AttributeValue<'a>),
}

impl SvgRootMachine {
    fn new() -> Self {
        SvgRootMachine::default()
    }

    fn attribute(&mut self, id: AttributeId, val: AttributeValue) {
        let val = match val {
            AttributeValue::Number(num) => num as f32,
            _ => return,
        };
        match id {
            AttributeId::Width => self.width = Some(val),
            AttributeId::Height => self.height = Some(val),
            _ => {}
        }
    }

    fn complete(self) -> Option<(f32, f32)> {
        println!("svg machine {:?}", self);
        match (self.width, self.height) {
            (Some(width), Some(height)) => Some((width, height)),
            _ => None,
        }
    }
}

#[derive(Debug, Default)]
struct LineMachine {
    x1: Option<f32>,
    y1: Option<f32>,
    x2: Option<f32>,
    y2: Option<f32>,
    width: Option<f32>,
}

impl LineMachine {
    fn new() -> Self {
        LineMachine::default()
    }

    fn attribute(&mut self, id: AttributeId, val: AttributeValue) {
        let val = match val {
            AttributeValue::Number(num) => num as f32,
            _ => return,
        };
        match id {
            AttributeId::X1 => self.x1 = Some(val),
            AttributeId::X2 => self.x2 = Some(val),
            AttributeId::Y1 => self.y1 = Some(val),
            AttributeId::Y2 => self.y2 = Some(val),
            AttributeId::StrokeWidth => self.width = Some(val),
            _ => {}
        };
    }

    fn complete(self, lines: &mut Vec<Line>) {
        let x1 = match self.x1 {
            Some(val) => val,
            None => return,
        };
        let y1 = match self.y1 {
            Some(val) => val,
            None => return,
        };
        let x2 = match self.x2 {
            Some(val) => val,
            None => return,
        };
        let y2 = match self.y2 {
            Some(val) => val,
            None => return,
        };
        let width = self.width.unwrap_or(1.);

        let v_orth = Vec2d::new(x2 - x1, y2 - y1).orth();
        let v_orth_n = v_orth / v_orth.norm();

        let start = Point::new(x1, y1);
        let end = Point::new(x2, y2);

        let p1 = start + v_orth_n * width / 2.;
        let p2 = end + v_orth_n * width / 2.;
        let p3 = end - v_orth_n * width / 2.;
        let p4 = start - v_orth_n * width / 2.;

        lines.push(Line::new(p1, p2));
        lines.push(Line::new(p2, p3));
        lines.push(Line::new(p3, p4));
        lines.push(Line::new(p4, p1));
    }
}

#[derive(Debug)]
struct PolygonMachine {
    pts: Vec<Point>,
}

impl PolygonMachine {
    fn new() -> PolygonMachine {
        PolygonMachine { pts: Vec::new() }
    }

    fn attribute(&mut self, id: AttributeId, val: AttributeValue) {
        match id {
            AttributeId::Points => {
                self.pts.clear();
                if let AttributeValue::NumberList(list) = val {
                    let mut tmp = None;
                    for num in list.into_iter() {
                        let num = num;
                        tmp = match tmp {
                            None => Some(num),
                            Some(x) => {
                                self.pts.push(Point::new(x, num));
                                None
                            }
                        }
                    }
                } else {
                    panic!()
                }
            }
            _ => {}
        }
    }

    fn complete(self, lines: &mut Vec<Line>) {
        if self.pts.len() < 2 {
            return;
        }

        let mut iter = self.pts.into_iter().peekable();
        loop {
            let pt = match iter.next() {
                Some(el) => el,
                None => break,
            };
            let next_pt = match iter.peek() {
                Some(expr) => expr,
                None => break,
            };
            lines.push(Line::new(pt, *next_pt));
        }
    }
}

#[derive(Debug, Default)]
struct Parser {
    result: VectorGraphic,
    stack: Vec<ParserState>,
    dpi: f32,
}

impl Parser {
    fn element_start(&mut self, id: ElementId) {
        let elem = match id {
            ElementId::Svg => Element::Svg(SvgRootMachine::new()),
            ElementId::Line => Element::Line(LineMachine::new()),
            ElementId::Polygon => Element::Polygon(PolygonMachine::new()),
            _ => return,
        };
        self.state().elem = Some(elem);
    }

    fn attribute(&mut self, id: AttributeId, val: TextFrame) {
        let dpi = self.dpi;
        self.state().attribute(id, val, dpi)
    }

    fn element_end(&mut self, end: ElementEnd) {
        let current_state = self.stack.last_mut().unwrap();
        match current_state.elem.take() {
            None => {}
            Some(Element::Svg(mach)) => {
                self.result.size = mach.complete();
            }
            Some(Element::Line(mach)) => {
                let mut new_path = Vec::with_capacity(4);
                mach.complete(&mut new_path);
                self.result.paths.push(Path { lines: new_path });
            }
            Some(Element::Polygon(mach)) => {
                let mut new_path = Vec::new();
                mach.complete(&mut new_path);
                self.result.paths.push(Path { lines: new_path });
            }
        };
    }

    fn state(&mut self) -> &mut ParserState {
        self.stack.last_mut().unwrap()
    }
}

#[derive(Debug)]
struct ParserState {
    /// The element the parser is currently processing, if any.
    elem: Option<Element>,
}

impl ParserState {
    fn attribute(&mut self, attr_id: AttributeId, val: TextFrame, dpi: f32) {
        let elem_id = match self.elem {
            Some(ref elem) => elem.element_id(),
            None => return,
        };
        let val = svgparser::AttributeValue::from_frame(elem_id, attr_id, val).unwrap();
        match val {
            svgparser::AttributeValue::Number(num) => {
                let val = AttributeValue::Number(num as f32);
                self.elem.as_mut().unwrap().svg_attribute(attr_id, val)
            }
            svgparser::AttributeValue::NumberList(numbers) => {
                let mut iter = numbers.map(|x| x.unwrap() as f32);
                let val = AttributeValue::NumberList(&mut iter);
                self.elem.as_mut().unwrap().svg_attribute(attr_id, val)
            }
            svgparser::AttributeValue::Length(Length { num, unit }) => {
                let unit = match unit {
                    LengthUnit::In => dpi,
                    LengthUnit::Cm => dpi * 0.39,
                    LengthUnit::Mm => dpi * 0.039,
                    LengthUnit::Pt => dpi / 72.,
                    LengthUnit::Pc => dpi / 6.,
                    LengthUnit::None => 1.,
                    _ => unimplemented!(),
                };
                let val = AttributeValue::Number(num as f32 * unit);
                self.elem.as_mut().unwrap().svg_attribute(attr_id, val)
            }
            other => {
                let val = AttributeValue::Other(other);
                self.elem.as_mut().unwrap().svg_attribute(attr_id, val)
            }
        };
    }
}

#[derive(Debug)]
enum Element {
    Svg(SvgRootMachine),
    Line(LineMachine),
    Polygon(PolygonMachine),
}

impl Element {
    fn svg_attribute(&mut self, id: AttributeId, val: AttributeValue) {
        match *self {
            Element::Svg(ref mut svg_machine) => svg_machine.attribute(id, val),
            Element::Line(ref mut line_machine) => line_machine.attribute(id, val),
            Element::Polygon(ref mut polygon_machine) => polygon_machine.attribute(id, val),
        }
    }

    fn element_id(&self) -> ElementId {
        match *self {
            Element::Svg(_) => ElementId::Svg,
            Element::Line(_) => ElementId::Line,
            Element::Polygon(_) => ElementId::Polygon,
        }
    }
}

pub fn parse_str(svg: &str, dpi: f32) -> VectorGraphic {
    let mut tokenizer = Tokenizer::from_str(svg);

    let mut parser = Parser::default();
    parser.dpi = dpi;
    parser.stack.push(ParserState { elem: None });

    loop {
        match tokenizer.parse_next().unwrap() {
            Token::SvgElementStart(id) => parser.element_start(id),
            Token::ElementEnd(end) => parser.element_end(end),
            Token::SvgAttribute(id, val) => parser.attribute(id, val),
            Token::EndOfStream => break,
            _ => {}
        };
    }

    parser.result
}