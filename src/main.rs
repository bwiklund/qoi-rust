use std::{fs::File, io::Write};

use image::{DynamicImage, GenericImage, GenericImageView};

fn main() {
    println!("QOI");

    // run_on_image("tests/atlas.png");
    run_on_image("tests/selene_neutral_0.png");
    run_on_image("tests/pinch_atlas32.png");
    run_on_image("tests/spotify.png");
    run_on_image("tests/yammy.png");
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

    let qoi = encode(&img);
    println!(
        "{} as qoi -> {} bytes ({}%)",
        path,
        qoi.len(),
        100.0 * qoi.len() as f32 / raw_size_bytes as f32
    );
    let mut file = File::create(path.to_owned() + ".qoi").unwrap();
    file.write(&qoi).unwrap();

    let decoded = decode(&qoi);
    decoded.save(path.to_owned() + ".decoded.png").unwrap();

    check(&img, &decoded);
}

fn check(a: &DynamicImage, b: &DynamicImage) {
    // comparing as_bytes wasn't reliable, because of alpha channel missing on opaque images i think?
    for y in 0..a.height() {
        for x in 0..a.width() {
            let a_pixel = a.get_pixel(x, y);
            let b_pixel = b.get_pixel(x, y);

            if a_pixel != b_pixel {
                println!(">>> !!!! pixel mismatch at {}, {}", x, y);
                return;
            }
        }
    }
}

const TOKEN_RGB: u8 = 0b11111110;
const TOKEN_RGBA: u8 = 0b11111111;
const TOKEN_LOOKBACK: u8 = 0b00000000;
const TOKEN_DIFF_RGB: u8 = 0b01000000;
const TOKEN_DIFF_LUMA: u8 = 0b10000000;
const TOKEN_RUN: u8 = 0b11000000;
const MASK: u8 = 0b00111111;
const TOKEN_END: u32 = 0x00000001;

fn color_hash(r: u8, g: u8, b: u8, a: u8) -> usize {
    ((r as u32 * 3 + g as u32 * 5 + b as u32 * 7 + a as u32 * 11) % 64) as usize
}

fn encode(img: &DynamicImage) -> Vec<u8> {
    let mut buffer: Vec<u8> = Vec::new();

    buffer.extend_from_slice("qoif".as_bytes());

    // push a u16 to u8 buffer as two bytes
    buffer.extend_from_slice(&(img.width() as u32).to_be_bytes());
    buffer.extend_from_slice(&(img.height() as u32).to_be_bytes());

    buffer.push(4); // rgba channels
    buffer.push(0); // srgb space, default

    let mut last_r: u8 = 0;
    let mut last_g: u8 = 0;
    let mut last_b: u8 = 0;
    let mut last_a: u8 = 255;

    let mut run_length: u8 = 0;

    let mut lookback_arr = [0u32; 64];

    for y in 0..img.height() {
        for x in 0..img.width() {
            let pixel = img.get_pixel(x, y); // todo: might not be the fastest way to get at the bytes
            let r = pixel[0];
            let g = pixel[1];
            let b = pixel[2];
            let a = pixel[3];

            let diff_r = r as i16 - last_r as i16;
            let diff_g = g as i16 - last_g as i16;
            let diff_b = b as i16 - last_b as i16;
            // let diff_a = r as i16 - last_a as i16;

            let is_same = r == last_r && g == last_g && b == last_b && a == last_a;

            if is_same && run_length < 62 {
                // 64 - 2 = 62 is the max, otherwise would be ambiguous with TOKEN_RGB(A)
                run_length += 1;
                continue;

                // if we reach the last pixel on a run, the decoder will implicitly run to the end of the image
            }

            // write out the run length if we're on a run, then continue with the new pixel
            if run_length > 0 {
                buffer.push((run_length - 1) | TOKEN_RUN);
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
            } else if a != last_a {
                // if the alpha changed we send an entire rgba no matter what
                buffer.push(TOKEN_RGBA);
                buffer.push(r);
                buffer.push(g);
                buffer.push(b);
                buffer.push(a);
            } else if (diff_r != 0 || diff_g != 0 || diff_b != 0)
                && (diff_r >= -2
                    && diff_r < 2
                    && diff_g >= -2
                    && diff_g < 2
                    && diff_b >= -2
                    && diff_b < 2)
            {
                // if alpha is unchanged, and diff can fit in a mini diff token:
                let packed_diff =
                    ((diff_r + 2) as u8) << 4 | ((diff_g + 2) as u8) << 2 | ((diff_b + 2) as u8);
                buffer.push(TOKEN_DIFF_RGB | packed_diff);
            } else if (diff_r != 0 || diff_g != 0 || diff_b != 0)
                && (diff_g >= -32
                    && diff_g < 32
                    && (diff_r - diff_g) >= -8
                    && (diff_r - diff_g) < 8
                    && (diff_b - diff_g) >= -8
                    && (diff_b - diff_g) < 8)
            {
                buffer.push(TOKEN_DIFF_LUMA | (((diff_g + 32) & 0b00111111) as u8));
                buffer.push((((diff_r - diff_g) + 8) as u8) << 4 | (((diff_b - diff_g) + 8) as u8));
            } else {
                // else diff is too big for either diff chunk. send a full rgb with unchanged alpha
                buffer.push(TOKEN_RGB);
                buffer.push(r);
                buffer.push(g);
                buffer.push(b);
            }

            // and cache it in the lookback array
            lookback_arr[lookback_hash] = color_u32;

            last_r = r;
            last_g = g;
            last_b = b;
            last_a = a;
        }
    }

    if run_length > 0 {
        buffer.push((run_length - 1) | TOKEN_RUN);
    }

    buffer.write(&TOKEN_END.to_le_bytes()).unwrap();

    buffer
}

fn pack_rgba(r: u8, g: u8, b: u8, a: u8) -> u32 {
    (r as u32) << 24 | (g as u32) << 16 | (b as u32) << 8 | (a as u32)
}

pub fn decode(buf: &Vec<u8>) -> DynamicImage {
    let magic_number = &buf[0..4];
    if magic_number != "qoif".as_bytes() {
        panic!("invalid magic number");
    }

    let w = u32::from_be_bytes([buf[4], buf[5], buf[6], buf[7]]);
    let h = u32::from_be_bytes([buf[8], buf[9], buf[10], buf[11]]);
    let _channels = buf[12];
    let _color_space = buf[13];

    let mut img = DynamicImage::new_rgba8(w, h);

    let mut r = 0;
    let mut g = 0;
    let mut b = 0;
    let mut a = 255;

    let mut lookback_arr = [0u32; 64];

    // for each byte starting after header
    let mut pixel_idx = 0;
    let mut buff_idx = 14;

    // helper fn to draw if in bounds
    let mut draw = |pix: u32, r: u8, g: u8, b: u8, a: u8| {
        let x = pix % w;
        let y = pix / w;
        if img.in_bounds(x, y) {
            img.put_pixel(x, y, image::Rgba([r, g, b, a]));
        }
    };

    while buff_idx < buf.len() {
        let token = buf[buff_idx];
        buff_idx += 1;

        if token == TOKEN_RGBA {
            r = buf[buff_idx + 0];
            g = buf[buff_idx + 1];
            b = buf[buff_idx + 2];
            a = buf[buff_idx + 3];
            buff_idx += 4;

            draw(pixel_idx, r, g, b, a);
            pixel_idx += 1;
        } else if token == TOKEN_RGB {
            r = buf[buff_idx + 0];
            g = buf[buff_idx + 1];
            b = buf[buff_idx + 2];
            // a is unchanged
            buff_idx += 3;

            draw(pixel_idx, r, g, b, a);
            pixel_idx += 1;
        } else if token & !MASK == TOKEN_RUN {
            let run_length = (token & MASK) + 1;
            for _ in 0..run_length {
                draw(pixel_idx, r, g, b, a);
                pixel_idx += 1;
            }
        } else if token & !MASK == TOKEN_LOOKBACK {
            let lookback_hash = token & MASK;
            let color_u32 = lookback_arr[lookback_hash as usize];
            a = (color_u32 & 0xFF) as u8;
            b = ((color_u32 >> 8) & 0xFF) as u8;
            g = ((color_u32 >> 16) & 0xFF) as u8;
            r = ((color_u32 >> 24) & 0xFF) as u8;

            draw(pixel_idx, r, g, b, a);
            pixel_idx += 1;
        } else if token & !MASK == TOKEN_DIFF_RGB {
            let diff = token & MASK;
            let diff_r = ((diff >> 4) & 0b11) as i16 - 2;
            let diff_g = ((diff >> 2) & 0b11) as i16 - 2;
            let diff_b = (diff & 0b11) as i16 - 2;

            r = (r as i16 + diff_r as i16) as u8;
            g = (g as i16 + diff_g as i16) as u8;
            b = (b as i16 + diff_b as i16) as u8;
            // a is unchanged

            draw(pixel_idx, r, g, b, a);
            pixel_idx += 1;
        } else if token & !MASK == TOKEN_DIFF_LUMA {
            let diff_p1 = token & MASK;
            let diff_g = (diff_p1 & 0b00111111) as i16 - 32;

            let diff_p2 = buf[buff_idx];
            buff_idx += 1;

            let diff_r = ((diff_p2 >> 4) & 0b00001111) as i16 - 8;
            let diff_b = (diff_p2 & 0b00001111) as i16 - 8;

            r = (r as i16 + diff_r + diff_g) as u8;
            g = (g as i16 + diff_g) as u8;
            b = (b as i16 + diff_b + diff_g) as u8;
            // a is unchanged

            draw(pixel_idx, r, g, b, a);
            pixel_idx += 1;
        } else {
            panic!("invalid token");
        }

        let lookback_hash = color_hash(r, g, b, a);
        lookback_arr[lookback_hash] = pack_rgba(r, g, b, a);
    }

    // if we didn't get to the last pixel, fill the rest with the last color
    while pixel_idx < w * h {
        draw(pixel_idx, r, g, b, a);
        pixel_idx += 1;
    }

    img
}
