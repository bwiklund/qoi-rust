use std::{fs::File, io::Write};

use image::{DynamicImage, GenericImage, GenericImageView};

fn main() {
    println!("BIP");

    run_on_image("tests/atlas.png");
    run_on_image("tests/selene_neutral_0.png");
    run_on_image("tests/pinch_atlas32.png");
}

fn run_on_image(path: &str) {
    let img = image::open(path).unwrap();

    // print disk size of the path
    let raw_size_bytes = img.width() * img.height() * 4;
    println!("{} as uncompressed -> {} bytes", path, raw_size_bytes);

    let metadata = std::fs::metadata(path).unwrap();
    let size = metadata.len();
    println!(
        "{} as png -> {} bytes ({}%)",
        path,
        size,
        100.0 * size as f32 / raw_size_bytes as f32
    );

    let bip = encode(img);
    println!(
        "{} as bip -> {} bytes ({}%)",
        path,
        bip.len(),
        100.0 * bip.len() as f32 / raw_size_bytes as f32
    );
    let mut file = File::create(path.to_owned() + ".bip").unwrap();
    file.write(&bip).unwrap();

    let decoded = decode(bip);
    decoded.save(path.to_owned() + ".decoded.png").unwrap();
}

const TOKEN_RGBA: u8 = 0b00000000;
const TOKEN_RUN: u8 = 0b01000000;
const TOKEN_LOOKBACK: u8 = 0b10000000;
const MASK: u8 = 0b00111111;

fn color_hash(r: u8, g: u8, b: u8, a: u8) -> usize {
    ((r as u32 * 3 + g as u32 * 5 + b as u32 * 7 + a as u32 * 11) % 64) as usize
}

fn encode(img: DynamicImage) -> Vec<u8> {
    let mut buffer: Vec<u8> = Vec::new();

    // push a u16 to u8 buffer as two bytes
    buffer.extend_from_slice(&(img.width() as u16).to_le_bytes());
    buffer.extend_from_slice(&(img.height() as u16).to_le_bytes());

    let mut last_r: u8 = 0;
    let mut last_g: u8 = 0;
    let mut last_b: u8 = 0;
    let mut last_a: u8 = 0;

    let mut run_length: u8 = 0;

    let mut lookback_arr = [0u32; 64];

    for y in 0..img.height() {
        for x in 0..img.width() {
            let pixel = img.get_pixel(x, y); // todo: might not be the fastest way to get at the bytes
            let r = pixel[0];
            let g = pixel[1];
            let b = pixel[2];
            let a = pixel[3];

            let is_same = r == last_r && g == last_g && b == last_b && a == last_a;
            last_r = r;
            last_g = g;
            last_b = b;
            last_a = a;

            if is_same && run_length < 63 {
                run_length += 1;
                continue;

                // if we reach the last pixel on a run, the decoder will implicitly run to the end of the image
            }

            // write out the run length if we're on a run, then continue with the new pixel
            if run_length > 0 {
                buffer.push(run_length | TOKEN_RUN);
                run_length = 0;

                if is_same {
                    // immediatelly start the next run here if it's continuing
                    run_length += 1;
                    continue;
                }
            }

            // else, we have a new pixel

            // maybe it's in the lookback array
            let lookback_hash = color_hash(r, g, b, a);
            let color_u32 = pack_rgba(r, g, b, a);
            if lookback_arr[lookback_hash] == color_u32 {
                buffer.push(lookback_hash as u8 | TOKEN_LOOKBACK);
                continue;
            }

            // else, just write the pixel out
            buffer.push(TOKEN_RGBA);
            buffer.push(r);
            buffer.push(g);
            buffer.push(b);
            buffer.push(a);

            // and cache it in the lookback array
            lookback_arr[lookback_hash] = color_u32;
        }
    }

    buffer
}

fn pack_rgba(r: u8, g: u8, b: u8, a: u8) -> u32 {
    (r as u32) << 24 | (g as u32) << 16 | (b as u32) << 8 | (a as u32)
}

pub fn decode(buf: Vec<u8>) -> DynamicImage {
    // for each byte,
    let w = u16::from_le_bytes([buf[0], buf[1]]) as u32;
    let h = u16::from_le_bytes([buf[2], buf[3]]) as u32;
    let mut img = DynamicImage::new_rgba8(w, h);

    let mut r = 0;
    let mut g = 0;
    let mut b = 0;
    let mut a = 0;

    let mut lookback_arr = [0u32; 64];

    // for each byte starting at 2
    let mut pixel_idx = 0;
    let mut buff_idx = 4;

    // helper fn to draw if in bounds
    let mut draw = |r: u8, g: u8, b: u8, a: u8| {
        let x = pixel_idx % w;
        let y = pixel_idx / w;
        pixel_idx += 1;
        if img.in_bounds(x, y) {
            img.put_pixel(x, y, image::Rgba([r, g, b, a]));
        }
    };

    while buff_idx < buf.len() {
        let token = buf[buff_idx];
        buff_idx += 1;

        if token & !MASK == TOKEN_RUN {
            let run_length = token & MASK;
            for _ in 0..run_length {
                draw(r, g, b, a);
            }
        } else if token & !MASK == TOKEN_LOOKBACK {
            let lookback_hash = token & MASK;
            let color_u32 = lookback_arr[lookback_hash as usize];
            a = (color_u32 & 0xFF) as u8;
            b = ((color_u32 >> 8) & 0xFF) as u8;
            g = ((color_u32 >> 16) & 0xFF) as u8;
            r = ((color_u32 >> 24) & 0xFF) as u8;

            draw(r, g, b, a);
        } else if token == TOKEN_RGBA {
            r = buf[buff_idx + 0];
            g = buf[buff_idx + 1];
            b = buf[buff_idx + 2];
            a = buf[buff_idx + 3];
            buff_idx += 4;

            draw(r, g, b, a);
        } else {
            panic!("invalid token");
        }

        let lookback_hash = color_hash(r, g, b, a);
        lookback_arr[lookback_hash] = pack_rgba(r, g, b, a);
    }

    img
}
