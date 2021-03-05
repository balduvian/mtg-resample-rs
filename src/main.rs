
use reqwest::{Client};
use serde::Deserialize;
use image::{DynamicImage, GenericImageView, ImageFormat, Rgba, EncodableLayout, load_from_memory, GenericImage};
use image::imageops::{FilterType};
use uuid::Uuid;
use std::fs::create_dir;
use std::path::Path;
use std::convert::From;
use std::fs;
use std::cmp::min;

const ASPECT: f32 = 4.0f32 / 3.0f32;
const IMAGE_DIR: &str = "./cardImages/";
const BASE_IMAGE_DIR: &str = "./test/real scale.png";

#[tokio::main]
async fn main() {
	//save_num_cards(IMAGE_DIR, &mut card_images, ASPECT, 16).await.unwrap();

	setup_dir(IMAGE_DIR).unwrap();

	let mut card_images: Vec<DynamicImage> = Vec::with_capacity(32);

	load_existing_images(IMAGE_DIR, &mut card_images);

	println!("Loaded {} images!", card_images.len());

	let base_image = image::load_from_memory(fs::read(BASE_IMAGE_DIR).unwrap().as_slice()).unwrap();

	println!("Loaded base image!");

	let mut card_grid = create_grid(128, ASPECT, base_image.width(), base_image.height());

	populate_grid(&base_image, &card_images, &mut card_grid, ASPECT);

	println!("Found cards to sample!");

	let output_image = draw_cards(&card_grid, &card_images, ASPECT, 2000);

	println!("Drew sampled image!");

	output_image.save_with_format("./test/sampled.png", ImageFormat::Png).unwrap();
}

#[derive(Deserialize)]
struct ImageUris {
	art_crop: String
}

#[derive(Deserialize)]
struct CardInfo {
	id: String,
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

	for c in 0..num_cards {
		let (card_image, card_uuid) = get_card(&client).await?;
		save_card(card_image, card_uuid, card_aspect, dir_path, card_images);
	}

	Ok(())
}

async fn get_card(client: &Client) -> Result<(DynamicImage, Uuid), Box<dyn std::error::Error>> {
	let response = client.get("https://api.scryfall.com/cards/random").send().await?;

	let card_info = response.json::<CardInfo>().await?;

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

struct CardGrid {
	grid: Vec<u32>,
	cards_wide: u32,
	cards_tall: u32
}

fn populate_grid(base_image: &DynamicImage, card_images: &Vec<DynamicImage>, card_grid: &mut CardGrid, card_aspect: f32) {
	for x in 0..card_grid.cards_wide {
		for y in 0..card_grid.cards_tall {
			card_grid.grid[(y * card_grid.cards_wide + x) as usize] = select_best_card(&base_image, &card_images, &card_grid, x, y);
		}
	}
}

fn create_grid(cards_wide: u32, card_aspect: f32, image_width: u32, image_height: u32) -> CardGrid {
	let card_width = image_width as f32 / cards_wide as f32;
	let card_height = card_width * (1f32 / card_aspect);

	let cards_tall = (image_height as f32 / card_height).round() as u32;

	let grid = vec![0u32; (cards_wide * cards_tall) as usize];

	CardGrid { grid, cards_wide, cards_tall }
}

fn select_best_card(base_image: &DynamicImage, card_images: &Vec<DynamicImage>, card_grid: &CardGrid, x: u32, y: u32) -> u32 {
	let region_width = base_image.width() as f32 / card_grid.cards_wide as f32;
	let region_height = base_image.height() as f32 / card_grid.cards_tall as f32;

	let region_x = region_width * x as f32;
	let region_y = region_height * y as f32;

	let min_x = region_x.floor() as u32;
	let max_x = ((region_x + region_width).ceil() as u32).min(base_image.width() - 1);
	let len_x = max_x - min_x + 1;

	let min_y = region_y.floor() as u32;
	let max_y = ((region_y + region_height).ceil() as u32).min(base_image.height() - 1);
	let len_y = max_y - min_y + 1;

	fn dimension_weight(i: u32, int_min: u32, int_max: u32, min: f32, max: f32) -> f32 {
		match i {
			i if i == int_min => (int_min as f32 + 1.0f32) - min,
			i if i == int_max => max - (int_max as f32 - 1.0f32),
			_ => 1.0f32
		}
	}

	fn pixel_compare(pixel0: Rgba<u8>, pixel1: Rgba<u8>) -> u32 {
		(pixel0.0[0] as i16 - pixel1.0[0] as i16).abs() as u32 +
		(pixel0.0[1] as i16 - pixel1.0[1] as i16).abs() as u32 +
		(pixel0.0[2] as i16 - pixel1.0[2] as i16).abs() as u32
	}

	let mut least_dif = u32::max_value();
	let mut best_card = 0u32;

	for (card_index, card_image) in card_images.iter().enumerate() {
		let mut current_dif = 0u32;

		/* all pixels sampled from the card image */
		for x in 0..len_x {
			let base_x = x + min_x;
			let weight_x = dimension_weight(base_x, min_x, max_x, region_x, region_x + region_width);
			let card_x = ((x as f32 / len_x as f32) * card_image.width() as f32) as u32;

			for y in 0..len_y {
				let base_y = y + min_y;
				let weight_y = dimension_weight(base_y, min_y, max_y, region_y, region_y + region_height);
				let card_y = ((y as f32 / len_y as f32) * card_image.height() as f32) as u32;

				let weight = weight_x * weight_y;

				current_dif += (pixel_compare(base_image.get_pixel(base_x, base_y), card_image.get_pixel(card_x, card_y)) as f32 * weight) as u32;
			}
		}

		if current_dif < least_dif {
			least_dif = current_dif;
			best_card = card_index as u32;
		}
	}

	best_card
}

fn draw_cards(card_grid: &CardGrid, card_images: &Vec<DynamicImage>, card_aspect: f32, image_width: u32) -> DynamicImage {
	let card_width = image_width as f32 / card_grid.cards_wide as f32;
	let card_height = (1f32 / card_aspect) * card_width;

	let image_height = (card_height * card_grid.cards_tall as f32).round() as u32;

	let mut draw_image = image::DynamicImage::new_rgb8(image_width, image_height);

	for x in 0..card_grid.cards_wide {
		let min_x = (x as f32 * card_width).round() as u32;
		let max_x = ((x + 1u32) as f32 * card_width).round() as u32;
		let x_len = max_x - min_x;

		for y in 0..card_grid.cards_tall {
			let card_image = &card_images[card_grid.grid[(y * card_grid.cards_wide + x) as usize] as usize];

			let min_y = (y as f32 * card_height).round() as u32;
			let max_y = ((y + 1u32) as f32 * card_height).round() as u32;
			let y_len = max_y - min_y;

			for x_along in 0..x_len {
				let draw_x = x_along + min_x;
				let card_x = ((x_along as f32 / x_len as f32) * card_image.width() as f32) as u32;

				for y_along in 0..y_len {
					let draw_y = y_along + min_y;
					let card_y = ((y_along as f32 / y_len as f32) * card_image.height() as f32) as u32;

					draw_image.put_pixel(draw_x, draw_y, card_image.get_pixel(card_x, card_y));
				}
			}
		}
	}

	draw_image
}