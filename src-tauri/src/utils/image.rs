// 判断是否是图片文件
pub fn is_image_file(path: &str) -> bool {
    let path_lower = path.to_lowercase();
    path_lower.ends_with(".jpg")
        || path_lower.ends_with(".jpeg")
        || path_lower.ends_with(".png")
        || path_lower.ends_with(".gif")
        || path_lower.ends_with(".bmp")
        || path_lower.ends_with(".webp")
}

// 读取图片尺寸
pub fn get_image_dimensions(path: &str) -> Option<(u32, u32)> {
    use image::ImageReader;
    use std::fs::File;
    use std::io::BufReader;

    let file = File::open(path).ok()?;
    let reader = BufReader::new(file);
    let img_reader = ImageReader::new(reader).with_guessed_format().ok()?;
    img_reader.into_dimensions().ok()
}
