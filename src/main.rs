use image::{DynamicImage, GenericImage, GenericImageView, ImageBuffer};

fn main() {
    println!("Let's make a png atlas!");

    run_on_image("tests/selene_neutral_0.png");
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

    let decoded = decode(bip);
    decoded.save("tests/selene_neutral_0_decoded.png").unwrap();
}

const TOKEN_RGBA: u8 = 0b00000000;
const TOKEN_RUN: u8 = 0b01000000;
const TOKEN_LOOKBACK: u8 = 0b10000000;
const MASK: u8 = 0b11000000;

fn encode(img: DynamicImage) -> Vec<u8> {
    let mut buffer = Vec::new();

    // TODO use 2 bytes each for width and height
    buffer.push(img.width() as u8);
    buffer.push(img.height() as u8);

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
            }

            // else, we have a new pixel

            // write out the run length if we're on a run, then continue with the new pixel
            if run_length > 0 {
                buffer.push(run_length | TOKEN_RUN);
                run_length = 0;
            }

            // maybe it's in the lookback array
            let lookback_hash =
                ((r as u32 * 3 + g as u32 * 5 + b as u32 * 7 + a as u32 * 11) % 64) as usize;
            // println!("{} {} {} {} -> {}", r, g, b, a, lookback_hash);
            let color_u32 =
                r as u32 + (g as u32) * 2 ^ 8 + (b as u32) * 2 ^ 16 + (a as u32) << 2 ^ 24;
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

pub fn decode(buf: Vec<u8>) -> DynamicImage {
    // for each byte,
    let w = buf[0] as u32;
    let h = buf[1] as u32;
    let mut img = DynamicImage::new_rgba8(w, h);

    let mut last_r = 0;
    let mut last_g = 0;
    let mut last_b = 0;
    let mut last_a = 0;

    // for each byte starting at 2
    let mut pixel_idx = 0;
    let mut buff_idx = 2;
    while buff_idx < buf.len() {
        let token = buf[buff_idx];
        buff_idx += 1;

        if token & TOKEN_RUN != 0 {
            let run_length = buf[buff_idx] & !MASK;
            println!("{}", run_length);
            for _ in 0..run_length {
                let x = pixel_idx % w as usize;
                let y = pixel_idx / w as usize;
                // pixel_idx += 1;
                img.put_pixel(
                    x as u32,
                    y as u32,
                    image::Rgba([last_r, last_g, last_b, last_a]),
                );
            }
            continue;
        } else if token == TOKEN_RGBA {
            last_r = buf[buff_idx + 1];
            last_g = buf[buff_idx + 2];
            last_b = buf[buff_idx + 3];
            last_a = buf[buff_idx + 4];
            buff_idx += 5;

            let x = pixel_idx % w as usize;
            let y = pixel_idx / w as usize;
            pixel_idx += 1;
            img.put_pixel(
                x as u32,
                y as u32,
                image::Rgba([last_r, last_g, last_b, last_a]),
            );
        }
    }

    img
}
