use image::{DynamicImage, EncodableLayout, RgbImage};
use tokio::macros::support::thread_rng_n;
use rand::thread_rng;
use rand::seq::SliceRandom;

pub fn create_brightness_counts() -> Vec<u32> {
	vec![0_u32; 256]
}

fn brightness_at(
	bytes: &[u8],
	i: usize,
) -> f32 {
	let red = bytes[i * 3    ];
	let gre = bytes[i * 3 + 1];
	let blu = bytes[i * 3 + 2];

	(red as f32 + gre as f32 + blu as f32) / 3_f32
}

fn pixel_at(
	bytes: &[u8],
	i: usize,
) -> (u8, u8, u8) {
	(
		bytes[i * 3    ],
		bytes[i * 3 + 1],
		bytes[i * 3 + 2],
	)
}

pub fn count_brightness(
	image: &RgbImage,
	counts: &mut Vec<u32>,
) {
	let image_bytes = image.as_bytes();

	for i in 0..image.width() * image.height() {
		let brightness = brightness_at(image_bytes, i as usize);
		counts[brightness.round() as usize] += 1;
	}
}

pub fn create_brightness_map(
	from_counts: &Vec<u32>,
	to_counts: &Vec<u32>,
) -> Vec<u32> {
	let mut map = Vec::with_capacity(256);
	let mut cumulative_start = 0_u32;

	for i in 0..256 {
		let end = cumulative_start + from_counts[i];
		let center = (end + cumulative_start) / 2;

		/* find which color center selects in to_color */
		let mut to_start = 0_u32;
		let mut to_index= 255;
		for j in 0..256 {
			if center < to_start + to_counts[j] {
				to_index = j;
				break;
			} else {
				to_start += to_counts[j];
			}
		}

		map.push(to_index as u32);

		cumulative_start += from_counts[i];
	}

	map
}

pub fn match_brightness(
	image: &RgbImage,
	brightness_map: &Vec<u32>,
) -> RgbImage {
	let old_bytes = image.as_bytes();
	let mut new_bytes = Vec::with_capacity(old_bytes.len());

	for i in 0..image.width() * image.height() {
		let (red , gre, blu) = pixel_at(old_bytes, i as usize);
		let base_brightness = (red as f32 + gre as f32 + blu as f32) / 3_f32;
		let new_brightness = brightness_map[base_brightness.round() as usize] as f32;

		let red = (red as f32 * (new_brightness / base_brightness)).min(255.0_f32).round();
		let gre = (gre as f32 * (new_brightness / base_brightness)).min(255.0_f32).round();
		let blu = (blu as f32 * (new_brightness / base_brightness)).min(255.0_f32).round();

		new_bytes.push(red as u8);
		new_bytes.push(gre as u8);
		new_bytes.push(blu as u8);
	}

	RgbImage::from_raw(image.width(), image.height(), new_bytes).unwrap()
}

pub fn add_duplicates(
	card_images: &mut Vec<DynamicImage>,
	num_duplicates: u32,
) {
	card_images.shuffle(&mut thread_rng());

	for i in 0..num_duplicates {
		let original_image = &card_images[i as usize];

		card_images.push(original_image.clone());
	}
}
