mod new_sample;

use reqwest::{Client};
use serde::Deserialize;
use image::{DynamicImage, GenericImageView, ImageFormat, EncodableLayout, RgbImage};
use image::imageops::{FilterType};
use uuid::Uuid;
use std::fs::create_dir;
use std::path::Path;
use std::convert::From;
use std::{fs, env};
use std::io::ErrorKind;
use std::ptr::null;
use crate::new_sample::{populate_grid_new, rank_all_cards};

const ASPECT: f32 = 16.0_f32 / 9.0_f32;
const IMAGE_DIR: &str = "./ankiImages/";
const BASE_IMAGE_DIR: &str = "./test/nancy-crop.jpg";
const CARDS_WIDE: u32 = 80;
const IMAGE_WIDTH: u32 = 2000;
const SAMPLE_SIZE: u32 = 9;
const NORMAL_LAYOUT: &str = "normal";

#[tokio::main]
async fn main() {
	/* see what we're gonna do for this run */
	let args: Vec<String> = env::args().collect();

	if args.len() == 1 {
		println!("No arguments provided!");

	} else if args[1] == "pull" {
		let mut card_images: Vec<DynamicImage> = Vec::new();
		save_num_cards(IMAGE_DIR, &mut card_images, ASPECT, 100).await.unwrap();

	} else if args[1] == "resample" {
		setup_dir(IMAGE_DIR).unwrap();

		let mut card_images: Vec<DynamicImage> = Vec::with_capacity(32);

		load_existing_images(IMAGE_DIR, &mut card_images);

		let mut used_cards = vec![false; card_images.len()];

		println!("Loaded {} images!", card_images.len());

		let base_image = image::load_from_memory(fs::read(BASE_IMAGE_DIR).unwrap().as_slice()).unwrap();

		println!("Loaded base image!");

		let mut card_grid = create_grid(CARDS_WIDE, ASPECT, base_image.width(), base_image.height());

		populate_grid(&base_image, &card_images, &mut used_cards, &mut card_grid, SAMPLE_SIZE);

		println!("Found cards to sample!");

		let (card_draw_images, card_draw_indices) = create_draw_cards(&card_images, &used_cards, CARDS_WIDE, IMAGE_WIDTH, ASPECT);

		let output_image = draw_cards(&card_grid, card_draw_images, card_draw_indices, ASPECT, IMAGE_WIDTH);

		println!("Drew sampled image!");

		output_image.save_with_format("./test/sampled.png", ImageFormat::Png).unwrap();

	} else if args[1] == "new" {
		setup_dir(IMAGE_DIR).unwrap();
		let mut card_images: Vec<DynamicImage> = Vec::with_capacity(32);

		load_existing_images(IMAGE_DIR, &mut card_images);
		println!("Loaded {} images!", card_images.len());

		let base_image = image::load_from_memory(fs::read(BASE_IMAGE_DIR).unwrap().as_slice()).unwrap();
		println!("Loaded base image!");

		let mut card_grid = create_grid_fitting(card_images.len() as u32, ASPECT, base_image.width(), base_image.height());
		populate_grid_new(&base_image, &card_images, &mut card_grid, SAMPLE_SIZE);
		println!("Found cards to sample!");

		let all_used_cards = card_images.iter().map(|_| true).collect::<Vec<bool>>();
		let (card_draw_images, card_draw_indices) = create_draw_cards(&card_images, &all_used_cards, card_grid.cards_wide, IMAGE_WIDTH, ASPECT);
		let output_image = draw_cards(&card_grid, card_draw_images, card_draw_indices, ASPECT, IMAGE_WIDTH);
		println!("Drew sampled image!");

		output_image.save_with_format("./test/sampled.png", ImageFormat::Png).unwrap();
		println!("Saved sampled image!");

	} else {
		println!("Invalid argument");
	}
}

#[derive(Deserialize)]
struct ImageUris {
	art_crop: String
}

#[derive(Deserialize)]
struct CardInfo {
	id: String,
	layout: String,
	image_uris: ImageUris
}

fn setup_dir(dir_path: &str) -> std::io::Result<()> {
	if !Path::new(dir_path).exists() {
		create_dir(dir_path)?;
	}

	Ok(())
}

fn load_existing_images(dir_path: &str, card_images: &mut Vec<DynamicImage>) {
	let paths = fs::read_dir(dir_path).unwrap();

	for path in paths {
		let path = path.unwrap().path();

		card_images.push(image::load_from_memory(fs::read(&path).unwrap().as_slice()).unwrap());
	}
}

async fn save_num_cards(dir_path: &str, card_images: &mut Vec<DynamicImage>, card_aspect: f32, num_cards: u32) -> Result<(), Box<dyn std::error::Error>> {
	let client = reqwest::Client::new();
	let mut count = 0u32;

	while count < num_cards {
		match get_card(&client).await {
			Ok((card_image, card_uuid)) => {
				save_card(card_image, card_uuid, card_aspect, dir_path, card_images);

				println!("Got card {} out of {}!", count + 1, num_cards);

				count += 1;
			},
			Err(_err) => {
				println!("Failed to get card!, retrying");
			}
		}
	}

	Ok(())
}

async fn get_card(client: &Client) -> Result<(DynamicImage, Uuid), Box<dyn std::error::Error>> {
	let response = client.get("https://api.scryfall.com/cards/random").send().await?;

	let card_info = response.json::<CardInfo>().await?;

	/* prevent tokens, double faced cards, other things that interfere with art */
	if card_info.layout != NORMAL_LAYOUT { return Err(Box::new(std::io::Error::new(ErrorKind::Other, "Bad layout!"))) };

	let response = client.get(&card_info.image_uris.art_crop).send().await?;

	let image_bytes = response.bytes().await?;

	let image = image::load_from_memory(&image_bytes).unwrap();

	let uuid = Uuid::parse_str(card_info.id.as_str())?;

	Ok((image, uuid))
}

fn save_card(card_image: DynamicImage, card_uuid: Uuid, card_aspect: f32, dir_path: &str, card_images: &mut Vec<DynamicImage>) {
	let cropped_image = crop_card(card_image, card_aspect);

	/* build path to save card image to disk */
	let mut save_path = String::from(dir_path);
	save_path.push_str(card_uuid.to_string().as_str());
	save_path.push_str(".png");

	/* save card image to disk */
	cropped_image.save_with_format(save_path, ImageFormat::Png).unwrap();

	/* add card image to card images list */
	card_images.push(cropped_image);
}

fn crop_card(card_image: DynamicImage, desired_aspect: f32) -> DynamicImage {
	let width = card_image.width();
	let height = card_image.height();

	let current_aspect = width as f32 / height as f32;

	let new_width;
	let new_height;

	if desired_aspect > current_aspect {
		new_width = width;
		new_height = ((1.0f32 / desired_aspect) * width as f32).round() as u32;

	} else {
		new_width = (desired_aspect * height as f32).round() as u32;
		new_height = height;
	}

	card_image.resize_to_fill(new_width, new_height, FilterType::Triangle)
}

pub struct CardGrid {
	grid: Vec<u32>,
	cards_wide: u32,
	cards_tall: u32
}

fn populate_grid(base_image: &DynamicImage, card_images: &Vec<DynamicImage>, used_cards: &mut Vec<bool>, card_grid: &mut CardGrid, sample_size: u32) {
	let card_samples = create_card_samples(card_images, sample_size);
	let sample_image = create_sample_image(base_image, sample_size, card_grid.cards_wide, card_grid.cards_tall);

	for x in 0..card_grid.cards_wide {
		for y in 0..card_grid.cards_tall {
			let selected_card = select_best_card(&sample_image, &card_samples, sample_size, x, y);
			card_grid.grid[(y * card_grid.cards_wide + x) as usize] = selected_card;
			used_cards[selected_card as usize] = true;
		}
	}
}

fn create_card_samples(card_images: &Vec<DynamicImage>, sample_size: u32) -> Vec<RgbImage> {
	card_images
		.iter()
		.map(|full_image| full_image.resize_exact(sample_size, sample_size, FilterType::CatmullRom).to_rgb8())
		.collect::<Vec<RgbImage>>()
}

/**
 * creates a list of card images to draw
 * will be approximately 2 times the size that they will be drawn at
 */
fn create_draw_cards(card_images: &Vec<DynamicImage>, used_cards: &Vec<bool>, cards_wide: u32, image_width: u32, card_aspect: f32) -> (Vec<RgbImage>, Vec<usize>) {
	let card_width = (image_width as f32 / cards_wide as f32) * 2f32;

	let card_height = (card_width * (1f32 / card_aspect)).round() as u32;
	let card_width = card_width.round() as u32;

	let mut resized_card_indices = vec![0usize; card_images.len()];
	let mut resized_index = 0usize;

	let resized_cards = card_images
		.iter().enumerate()
		.filter_map(|(full_index, full_image)| if used_cards[full_index] {
			let resized_image = full_image.resize_exact(card_width, card_height, FilterType::Triangle).to_rgb8();

			resized_card_indices[full_index] = resized_index;
			resized_index += 1;

			Some(resized_image)
		} else {
			None
		})
		.collect::<Vec<RgbImage>>();

	(resized_cards, resized_card_indices)
}

fn cards_tall(cards_wide: u32, card_aspect: f32, image_width: u32, image_height: u32) -> u32 {
	let card_width = image_width as f32 / cards_wide as f32;
	let card_height = card_width * (1_f32 / card_aspect);

	/* cards tall = */
	(image_height as f32 / card_height).round() as u32
}

fn cards_wide(cards_tall: u32, card_aspect: f32, image_width: u32, image_height: u32) -> u32 {
	let card_height = image_height as f32 / cards_tall as f32;
	let card_width = card_height * card_aspect;

	/* cards tall = */
	(image_width as f32 / card_width).round() as u32
}

fn create_grid(cards_wide: u32, card_aspect: f32, image_width: u32, image_height: u32) -> CardGrid {
	let cards_tall = cards_tall(cards_wide, card_aspect, image_width, image_height);

	let grid = vec![0u32; (cards_wide * cards_tall) as usize];
	CardGrid { grid, cards_wide, cards_tall }
}

fn create_grid_fitting(n: u32, card_aspect: f32, image_width: u32, image_height: u32) -> CardGrid {
	let mut increment = 1;

	let mut result_wide;
	let mut result_tall;

	struct Result {
		width: u32,
		height: u32,
	}
	impl Result {
		fn size(&self) -> u32 {
			self.width * self.height
		}
	}

	loop {
		let results = [
			Result { width: cards_wide(increment, card_aspect, image_width, image_height), height: increment },
			Result { width: increment, height: cards_tall(increment, card_aspect, image_width, image_height) },
		];

		let best_result = results.iter()
			.filter(|result| result.size() >= n)
			.min_by(|result0, result1| result0.size().partial_cmp(&result1.size()).unwrap());

		if best_result.is_some() {
			let best = best_result.unwrap();
			result_wide = best.width;
			result_tall = best.height;
			break;
		}

		increment += 1;
	}

	CardGrid {
		grid: vec![0u32; (result_wide * result_tall) as usize],
		cards_wide: result_wide,
		cards_tall: result_tall
	}
}

fn create_sample_image(base_image: &DynamicImage, sample_size: u32, cards_wide: u32, cards_tall: u32) -> RgbImage {
	base_image.resize_exact(cards_wide * sample_size, cards_tall * sample_size, FilterType::Triangle).to_rgb8()
}

fn select_best_card(sample_image: &RgbImage, card_images: &Vec<RgbImage>, sample_size: u32, grid_x: u32, grid_y: u32) -> u32 {
	fn pixel_at(bytes: &[u8], width: u32, x: u32, y: u32) -> [u8; 3] {
		[bytes[((width * y + x) * 3u32) as usize], bytes[((width * y + x) * 3u32 + 1) as usize], bytes[((width * y + x) * 3u32 + 2) as usize]]
	}

	fn pixel_difference(pixel0: [u8; 3], pixel1: [u8; 3]) -> u32 {
		(pixel0[0] as i16 - pixel1[0] as i16).abs() as u32 +
		(pixel0[1] as i16 - pixel1[1] as i16).abs() as u32 +
		(pixel0[2] as i16 - pixel1[2] as i16).abs() as u32
	}

	let mut least_dif = u32::max_value();
	let mut best_card = 0u32;

	for (card_index, card_image) in card_images.iter().enumerate() {
		let mut current_dif = 0u32;

		/* all pixels sampled from the card image */
		for sample_x in 0..sample_size {
			let base_x = grid_x * sample_size + sample_x;

			for sample_y in 0..sample_size {
				let base_y = grid_y * sample_size + sample_y;

				current_dif += pixel_difference(
					pixel_at(sample_image.as_bytes(), sample_image.width(), base_x, base_y),
					pixel_at(card_image.as_bytes(), sample_size, sample_x, sample_y)
				);
			}
		}

		if current_dif < least_dif {
			least_dif = current_dif;
			best_card = card_index as u32;
		}
	}

	best_card
}

fn draw_cards(card_grid: &CardGrid, card_draw_images: Vec<RgbImage>, card_draw_indices: Vec<usize>, card_aspect: f32, image_width: u32) -> RgbImage {
	fn bilinear(bytes: &[u8], width: u32, height: u32, x: f32, y: f32) -> [u8; 3] {
		let pixel_x0 = x as u32;
		let pixel_x1 = (pixel_x0 + 1u32).min(width - 1);
		let weight_x1 = pixel_x0 as f32 - x;
		let weight_x0 = 1f32 - weight_x1;

		let pixel_y0 = y as u32;
		let pixel_y1 = (pixel_y0 + 1u32).min(height - 1);
		let weight_y1 = pixel_y0 as f32 - y;
		let weight_y0 = 1f32 - weight_y1;

		let channel_weight = |offset: u32| -> u8 {
			(
				(bytes[((pixel_y0 * width + pixel_x0) * 3u32 + offset) as usize] as f32 * weight_x0 * weight_y0) +
				(bytes[((pixel_y1 * width + pixel_x0) * 3u32 + offset) as usize] as f32 * weight_x0 * weight_y1) +
				(bytes[((pixel_y0 * width + pixel_x1) * 3u32 + offset) as usize] as f32 * weight_x1 * weight_y0) +
				(bytes[((pixel_y1 * width + pixel_x1) * 3u32 + offset) as usize] as f32 * weight_x1 * weight_y1)
			).round() as u8
		};

		[channel_weight(0), channel_weight(1), channel_weight(2)]
	}

	fn put_pixel(bytes: &mut [u8], pixel: &[u8;3], width: u32, x: u32, y: u32) {
		bytes[((y * width + x) * 3    ) as usize] = pixel[0];
		bytes[((y * width + x) * 3 + 1) as usize] = pixel[1];
		bytes[((y * width + x) * 3 + 2) as usize] = pixel[2];
	}

	let card_width = image_width as f32 / card_grid.cards_wide as f32;
	let card_height = (1f32 / card_aspect) * card_width;

	let image_height = (card_height * card_grid.cards_tall as f32).round() as u32;

	let mut draw_bytes = vec![0u8; (image_width * image_height * 3) as usize];

	for x in 0..card_grid.cards_wide {
		let min_x = (x as f32 * card_width).round() as u32;
		let max_x = ((x + 1u32) as f32 * card_width).round() as u32;
		let x_len = max_x - min_x;

		for y in 0..card_grid.cards_tall {
			let card_image = &card_draw_images[card_draw_indices[card_grid.grid[(y * card_grid.cards_wide + x) as usize] as usize]];
			let card_bytes = card_image.as_bytes();

			let min_y = (y as f32 * card_height).round() as u32;
			let max_y = ((y + 1u32) as f32 * card_height).round() as u32;
			let y_len = max_y - min_y;

			for x_along in 0..x_len {
				let draw_x = x_along + min_x;
				let card_x = (x_along as f32 / x_len as f32) * card_image.width() as f32;

				for y_along in 0..y_len {
					let draw_y = y_along + min_y;
					let card_y = (y_along as f32 / y_len as f32) * card_image.height() as f32;

					let pixel = bilinear(card_bytes, card_image.width(), card_image.height(), card_x, card_y);
					put_pixel(&mut draw_bytes, &pixel, image_width, draw_x, draw_y);
				}
			}
		}
	}

	RgbImage::from_raw(image_width, image_height, draw_bytes).unwrap()
}
