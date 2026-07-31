//! Animated XBM easter egg from Pi's `/arminsayshi` command.

use std::time::{SystemTime, UNIX_EPOCH};

use zedflow_tui::Component;

const WIDTH: usize = 31;
const HEIGHT: usize = 36;
const DISPLAY_HEIGHT: usize = HEIGHT.div_ceil(2);
const BITS: [u8; 144] = [
    0xff, 0xff, 0xff, 0x7f, 0xff, 0xf0, 0xff, 0x7f, 0xff, 0xed, 0xff, 0x7f, 0xff,
    0xdb, 0xff, 0x7f, 0xff, 0xb7, 0xff, 0x7f, 0xff, 0x77, 0xfe, 0x7f, 0x3f, 0xf8,
    0xfe, 0x7f, 0xdf, 0xff, 0xfe, 0x7f, 0xdf, 0x3f, 0xfc, 0x7f, 0x9f, 0xc3, 0xfb,
    0x7f, 0x6f, 0xfc, 0xf4, 0x7f, 0xf7, 0x0f, 0xf7, 0x7f, 0xf7, 0xff, 0xf7, 0x7f,
    0xf7, 0xff, 0xe3, 0x7f, 0xf7, 0x07, 0xe8, 0x7f, 0xef, 0xf8, 0x67, 0x70, 0x0f,
    0xff, 0xbb, 0x6f, 0xf1, 0x00, 0xd0, 0x5b, 0xfd, 0x3f, 0xec, 0x53, 0xc1, 0xff,
    0xef, 0x57, 0x9f, 0xfd, 0xee, 0x5f, 0x9f, 0xfc, 0xae, 0x5f, 0x1f, 0x78, 0xac,
    0x5f, 0x3f, 0x00, 0x50, 0x6c, 0x7f, 0x00, 0xdc, 0x77, 0xff, 0xc0, 0x3f, 0x78,
    0xff, 0x01, 0xf8, 0x7f, 0xff, 0x03, 0x9c, 0x78, 0xff, 0x07, 0x8c, 0x7c, 0xff,
    0x0f, 0xce, 0x78, 0xff, 0xff, 0xcf, 0x7f, 0xff, 0xff, 0xcf, 0x78, 0xff, 0xff,
    0xdf, 0x78, 0xff, 0xff, 0xdf, 0x7d, 0xff, 0xff, 0x3f, 0x7e, 0xff, 0xff, 0xff,
    0x7f,
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArminEffect { Typewriter, Scanline, Rain, Fade, Crt, Glitch, Dissolve }

impl ArminEffect {
    const ALL: [Self; 7] = [Self::Typewriter, Self::Scanline, Self::Rain, Self::Fade, Self::Crt, Self::Glitch, Self::Dissolve];
}

#[derive(Debug, Clone)]
enum State {
    Typewriter { pos: usize }, Scanline { row: usize }, Rain { drops: Vec<(isize, usize)> },
    Reveal { positions: Vec<(usize, usize)>, index: usize }, Crt { expansion: usize },
    Glitch { phase: usize },
}

/// The animation is advanced by the interactive runtime at `frame_interval_ms`.
#[derive(Debug, Clone)]
pub struct ArminComponent {
    effect: ArminEffect,
    final_grid: Vec<Vec<char>>,
    current_grid: Vec<Vec<char>>,
    state: State,
    rng: u64,
    finished: bool,
}

impl ArminComponent {
    #[must_use]
    pub fn new() -> Self {
        let seed = SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |time| time.as_nanos() as u64);
        Self::with_effect(ArminEffect::ALL[seed as usize % ArminEffect::ALL.len()], seed)
    }

    #[must_use]
    pub fn with_effect(effect: ArminEffect, seed: u64) -> Self {
        let final_grid = final_grid();
        let mut result = Self { effect, final_grid, current_grid: empty_grid(), state: State::Typewriter { pos: 0 }, rng: seed, finished: false };
        result.state = match effect {
            ArminEffect::Typewriter => State::Typewriter { pos: 0 },
            ArminEffect::Scanline => State::Scanline { row: 0 },
            ArminEffect::Rain => State::Rain { drops: (0..WIDTH).map(|_| (-(result.random(DISPLAY_HEIGHT * 2) as isize), 0)).collect() },
            ArminEffect::Fade => State::Reveal { positions: result.shuffled_positions(), index: 0 },
            ArminEffect::Crt => State::Crt { expansion: 0 },
            ArminEffect::Glitch => State::Glitch { phase: 0 },
            ArminEffect::Dissolve => {
                result.current_grid = (0..DISPLAY_HEIGHT).map(|_| (0..WIDTH).map(|_| [' ', '░', '▒', '▓', '█', '▀', '▄'][result.random(7)]).collect()).collect();
                State::Reveal { positions: result.shuffled_positions(), index: 0 }
            }
        };
        result
    }

    #[must_use]
    pub const fn frame_interval_ms(&self) -> u64 { if matches!(self.effect, ArminEffect::Glitch) { 1000 / 60 } else { 1000 / 30 } }
    #[must_use]
    pub const fn is_finished(&self) -> bool { self.finished }
    pub fn dispose(&mut self) { self.finished = true; }

    /// Advances one Pi animation frame and returns whether the animation is complete.
    pub fn tick(&mut self) -> bool {
        if self.finished { return true; }
        self.finished = match self.effect {
            ArminEffect::Typewriter => self.typewriter(), ArminEffect::Scanline => self.scanline(),
            ArminEffect::Rain => self.rain(), ArminEffect::Fade => self.reveal(15), ArminEffect::Dissolve => self.reveal(20),
            ArminEffect::Crt => self.crt(), ArminEffect::Glitch => self.glitch(),
        };
        self.finished
    }

    fn typewriter(&mut self) -> bool {
        let State::Typewriter { pos } = &mut self.state else { unreachable!() };
        for _ in 0..3 { if *pos == WIDTH * DISPLAY_HEIGHT { return true; } let (row, x) = (*pos / WIDTH, *pos % WIDTH); self.current_grid[row][x] = self.final_grid[row][x]; *pos += 1; }
        false
    }
    fn scanline(&mut self) -> bool {
        let State::Scanline { row } = &mut self.state else { unreachable!() }; if *row >= DISPLAY_HEIGHT { return true; }
        self.current_grid[*row].clone_from(&self.final_grid[*row]); *row += 1; false
    }
    fn rain(&mut self) -> bool {
        let mut rng = self.rng;
        let State::Rain { drops } = &mut self.state else { unreachable!() }; self.current_grid = empty_grid(); let mut done = true;
        for (x, (y, settled)) in drops.iter_mut().enumerate() {
            for row in (DISPLAY_HEIGHT.saturating_sub(*settled)..DISPLAY_HEIGHT).rev() { self.current_grid[row][x] = self.final_grid[row][x]; }
            if *settled >= DISPLAY_HEIGHT { continue; } done = false;
            let target = (0..DISPLAY_HEIGHT.saturating_sub(*settled)).rev().find(|&row| self.final_grid[row][x] != ' ');
            *y += 1;
            if (0..DISPLAY_HEIGHT as isize).contains(y) { if target.is_some_and(|row| *y >= row as isize) { *settled = DISPLAY_HEIGHT - target.unwrap(); *y = -(next_random(&mut rng, 5) as isize) - 1; } else { self.current_grid[*y as usize][x] = '▓'; } }
        }
        self.rng = rng;
        done
    }
    fn reveal(&mut self, amount: usize) -> bool {
        let State::Reveal { positions, index } = &mut self.state else { unreachable!() };
        for _ in 0..amount { if *index == positions.len() { return true; } let (row, x) = positions[*index]; self.current_grid[row][x] = self.final_grid[row][x]; *index += 1; }
        false
    }
    fn crt(&mut self) -> bool {
        let State::Crt { expansion } = &mut self.state else { unreachable!() }; self.current_grid = empty_grid(); let mid = DISPLAY_HEIGHT / 2;
        for row in mid.saturating_sub(*expansion)..=(mid + *expansion).min(DISPLAY_HEIGHT - 1) { self.current_grid[row].clone_from(&self.final_grid[row]); }
        *expansion += 1; *expansion > DISPLAY_HEIGHT
    }
    fn glitch(&mut self) -> bool {
        let phase = match self.state { State::Glitch { phase } => phase, _ => unreachable!() };
        if phase >= 8 { self.current_grid.clone_from(&self.final_grid); return true; }
        let mut rng = self.rng;
        self.current_grid = (0..DISPLAY_HEIGHT).map(|row| { let offset = next_random(&mut rng, 7) as isize - 3; if next_random(&mut rng, 10) < 3 { (0..WIDTH).map(|x| self.final_grid[row][(x as isize + offset).rem_euclid(WIDTH as isize) as usize]).collect() } else if next_random(&mut rng, 10) < 2 { self.final_grid[next_random(&mut rng, DISPLAY_HEIGHT)].clone() } else { self.final_grid[row].clone() } }).collect();
        self.rng = rng;
        if let State::Glitch { phase } = &mut self.state { *phase += 1; }
        false
    }
    fn shuffled_positions(&mut self) -> Vec<(usize, usize)> { let mut positions: Vec<_> = (0..DISPLAY_HEIGHT).flat_map(|row| (0..WIDTH).map(move |x| (row, x))).collect(); for i in (1..positions.len()).rev() { let j = self.random(i + 1); positions.swap(i, j); } positions }
    fn random(&mut self, upper: usize) -> usize { next_random(&mut self.rng, upper) }
}

fn next_random(seed: &mut u64, upper: usize) -> usize {
    *seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
    (*seed >> 32) as usize % upper
}

impl Component for ArminComponent {
    fn render(&self, width: usize) -> Vec<String> {
        let content_width = width.saturating_sub(1);
        let mut lines: Vec<_> = self.current_grid.iter().map(|row| format!(" {:<content_width$}", row.iter().take(content_width).collect::<String>())).collect();
        lines.push(format!(" {:<content_width$}", "ARMIN SAYS HI".chars().take(content_width).collect::<String>()));
        lines
    }
}

fn empty_grid() -> Vec<Vec<char>> { vec![vec![' '; WIDTH]; DISPLAY_HEIGHT] }
fn final_grid() -> Vec<Vec<char>> { (0..DISPLAY_HEIGHT).map(|row| (0..WIDTH).map(|x| pixel(x, row * 2, &BITS).then_some(pixel(x, row * 2 + 1, &BITS)).map_or_else(|| if pixel(x, row * 2 + 1, &BITS) { '▄' } else { ' ' }, |lower| if lower { '█' } else { '▀' })).collect()).collect() }
fn pixel(x: usize, y: usize, bits: &[u8]) -> bool { y < HEIGHT && (bits[y * WIDTH.div_ceil(8) + x / 8] >> (x % 8)) & 1 == 0 }

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn scanline_reveals_bitmap_and_message() { let mut armin = ArminComponent::with_effect(ArminEffect::Scanline, 1); assert!(armin.render(31).iter().take(DISPLAY_HEIGHT).all(|line| line.trim().is_empty())); for _ in 0..=DISPLAY_HEIGHT { armin.tick(); } let lines = armin.render(31); assert_eq!(lines.len(), DISPLAY_HEIGHT + 1); assert_eq!(lines.last().unwrap().trim(), "ARMIN SAYS HI"); assert!(lines.iter().take(DISPLAY_HEIGHT).any(|line| line.contains('█') || line.contains('▀') || line.contains('▄'))); }
}
