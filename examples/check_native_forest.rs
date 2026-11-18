//! Displayed demo regression: a forest drag must replace sea in its empty corner.
//! Input is a CUA screenshot at the fixed 1440x972 native test window size.
fn main() -> Result<(), String> {
    let path = std::env::args()
        .nth(1)
        .ok_or("Pass the native screenshot PNG")?;
    let image = image::ImageReader::open(path)
        .map_err(|e| e.to_string())?
        .with_guessed_format()
        .map_err(|e| e.to_string())?
        .decode()
        .map_err(|e| e.to_string())?
        .to_rgb8();
    if image.dimensions() != (1440, 972) {
        return Err("This fixed-camera visual fixture requires a 1440x972 screenshot".into());
    }
    // The supplied native forest replay paints (250,720) → (620,790).
    // This part of the demo is sea before the stroke, clear of chrome/old art.
    let ink = (740..815)
        .flat_map(|y| (300..570).map(move |x| (x, y)))
        .filter(|&(x, y)| {
            let p = image.get_pixel(x, y);
            p[1] < 218 && p[2] < 200
        })
        .count();
    println!("Painted forest pixels in the native fixture region: {ink}");
    if ink < 3000 {
        return Err("committed forest is absent from the displayed native canvas".into());
    }
    Ok(())
}
