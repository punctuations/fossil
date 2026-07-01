use fossil::core::{container, crc, image};

fn synthetic_ppm(w: usize, h: usize) -> Vec<u8> {
    let mut data = format!("P6\n{} {}\n255\n", w, h).into_bytes();
    for y in 0..h {
        for x in 0..w {
            data.push(((x * 3) % 256) as u8);
            data.push(((y * 5) % 256) as u8);
            data.push((((x + y) * 7) % 256) as u8);
        }
    }
    data
}

#[test]
fn detect_reads_geometry() {
    let img = synthetic_ppm(64, 48);
    let info = image::detect(&img).expect("should detect a P6 ppm");
    assert_eq!(info.bpp, 3);
    assert_eq!(info.stride, 64 * 3);
}

#[test]
fn filter_unfilter_round_trips() {
    let img = synthetic_ppm(100, 70);
    let info = image::detect(&img).unwrap();
    let filtered = image::filter(&img, &info);
    assert_eq!(filtered.len(), img.len());
    assert_eq!(image::unfilter(&filtered), img);
}

#[test]
fn container_round_trips_a_filtered_image() {
    let img = synthetic_ppm(200, 150);
    let packed = container::write(&img, "ppm");
    let c = container::read(&packed).unwrap();
    assert_eq!(c.filter, 1);
    let out = c.decode();
    assert_eq!(out, img);
    assert_eq!(crc::crc32(&out), c.crc);
}

#[test]
fn non_image_is_not_filtered() {
    let data = b"P6 this is not really an image header at all".to_vec();
    assert!(image::detect(&data).is_none());
    let packed = container::write(&data, "bin");
    let c = container::read(&packed).unwrap();
    assert_eq!(c.filter, 0);
    assert_eq!(c.decode(), data);
}
