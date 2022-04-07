use image::{DynamicImage, RgbImage, EncodableLayout, ImageFormat};
use crate::{CardGrid, create_card_samples, create_sample_image, get_card};
use crate::preprocess::{add_duplicates, count_brightness, create_brightness_counts, create_brightness_map, match_brightness};

pub fn pixel_at(bytes: &[u8], width: u32, x: u32, y: u32) -> (u8, u8, u8) {
	(
		bytes[((width * y + x) * 3) as usize],
		bytes[((width * y + x) * 3 + 1) as usize],
		bytes[((width * y + x) * 3 + 2) as usize],
	)
}

fn pixel_difference(pixel0: (u8, u8, u8), pixel1: (u8, u8, u8)) -> u32 {
	(pixel0.0 as i32 - pixel1.0 as i32).abs() as u32 +
	(pixel0.1 as i32 - pixel1.1 as i32).abs() as u32 +
	(pixel0.2 as i32 - pixel1.2 as i32).abs() as u32
}

fn card_dif(
	base_bytes: &[u8],
    base_width: u32,
    card_bytes: &[u8],
    card_width: u32,
    x: u32,
    y: u32,
    sample_size: u32,
) -> u32 {
	let mut total_difference = 0;

	for j in 0..sample_size {
		for i in 0..sample_size {
			total_difference += pixel_difference(
				pixel_at(base_bytes, base_width,x + i, y + j),
				pixel_at(card_bytes, card_width, i, j),
			);
		}
	}

	total_difference
}

pub fn populate_grid_new(
	base_image: &DynamicImage,
	card_images: &Vec<DynamicImage>,
	card_grid: &mut CardGrid,
	sample_size: u32,
) {
	println!("Creating sample image...");
	let sample_image = create_sample_image(base_image, sample_size, card_grid.cards_wide, card_grid.cards_tall);
	println!("Creating sample cards...");
	let mut sample_cards = create_card_samples(card_images, sample_size);

	println!("brightness preprocessing...");
	let mut base_brightness_counts = create_brightness_counts();
	let mut card_brightness_counts = create_brightness_counts();

	count_brightness(&sample_image, &mut base_brightness_counts);
	for card in &sample_cards {
		count_brightness(card, &mut card_brightness_counts);
	}

	let brightness_map = create_brightness_map(&base_brightness_counts, &card_brightness_counts);
	let sample_image = match_brightness(&sample_image, &brightness_map);

	sample_image.save_with_format("./test/brightness-matched.png", ImageFormat::Png).unwrap();

	println!("Ranking cards...");
	let mut columns = rank_all_cards(
		&sample_image,
		&sample_cards,
		card_grid.cards_wide,
		card_grid.cards_tall,
		sample_size,
	);

	//println!("Determining visit order...");
	//let visit_order = create_visit_order(card_grid.cards_wide, card_grid.cards_tall);

	//println!("determining detail...");
	//let detail_order = create_detail_order(&sample_image, card_grid.cards_wide, card_grid.cards_tall, sample_size);

	println!("determining best fit...");
	let best_fit_order = create_best_fit_order(&columns);

	println!("Selecting cards...");
	rank_selection(&mut card_grid.grid, &mut columns, &best_fit_order);
}

/**
 * returns (optimal cards in each spot, full columns list)
 */
pub fn rank_all_cards(
	sample_image: &RgbImage,
	sample_cards: &Vec<RgbImage>,
	cards_wide: u32,
	cards_tall: u32,
	sample_size: u32,
) -> Vec<Vec<ColumnEntry>> {
	ranks_to_columns(
		sample_cards.iter().map(|sample_card| rank_card(
			sample_image,
			&sample_card,
			cards_wide,
			cards_tall,
			sample_size
		)).collect::<Vec<Vec<u32>>>(),
		cards_wide,
		cards_tall
	)
}

pub fn added_focus_cost(
	x: u32,
	y: u32,
	cards_wide: u32,
	cards_tall: u32,
	sample_size: u32,
) -> u32 {
	(
		(2.0_f32 * (x as f32 / cards_wide as f32) - 1.0_f32).powi(2) *
		(2.0_f32 * (y as f32 / cards_tall as f32) - 1.0_f32).powi(2) *
		(sample_size as f32).powi(2) * 255.0_f32
	).round() as u32
}

pub fn rank_card(
	sample_image: &RgbImage,
	sample_card: &RgbImage,
	cards_wide: u32,
	cards_tall: u32,
	sample_size: u32,
) -> Vec<u32> {
	let mut ranks = vec![0u32; (cards_wide * cards_tall) as usize];

	let base_bytes = sample_image.as_bytes();
	let card_bytes = sample_card.as_bytes();

	for j in 0..cards_tall {
		for i in 0..cards_wide {
			let difference = card_dif(
				base_bytes,
				sample_image.width(),
				card_bytes,
				sample_card.width(),
				i * sample_size,
				j * sample_size,
				sample_size,
			);

			ranks[(j * cards_wide + i) as usize] = difference + added_focus_cost(i, j, cards_wide, cards_tall, sample_size);
		}
	}

	ranks
}


pub fn create_detail_order(
	sample_image: &RgbImage,
	cards_wide: u32,
	cards_tall: u32,
	sample_size: u32,
) -> Vec<usize> {
	let base_bytes = sample_image.as_bytes();

	let mut sort_list = Vec::with_capacity((cards_wide * cards_tall) as usize);

	for spot in 0..(cards_wide * cards_tall) {
		let x = spot % cards_wide;
		let y = spot / cards_wide;
		let mut running_difference = 0;

		/* horizontal running difference */
		for j in y * sample_size..(y + 1) * sample_size {
			let mut last_pixel = pixel_at(base_bytes, sample_image.width(), x * sample_size, j);

			for i in x * sample_size + 1..(x + 1) * sample_size {
				let this_pixel = pixel_at(base_bytes, sample_image.width(), i, j);
				running_difference += pixel_difference(last_pixel, this_pixel);
				last_pixel = this_pixel;
			}
		}

		/* vertical running difference */
		for i in x * sample_size..(x + 1) * sample_size {
			let mut last_pixel = pixel_at(base_bytes, sample_image.width(), i, y * sample_size);

			for j in y * sample_size + 1..(y + 1) * sample_size {
				let this_pixel = pixel_at(base_bytes, sample_image.width(), i, j);
				running_difference += pixel_difference(last_pixel, this_pixel);
				last_pixel = this_pixel;
			}
		}

		match sort_list.binary_search_by(|item: &(u32, u32)| {
			item.1.cmp(&running_difference).reverse()
		}) {
			Ok(pos) => sort_list.insert(pos, (spot, running_difference)),
			Err(pos) => sort_list.insert(pos, (spot, running_difference)),
		}
	}

	sort_list.iter().map(|item| item.0 as usize).collect::<Vec<usize>>()
}

fn create_best_fit_order(
	columns: &Vec<Vec<ColumnEntry>>
) -> Vec<usize> {
	let mut sort_list = Vec::with_capacity(columns.len());

	for spot in 0..columns.len() {
		let best = columns[spot].last().unwrap();

		match sort_list.binary_search_by(|item: &(usize, u32)| {
			item.1.cmp(&best.difference)
		}) {
			Ok(pos) => sort_list.insert(pos, (spot, best.difference)),
			Err(pos) => sort_list.insert(pos, (spot, best.difference)),
		}
	}

	sort_list.iter().map(|item| item.0 as usize).collect::<Vec<usize>>()
}

pub struct ColumnEntry {
	difference: u32,
	id: u32,
}

pub fn ranks_to_columns(
	ranks: Vec<Vec<u32>>,
	cards_wide: u32,
	cards_tall: u32,
) -> Vec<Vec<ColumnEntry>> {
	let num_cards = ranks.len() as u32;

	let mut columns = Vec::with_capacity((cards_wide * cards_tall) as usize);

	for space in 0..(cards_wide * cards_tall) {
		let mut column = Vec::with_capacity(num_cards as usize);

		for card_index in 0..num_cards {
			column.push(ColumnEntry { difference: ranks[card_index as usize][space as usize], id: card_index });
		}

		/* sort by highest differences first (optimal will be last) */
		column.sort_unstable_by(|column0, column1| column0.difference.partial_cmp(&column1.difference).unwrap().reverse());
		columns.push(column);
	}

	columns
}

pub fn create_select_grid(
	cards_wide: u32,
	cards_tall: u32,
) -> Vec<i32> {
	vec![-1_i32; (cards_wide * cards_tall) as usize]
}

/* two step filler for the select grid */

pub fn distance_from_center_squared(
	x: u32,
	y: u32,
	cards_wide: u32,
	cards_tall: u32,
) -> f32 {
	((x as f32 + 0.5_f32) - (cards_wide as f32 / 2.0_f32)).powi(2) +
	((y as f32 + 0.5_f32) - (cards_tall as f32 / 2.0_f32)).powi(2)
}

fn create_visit_order(
	cards_wide: u32,
	cards_tall: u32,
) -> Vec<usize> {
	let distance_board = (0..cards_wide * cards_tall)
		.map(|spot| distance_from_center_squared(
			spot % cards_wide,
			spot / cards_wide,
			cards_wide,
			cards_tall,
		)).collect::<Vec<f32>>();

	let mut gotten_board = vec![false; (cards_wide * cards_tall) as usize];

	let mut order = Vec::with_capacity((cards_wide * cards_tall) as usize);

	let largest_circle =
		((cards_wide as f32 / 2.0_f32).powi(2) + (cards_tall as f32 / 2.0_f32).powi(2))
		.sqrt().ceil() as u32;

	/* circle rounds! */
	for radius in 1..=largest_circle {
		let radius = radius as f32;
		let inner_radius = radius * 2.0_f32.sqrt() / 2.0_f32;
		let radius_squared = radius.powi(2);

		let center_x = cards_wide as f32 / 2.0_f32;
		let center_y = cards_tall as f32 / 2.0_f32;

		/*
		 * #   #
		 *  # #
		 *  # #
		 * #   #
		 */

		let right_bound = cards_wide as i32 - 1;
		let down_bound = cards_tall as i32 - 1;

		let outer_left = ((center_x - radius).floor() as i32).max(0) as u32;
		let outer_right = ((center_x + radius).ceil() as i32).min(right_bound) as u32;
		let inner_left = ((center_x - inner_radius + 1_f32).ceil() as i32).max(0) as u32;
		let inner_right = ((center_x + inner_radius - 1_f32).floor() as i32).min(right_bound) as u32;

		let outer_up = ((center_y - radius).floor() as i32).max(0) as u32;
		let outer_down = ((center_y + radius).ceil() as i32).min(down_bound) as u32;
		let inner_up = ((center_y - inner_radius + 1_f32).ceil() as i32).max(0) as u32;
		let inner_down = ((center_y + inner_radius - 1_f32).floor() as i32).min(down_bound) as u32;

		let mut try_space = |x: u32, y: u32| {
			let spot = (y * cards_wide + x) as usize;
			if !gotten_board[spot] {
				if distance_board[spot] <= radius_squared {
					/* potentially add randomness?? */
					order.push(spot);
					gotten_board[spot] = true;
				}
			}
		};

		for x in outer_left..=outer_right {
			for y in outer_up..=inner_up {
				try_space(x, y);
			}
			for y in inner_down..=outer_down {
				try_space(x, y);
			}
		}
		for y in inner_up..=inner_down {
			for x in outer_left..=inner_left {
				try_space(x, y);
			}
			for x in inner_right..=outer_right {
				try_space(x, y);
			}
		}
	}

	order
}

/**
 * first, insert each card at least once
 */
pub fn rank_selection(
	card_grid: &mut Vec<u32>,
	columns: &mut Vec<Vec<ColumnEntry>>,
	visit_order: &Vec<usize>,
) {
	let num_cards = columns[0].len();

	/* place every unique card */
	sub_rank_selection(
		card_grid,
		columns,
		visit_order,
		0,
		num_cards.min(visit_order.len())
	);

	/* place remaining duplicates */
	sub_rank_selection(
		card_grid,
		columns,
		visit_order,
		num_cards,
		visit_order.len()
	);
}

fn sub_rank_selection(
	card_grid: &mut Vec<u32>,
	columns: &mut Vec<Vec<ColumnEntry>>,
	visit_order: &Vec<usize>,
	start_index: usize,
	end_index: usize,
) {
	for i in start_index..end_index {
		let best_index = visit_order[i];
		let best_column = &columns[best_index];

		/* insert that card into that spot */
		/* risky, must guarantee that the column has not run out */
		let card_id = best_column.last().unwrap().id;
		card_grid[best_index] = card_id;

		/* delete all of that card's entries in the future columns */
		/* leave the overflow duplicate untouched */
		for j in i + 1..end_index {
			let remove_index = visit_order[j];
			let column = &mut columns[remove_index];

			let (remove_index, _) = column.iter()
				.enumerate().rev().find(|(_, entry)| entry.id == card_id).unwrap();

			column.remove(remove_index);
		}
	}
}