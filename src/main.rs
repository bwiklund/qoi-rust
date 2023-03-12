use image::ImageBuffer;

fn main() {
    println!("Let's make a png atlas!");

    encode("tests/selene_neutral_0.png");
}

fn encode(path: &str) {
    let img = image::open(path).unwrap();

    // print disk size of the path
    let raw_size_bytes = img.width() * img.height() * 4;
    println!(
        "{} uncompress -> {} bytes",
        path,
        raw_size_bytes
    );

    let metadata = std::fs::metadata(path).unwrap();
    let size = metadata.len();
    println!(
        "{} as png -> {} bytes ({}%)",
        path,
        size,
        100.0 * (img.width() * img.height()) as f32 / raw_size_bytes as f32
    );
}
