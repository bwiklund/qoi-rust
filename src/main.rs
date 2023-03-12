use image::{DynamicImage, GenericImageView, ImageBuffer};

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
}

fn encode(img: DynamicImage) -> Vec<u8> {
    let mut buffer = Vec::new();

    let mut last_r: u8 = 0xff;
    let mut last_g: u8 = 0xff;
    let mut last_b: u8 = 0xff;
    let mut last_a: u8 = 0xff;

    let mut run_length: u8 = 0;

    let token_rgba = 0b00000000;
    let token_run = 0b11000000;

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

            if is_same && run_length < 64 {
                run_length += 1;
                continue;
            }

            // else, we have a new pixel

            // write out the run length if we're on a run
            if run_length > 0 {
                buffer.push(run_length as u8 & token_run);
                run_length = 0;
                continue;
            }

            // else, just write the pixel out

            buffer.push(token_rgba);
            buffer.push(r);
            buffer.push(g);
            buffer.push(b);
            buffer.push(a);
        }
    }

    buffer
}
