use image::{DynamicImage, RgbImage, EncodableLayout};
use crate::{CardGrid, create_card_samples, create_sample_image, get_card};

fn pixel_at(bytes: &[u8], width: u32, x: u32, y: u32) -> (u8, u8, u8) {
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
	sample_size: u32
) {
	let sample_image = create_sample_image(base_image, sample_size, card_grid.cards_wide, card_grid.cards_tall);
	let sample_cards = create_card_samples(card_images, sample_size);

	let (optimal_cards, mut columns) = rank_all_cards(
		&sample_image,
		&sample_cards,
		card_grid.cards_wide,
		card_grid.cards_tall,
		sample_size,
	);

	let mut select_grid = create_select_grid(card_grid.cards_wide, card_grid.cards_tall);

	rank_selection(&mut select_grid, &mut columns);
	fill_in_rest(&mut select_grid, &optimal_cards);

	for i in 0..card_grid.grid.len() {
		card_grid.grid[i] = select_grid[i] as u32;
	}
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
) -> (Vec<u32>, Vec<Vec<ColumnEntry>>) {
	let columns = ranks_to_columns(
		sample_cards.iter().map(|sample_card| rank_card(
			sample_image,
			&sample_card,
			cards_wide,
			cards_tall,
			sample_size
		)).collect::<Vec<Vec<u32>>>(),
		cards_wide,
		cards_tall
	);

	let optimal_cards = (0..cards_wide * cards_tall)
		.map(|spot| columns[spot as usize].last().unwrap().id)
		.collect::<Vec<u32>>();

	(optimal_cards, columns)
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

/**
 * first, insert each card at least once
 */
pub fn rank_selection(
	select_grid: &mut Vec<i32>,
	columns: &mut Vec<Vec<ColumnEntry>>,
) {
	let num_cards = columns[0].len();
	for _ in 0..num_cards {
		/* find the column with the lowest, optimal, value */
		let (best_index, best_column) = columns.iter()
			.enumerate()
			.filter(|(i, _)| select_grid[*i] == -1)
			.min_by(|(_, column0), (_, column1)|
				column0.last().unwrap()
					.difference.partial_cmp(&column1.last().unwrap().difference).unwrap()
			).unwrap();

		/* insert that card into that spot */
		let card_id = best_column.last().unwrap().id;
		select_grid[best_index] = card_id as i32;

		/* delete all of that card's entries in the columns */
		for spot in 0..select_grid.len() {
			let mut column = &mut columns[spot as usize];

			let (remove_index, _) = column.iter()
				.enumerate().rev().find(|(_, entry)| entry.id == card_id).unwrap();

			column.remove(remove_index);
		}
	}
}

/**
 * then, for spots that weren't chosen yet, find the optimal card
 */
pub fn fill_in_rest(
	select_grid: &mut Vec<i32>,
	optimal_cards: &Vec<u32>,
) {
	for spot in 0..select_grid.len() {
		if select_grid[spot as usize] == -1 {
			select_grid[spot as usize] = optimal_cards[spot as usize] as i32;
		}
	}
}
