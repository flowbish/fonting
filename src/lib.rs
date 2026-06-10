use allsorts::binary::read::ReadScope;
use allsorts::tables::cpal::{CpalTable, ColorRecord};
use allsorts::binary::read::{ReadBinary};

use allsorts::tag::{CPAL};
use allsorts::tables::{FontTableProvider};
use allsorts::font_data::FontData;
use wasm_bindgen::prelude::*;

use nom::{
    IResult,
    branch::alt,
    combinator::{cut},
    bytes::complete::tag,
    number::complete::float,
    character::complete::{multispace1, char},
    sequence::{delimited, preceded, terminated},
    error::Error,
    combinator::{map},
    multi::separated_list1,
    Parser,
};

#[wasm_bindgen]
extern "C" {
    pub fn alert(s: &str);
}

#[wasm_bindgen]
pub fn greet(s: &str) {
    alert(&format!("HELLO {s}"));
}

trait Filter {
    fn filter(&self, color: ColorRecord) -> ColorRecord;
}

struct Invert(f32);

impl Filter for Invert {
    fn filter(&self, color: ColorRecord) -> ColorRecord {
        let factor = 1f32 - (self.0 * 2f32);
        let red = (0x7f as f32 + ((color.red as f32 - 0x7f as f32) * factor)) as u8;
        let green = (0x7f as f32 + ((color.green as f32 - 0x7f as f32) as f32 * factor)) as u8;
        let blue = (0x7f as f32 + ((color.blue as f32 - 0x7f as f32) as f32 * factor)) as u8;
        let alpha = color.alpha;
        ColorRecord { red, green, blue, alpha }
    }
}

struct Contrast(f32);

impl Filter for Contrast {
    fn filter(&self, color: ColorRecord) -> ColorRecord {
        let factor = self.0;
        let red = ((color.red as f32 * factor) + (255.0 * (1.0 - factor) / 2.0)) as u8;
        let green = ((color.green as f32 * factor) + (255.0 * (1.0 - factor) / 2.0)) as u8;
        let blue = ((color.blue as f32 * factor) + (255.0 * (1.0 - factor) / 2.0)) as u8;
        let alpha = color.alpha;
        ColorRecord { red, green, blue, alpha }
    }
}

struct Brightness(f32);

impl Filter for Brightness {
    fn filter(&self, color: ColorRecord) -> ColorRecord {
        let factor = self.0;
        let red = (color.red as f32 * factor) as u8;
        let green = (color.green as f32 * factor) as u8;
        let blue = (color.blue as f32 * factor) as u8;
        let alpha = color.alpha;
        ColorRecord { red, green, blue, alpha }
    }
}

struct Opacity(f32);

impl Filter for Opacity {
    fn filter(&self, color: ColorRecord) -> ColorRecord {
        let factor = self.0;
        let alpha = (color.alpha as f32 * factor) as u8;
        ColorRecord { red: color.red, green: color.green, blue: color.blue, alpha }
    }
}


struct Saturate(f32);

impl Filter for Saturate {
    fn filter(&self, color: ColorRecord) -> ColorRecord {
        let s = self.0;
        let (r, g, b) = (color.red as f32, color.green as f32, color.blue as f32);
        let red = (0.213 + 0.787 * s) * r + (0.715 - 0.715 * s) * g + (0.072 - 0.072 * s) * b;
        let green = (0.213 - 0.213 * s) * r + (0.715 + 0.285 * s) * g + (0.072 - 0.072 * s) * b;
        let blue = (0.213 - 0.213 * s) * r + (0.715 - 0.715 * s) * g + (0.072 + 0.928 * s) * b;
        let alpha = color.alpha;
        ColorRecord { red: red as u8, green: green as u8, blue: blue as u8, alpha }
    }
}

struct HueRotate(f32);

impl Filter for HueRotate {
    fn filter(&self, color: ColorRecord) -> ColorRecord {
        let s = self.0;
        let angle = std::f32::consts::PI * s / 180.0;
        let sin = angle.sin();
        let cos = angle.cos();
        let (r, g, b) = (color.red as f32, color.green as f32, color.blue as f32);

        let red = (0.213 + cos * 0.787 - sin * 0.213) * r +
                  (0.715 - cos * 0.715 - sin * 0.715) * g +
                  (0.072 - cos * 0.072 + sin * 0.928) * b;  
        let green = (0.213 - cos * 0.213 + sin * 0.143) * r +
                    (0.715 + cos * 0.285 + sin * 0.140) * g +
                    (0.072 - cos * 0.072 - sin * 0.283) * b;
        let blue = (0.213 - cos * 0.213 - sin * 0.787) * r +
                   (0.715 - cos * 0.715 + sin * 0.715) * g +
                   (0.072 + cos * 0.928 + sin * 0.072) * b;
        let alpha = color.alpha;
        ColorRecord { red: red as u8, green: green as u8, blue: blue as u8, alpha }
    }
}

enum Operation {
    Invert(Invert),
    Contrast(Contrast),
    Saturate(Saturate),
    Brightness(Brightness),
    Opacity(Opacity),
    HueRotate(HueRotate),
}

impl Operation {
    fn filter(&self) -> &dyn Filter {
        match self {
            Self::Invert(i) => i,
            Self::Contrast(c) => c,
            Self::Brightness(b) => b,
            Self::Opacity(o) => o,
            Self::Saturate(s) => s,
            Self::HueRotate(h) => h,
        }
    }
}

fn parse_percent<'a>(i: &'a str) -> IResult<&'a str, f32, Error<&'a str>> {
    terminated(float, tag("%")).parse(i)
}

fn parse_deg<'a>(i: &'a str) -> IResult<&'a str, f32, Error<&'a str>> {
    terminated(float, tag("deg")).parse(i)
}

fn parse_value<'a>(i: &'a str) -> IResult<&'a str, f32, Error<&'a str>> {
    alt((
        parse_percent,
        parse_deg,
        float,
    )).parse(i)
}

fn parse_expr<'a>(t: &str, i: &'a str) -> IResult<&'a str, f32, Error<&'a str>> {
    preceded(tag(t), delimited(char('('), parse_value, char(')'))).parse(i)
}

fn parse_operation<'a>(i: &'a str) -> IResult<&'a str, Operation, Error<&'a str>> {
    alt((
      map(|i| parse_expr("invert", i), |i| Operation::Invert(Invert(i))),
      map(|i| parse_expr("contrast", i), |i| Operation::Contrast(Contrast(i))),
      map(|i| parse_expr("brightness", i), |i| Operation::Brightness(Brightness(i))),
      map(|i| parse_expr("opacity", i), |i| Operation::Opacity(Opacity(i))),
      map(|i| parse_expr("saturate", i), |i| Operation::Saturate(Saturate(i))),
      map(|i| parse_expr("hue-rotate", i), |i| Operation::HueRotate(HueRotate(i))),
    )).parse(i)
}

fn parse_operations<'a>(i: &'a str) -> IResult<&'a str, Vec<Operation>, Error<&'a str>> {
    separated_list1(multispace1, cut(parse_operation)).parse(i)
}

fn filter_first_palette(operations: &Vec<Operation>, buffer: &[u8]) -> String {
    let scope = ReadScope::new(&buffer);

    let font_data = FontData::read(&mut scope.ctxt()).unwrap();
    let index = 0;
    let provider = font_data.table_provider(index).unwrap();

    let cpal_data = provider.read_table_data(CPAL).unwrap();
    let cpal_scope = ReadScope::new(&cpal_data);
    let cpal = CpalTable::read(&mut cpal_scope.ctxt()).unwrap();

    let palette = cpal.palette(0).unwrap();

    let mut output = "override-colors:\n".to_string();

    let mut i = 0u16;
    while let Some(mut color) = palette.color(i) {
        for op in operations {
            color = op.filter().filter(color);

        }
        let ColorRecord { blue, green, red, alpha } = color;
        if i != 0 {
            output += ",\n";
        }
        output += &format!("    {i} #{red:02x}{green:02x}{blue:02x}{alpha:02x}");
        i += 1;
    }
    output += ";";
    output
}

pub fn parse_and_filter_first_palette(operations: &str, buffer: &[u8]) -> String {
    let operations = parse_operations(operations).unwrap().1;
    filter_first_palette(&operations, buffer)
}

#[wasm_bindgen]
pub struct FontFilter {
    filter: Option<Vec<Operation>>
}

#[wasm_bindgen]
impl FontFilter {
    #[wasm_bindgen(constructor)]
    pub fn new(filter: &str) -> Self {
        Self {
            filter: parse_operations(filter).ok().map(|e| e.1),
        }
    }

    pub fn parse_successful(&self) -> bool {
        self.filter.is_some()
    }

    pub fn generate_palette(&self, buffer: &[u8]) -> String {
        filter_first_palette(self.filter.as_ref().unwrap(), buffer)
    }
}

#[test]
fn invert() {
    let font = "./font.woff2";
    let buffer = std::fs::read(font).unwrap();
    let unfiltered = parse_and_filter_first_palette("opacity(1.0)", &buffer);
    let inverted = parse_and_filter_first_palette("invert(1.0)", &buffer);
    assert_ne!(unfiltered, inverted);

    let uninverted = parse_and_filter_first_palette("invert(0.0)", &buffer);
    assert_eq!(unfiltered, uninverted);

    let uninverted = parse_and_filter_first_palette("invert(1.0) invert(1.0)", &buffer);
    assert_eq!(unfiltered, uninverted);
}

#[test]
fn together() {
    let font = "./font.woff2";
    let buffer = std::fs::read(font).unwrap();
    let unfiltered = parse_and_filter_first_palette("opacity(1.0)", &buffer);
    let inverted = parse_and_filter_first_palette("brightness(0.5) saturate(2.0) invert(1.0)", &buffer);
    assert_ne!(unfiltered, inverted);
}
