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

fn synthetic_bmp(w: usize, h: usize) -> Vec<u8> {
    let bpp = 3;
    let stride = ((w * bpp + 3) / 4) * 4;
    let pixels = stride * h;
    let offset = 54usize;
    let filesize = offset + pixels;

    let mut d = Vec::with_capacity(filesize);
    d.extend_from_slice(b"BM");
    d.extend_from_slice(&(filesize as u32).to_le_bytes());
    d.extend_from_slice(&0u32.to_le_bytes());
    d.extend_from_slice(&(offset as u32).to_le_bytes());
    d.extend_from_slice(&40u32.to_le_bytes());
    d.extend_from_slice(&(w as u32).to_le_bytes());
    d.extend_from_slice(&(h as u32).to_le_bytes());
    d.extend_from_slice(&1u16.to_le_bytes());
    d.extend_from_slice(&24u16.to_le_bytes());
    d.extend_from_slice(&0u32.to_le_bytes());
    d.extend_from_slice(&(pixels as u32).to_le_bytes());
    d.extend_from_slice(&0i32.to_le_bytes());
    d.extend_from_slice(&0i32.to_le_bytes());
    d.extend_from_slice(&0u32.to_le_bytes());
    d.extend_from_slice(&0u32.to_le_bytes());

    for y in 0..h {
        for x in 0..w {
            d.push((x % 256) as u8);
            d.push((y % 256) as u8);
            d.push(((x + y) % 256) as u8);
        }
        for _ in 0..(stride - w * bpp) {
            d.push(0);
        }
    }
    d
}

#[test]
fn detects_ppm_geometry() {
    let img = synthetic_ppm(64, 48);
    let info = image::detect(&img).expect("should detect a P6 ppm");
    assert_eq!(info.bpp, 3);
    assert_eq!(info.stride, 64 * 3);
    assert_eq!(info.rows, 48);
}

#[test]
fn detects_bmp_geometry() {
    let img = synthetic_bmp(100, 60);
    let info = image::detect(&img).expect("should detect a 24-bit BMP");
    assert_eq!(info.bpp, 3);
    assert_eq!(info.stride, 300);
    assert_eq!(info.rows, 60);
}

#[test]
fn filter_unfilter_round_trips() {
    let img = synthetic_ppm(100, 70);
    let info = image::detect(&img).unwrap();
    let filtered = image::filter(&img, &info);
    assert!(filtered.len() > img.len());
    assert_eq!(image::unfilter(&filtered), img);
}

#[test]
fn container_round_trips_a_filtered_ppm() {
    let img = synthetic_ppm(200, 150);
    let packed = container::write(&img, "ppm");
    let c = container::read(&packed).unwrap();
    assert_eq!(c.filter, 1);
    let out = c.decode();
    assert_eq!(out, img);
    assert_eq!(crc::crc32(&out), c.crc);
}

#[test]
fn container_round_trips_a_bmp() {
    let img = synthetic_bmp(120, 90);
    let packed = container::write(&img, "bmp");
    let c = container::read(&packed).unwrap();
    assert_eq!(c.filter, 1);
    assert_eq!(c.decode(), img);
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
