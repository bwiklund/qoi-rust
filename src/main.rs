use image::ImageBuffer;

fn main() {
    println!("Let's make a png atlas!");

    encode("tests/selene_neutral_0.png");
}

fn encode(path: &str) {
    let img = image::open(path).unwrap();

    // print disk size of the path
    let metadata = std::fs::metadata(path).unwrap();
    let size = metadata.len();
    println!("{} is {} bytes", path, size);
}
