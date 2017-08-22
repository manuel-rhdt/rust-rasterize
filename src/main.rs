extern crate image as img;
extern crate palette;
extern crate serde;
extern crate serde_json;
extern crate itertools;
extern crate arrayvec;
extern crate rmp_serde as rmps;
extern crate simd;
extern crate clap;
extern crate svgparser;
extern crate rayon;

#[macro_use]
extern crate serde_derive;

use clap::{Arg, App, ArgGroup};

pub mod rasterizer;
pub mod filter;
pub mod geometry;
mod svg;

use std::fs::File;
use std::io::prelude::*;

enum FilterType {
    BoxFilter(filter::BoxFilter),
    Dynamic(filter::DynamicFilter),
}

fn main() {
    let matches = App::new("svg-render")
        .version("0.1")
        .author("Manuel R.")
        .arg(
            Arg::with_name("named-filter")
                .short("f")
                .long("filter")
                .value_name("name")
                .help("Use one of the predefined filters")
                .possible_values(&["box", "lanczos"]),
        )
        .arg(
            Arg::with_name("file-filter")
                .short("c")
                .long("custom-filter")
                .value_name("path")
                .help("Load und use a custom filter from path"),
        )
        .group(
            ArgGroup::with_name("filter")
                .arg("named-filter")
                .arg("file-filter")
                .required(false),
        )
        .arg(
            Arg::with_name("input")
                .required(true)
                .help("SVG file to render or \"-\" to read from standard input")
                .index(1),
        )
        .arg(
            Arg::with_name("output")
                .required(true)
                .help("Output image file")
                .index(2),
        )
        .arg(
            Arg::with_name("dpi")
                .short("r")
                .long("resolution")
                .value_name("num")
                .default_value("300")
                .required(false)
                .help("Dots per inch of rasterization"),
        )
        .get_matches();


    let default_size = (800., 600.);

    let data = include_bytes!("../lanczos.json");
    let filter = match matches.value_of("named-filter") {
        Some("box") => FilterType::BoxFilter(filter::BoxFilter::new(1., 1.)),
        Some("lanczos") => FilterType::Dynamic(serde_json::from_slice(data).unwrap()),
        Some(_) => unreachable!(),
        // custom filter path provided
        None => {
            match matches.value_of("file-filter") {
                Some(path) => {
                    let file = File::open(path).unwrap();
                    FilterType::Dynamic(serde_json::from_reader(file).unwrap())
                }
                None => FilterType::BoxFilter(filter::BoxFilter::new(1., 1.)),
            }
        }
    };

    let input_path = matches.value_of("input").expect("No input");
    let mut input_file = File::open(input_path).unwrap();
    let mut svg = String::new();
    input_file.read_to_string(&mut svg).unwrap();

    let dpi = matches.value_of("dpi").expect("no dpi").parse().unwrap();

    let parsed_svg = svg::parse_str(&svg, dpi);
    let size = parsed_svg.size.unwrap_or(default_size);
    let size = (size.0 as usize, size.1 as usize);
    let curves = parsed_svg
        .paths
        .into_iter()
        .flat_map(|path| path.lines.into_iter())
        .collect::<Vec<_>>();

    let mut buffer = Vec::new();
    let viewport = geometry::Rect {
        origin: geometry::Point::origin(),
        size: geometry::Size {
            width: size.0 as f32,
            height: size.1 as f32,
        },
    };

    println!("{:?}", curves);

    match filter {
        FilterType::BoxFilter(filter) => {
            rasterizer::rasterize_parallel(viewport, &filter, &curves, &mut buffer)
        }
        FilterType::Dynamic(filter) => {
            rasterizer::rasterize_parallel(viewport, &filter, &curves, &mut buffer)
        }
    }

    let image_buffer = img::ImageBuffer::from_fn(size.0 as u32, size.1 as u32, |x, y| {
        let v = buffer[y as usize * size.0 + x as usize];
        let val = palette::Rgba::new(0.0, 0.0, 0.0, v);
        img::Rgba { data: palette::pixel::Srgb::linear_to_pixel(val) }
    });

    let output_path = matches.value_of("output").expect("No output");
    image_buffer.save(output_path).unwrap();
}
