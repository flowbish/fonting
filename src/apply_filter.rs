use std::env;

use fonting::parse_and_filter_first_palette;

fn main() -> Result<(), u8> {
    let args: Vec<String> = env::args().collect();
    if args.len() < 3 {
        eprintln!("Usage: {} <font>.woff2 <filter>", &args[0]);
        return Err(1); 
    }
    let font = &args[1];
    let filter = &args[2];
    let buffer = std::fs::read(font).unwrap();
    let css = parse_and_filter_first_palette(filter, &buffer);
    println!("{css}"); 

    Ok(())
}