//! Animated XBM easter egg from Pi's interactive mode.

use std::time::{SystemTime, UNIX_EPOCH};
use zedflow_tui::Component;

const WIDTH: usize = 31;
const HEIGHT: usize = 36;
const BYTES_PER_ROW: usize = WIDTH.div_ceil(8);
const DISPLAY_HEIGHT: usize = HEIGHT.div_ceil(2);
const BITS: [u8; 144] = [
    0xff, 0xff, 0xff, 0x7f, 0xff, 0xf0, 0xff, 0x7f, 0xff, 0xed, 0xff, 0x7f, 0xff, 0xdb, 0xff, 0x7f,
    0xff, 0xb7, 0xff, 0x7f, 0xff, 0x77, 0xfe, 0x7f, 0x3f, 0xf8, 0xfe, 0x7f, 0xdf, 0xff, 0xfe, 0x7f,
    0xdf, 0x3f, 0xfc, 0x7f, 0x9f, 0xc3, 0xfb, 0x7f, 0x6f, 0xfc, 0xf4, 0x7f, 0xf7, 0x0f, 0xf7, 0x7f,
    0xf7, 0xff, 0xf7, 0x7f, 0xf7, 0xff, 0xe3, 0x7f, 0xf7, 0x07, 0xe8, 0x7f, 0xef, 0xf8, 0x67, 0x70,
    0x0f, 0xff, 0xbb, 0x6f, 0xf1, 0x00, 0xd0, 0x5b, 0xfd, 0x3f, 0xec, 0x53, 0xc1, 0xff, 0xef, 0x57,
    0x9f, 0xfd, 0xee, 0x5f, 0x9f, 0xfc, 0xae, 0x5f, 0x1f, 0x78, 0xac, 0x5f, 0x3f, 0x00, 0x50, 0x6c,
    0x7f, 0x00, 0xdc, 0x77, 0xff, 0xc0, 0x3f, 0x78, 0xff, 0x01, 0xf8, 0x7f, 0xff, 0x03, 0x9c, 0x78,
    0xff, 0x07, 0x8c, 0x7c, 0xff, 0x0f, 0xce, 0x78, 0xff, 0xff, 0xcf, 0x7f, 0xff, 0xff, 0xcf, 0x78,
    0xff, 0xff, 0xdf, 0x78, 0xff, 0xff, 0xdf, 0x7d, 0xff, 0xff, 0x3f, 0x7e, 0xff, 0xff, 0xff, 0x7f,
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Effect {
    Typewriter,
    Scanline,
    Rain,
    Fade,
    Crt,
    Glitch,
    Dissolve,
}

const EFFECTS: [Effect; 7] = [
    Effect::Typewriter,
    Effect::Scanline,
    Effect::Rain,
    Effect::Fade,
    Effect::Crt,
    Effect::Glitch,
    Effect::Dissolve,
];

#[derive(Debug, Clone)]
enum EffectState {
    Typewriter {
        pos: usize,
    },
    Scanline {
        row: usize,
    },
    Rain {
        drops: Vec<Drop>,
    },
    Fade {
        positions: Vec<(usize, usize)>,
        idx: usize,
    },
    Crt {
        expansion: usize,
    },
    Glitch {
        phase: usize,
        glitch_frames: usize,
    },
    Dissolve {
        positions: Vec<(usize, usize)>,
        idx: usize,
    },
}

#[derive(Debug, Clone)]
struct Drop {
    y: isize,
    settled: usize,
}

/// Pi's `/arminsayshi` bitmap and its selected reveal animation.
#[derive(Debug, Clone)]
pub struct ArminComponent {
    effect: Effect,
    final_grid: Vec<Vec<char>>,
    current_grid: Vec<Vec<char>>,
    state: EffectState,
    rng: u64,
    running: bool,
}

impl ArminComponent {
    #[must_use]
    pub fn new() -> Self {
        let seed = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |duration| duration.as_nanos() as u64);
        Self::with_effect(EFFECTS[(seed as usize) % EFFECTS.len()], seed)
    }

    /// Construct a chosen effect; `seed` makes the random Pi effects testable.
    #[must_use]
    pub fn with_effect(effect: Effect, seed: u64) -> Self {
        let final_grid = build_final_grid();
        let mut component = Self {
            effect,
            final_grid,
            current_grid: empty_grid(),
            state: EffectState::Typewriter { pos: 0 },
            rng: seed,
            running: true,
        };
        component.state = component.initial_state();
        component
    }

    #[must_use]
    pub const fn effect(&self) -> Effect {
        self.effect
    }

    #[must_use]
    pub const fn fps(&self) -> u8 {
        if matches!(self.effect, Effect::Glitch) {
            60
        } else {
            30
        }
    }

    #[must_use]
    pub const fn is_running(&self) -> bool {
        self.running
    }

    /// Advance one Pi animation interval. The interactive runtime redraws after this call.
    pub fn tick(&mut self) -> bool {
        if !self.running {
            return true;
        }
        let done = self.tick_effect();
        if done {
            self.running = false;
        }
        done
    }

    pub fn dispose(&mut self) {
        self.running = false;
    }

    fn initial_state(&mut self) -> EffectState {
        match self.effect {
            Effect::Typewriter => EffectState::Typewriter { pos: 0 },
            Effect::Scanline => EffectState::Scanline { row: 0 },
            Effect::Rain => EffectState::Rain {
                drops: (0..WIDTH)
                    .map(|_| Drop {
                        y: -(self.random(DISPLAY_HEIGHT * 2) as isize),
                        settled: 0,
                    })
                    .collect(),
            },
            Effect::Fade => EffectState::Fade {
                positions: self.shuffled_positions(),
                idx: 0,
            },
            Effect::Crt => EffectState::Crt { expansion: 0 },
            Effect::Glitch => EffectState::Glitch {
                phase: 0,
                glitch_frames: 8,
            },
            Effect::Dissolve => {
                self.current_grid = (0..DISPLAY_HEIGHT)
                    .map(|_| {
                        (0..WIDTH)
                            .map(|_| [' ', '░', '▒', '▓', '█', '▀', '▄'][self.random(7)])
                            .collect()
                    })
                    .collect();
                EffectState::Dissolve {
                    positions: self.shuffled_positions(),
                    idx: 0,
                }
            }
        }
    }

    fn tick_effect(&mut self) -> bool {
        match &mut self.state {
            EffectState::Typewriter { pos } => {
                for _ in 0..3 {
                    let row = *pos / WIDTH;
                    if row >= DISPLAY_HEIGHT {
                        return true;
                    }
                    self.current_grid[row][*pos % WIDTH] = self.final_grid[row][*pos % WIDTH];
                    *pos += 1;
                }
                false
            }
            EffectState::Scanline { row } => {
                if *row >= DISPLAY_HEIGHT {
                    return true;
                }
                self.current_grid[*row].clone_from(&self.final_grid[*row]);
                *row += 1;
                false
            }
            EffectState::Rain { drops } => {
                self.current_grid = empty_grid();
                let mut all_settled = true;
                for (x, drop) in drops.iter_mut().enumerate() {
                    for row in (DISPLAY_HEIGHT.saturating_sub(drop.settled)..DISPLAY_HEIGHT).rev() {
                        self.current_grid[row][x] = self.final_grid[row][x];
                    }
                    if drop.settled >= DISPLAY_HEIGHT {
                        continue;
                    }
                    all_settled = false;
                    let target = (0..DISPLAY_HEIGHT.saturating_sub(drop.settled))
                        .rev()
                        .find(|&row| self.final_grid[row][x] != ' ');
                    drop.y += 1;
                    if (0..DISPLAY_HEIGHT as isize).contains(&drop.y) {
                        if target.is_some_and(|row| drop.y as usize >= row) {
                            drop.settled = DISPLAY_HEIGHT - target.unwrap();
                            drop.y = -(next_random(&mut self.rng, 5) as isize) - 1;
                        } else {
                            self.current_grid[drop.y as usize][x] = '▓';
                        }
                    }
                }
                all_settled
            }
            EffectState::Fade { positions, idx } => {
                for _ in 0..15 {
                    if *idx >= positions.len() {
                        return true;
                    }
                    let (row, x) = positions[*idx];
                    self.current_grid[row][x] = self.final_grid[row][x];
                    *idx += 1;
                }
                false
            }
            EffectState::Dissolve { positions, idx } => {
                for _ in 0..20 {
                    if *idx >= positions.len() {
                        return true;
                    }
                    let (row, x) = positions[*idx];
                    self.current_grid[row][x] = self.final_grid[row][x];
                    *idx += 1;
                }
                false
            }
            EffectState::Crt { expansion } => {
                self.current_grid = empty_grid();
                let middle = DISPLAY_HEIGHT / 2;
                for row in middle.saturating_sub(*expansion)
                    ..=(middle + *expansion).min(DISPLAY_HEIGHT - 1)
                {
                    self.current_grid[row].clone_from(&self.final_grid[row]);
                }
                *expansion += 1;
                *expansion > DISPLAY_HEIGHT
            }
            EffectState::Glitch {
                phase,
                glitch_frames,
            } => {
                if *phase >= *glitch_frames {
                    self.current_grid.clone_from(&self.final_grid);
                    return true;
                }
                let mut grid = Vec::with_capacity(DISPLAY_HEIGHT);
                for row in 0..DISPLAY_HEIGHT {
                    let mut line = self.final_grid[row].clone();
                    let offset = next_random(&mut self.rng, 7) as isize - 3;
                    if next_random(&mut self.rng, 10) < 3 {
                        line.rotate_left(offset.rem_euclid(WIDTH as isize) as usize);
                    }
                    if next_random(&mut self.rng, 10) < 2 {
                        let swap_row = next_random(&mut self.rng, DISPLAY_HEIGHT);
                        line = self.final_grid[swap_row].clone();
                    }
                    grid.push(line);
                }
                self.current_grid = grid;
                *phase += 1;
                false
            }
        }
    }

    fn random(&mut self, upper: usize) -> usize {
        next_random(&mut self.rng, upper)
    }

    fn shuffled_positions(&mut self) -> Vec<(usize, usize)> {
        let mut positions = (0..DISPLAY_HEIGHT)
            .flat_map(|row| (0..WIDTH).map(move |x| (row, x)))
            .collect::<Vec<_>>();
        for i in (1..positions.len()).rev() {
            positions.swap(i, self.random(i + 1));
        }
        positions
    }
}

impl Default for ArminComponent {
    fn default() -> Self {
        Self::new()
    }
}

impl Component for ArminComponent {
    fn render(&self, width: usize) -> Vec<String> {
        let available = width.saturating_sub(1);
        let mut lines = self
            .current_grid
            .iter()
            .map(|row| {
                let content: String = row.iter().take(available).collect();
                format!(
                    " {content}{:width$}",
                    "",
                    width = available.saturating_sub(content.chars().count())
                )
            })
            .collect::<Vec<_>>();
        let message = "ARMIN SAYS HI";
        lines.push(format!(
            " {message}{:width$}",
            "",
            width = available.saturating_sub(message.len())
        ));
        lines
    }
}

fn next_random(state: &mut u64, upper: usize) -> usize {
    *state = state.wrapping_mul(6364136223846793005).wrapping_add(1);
    (*state as usize) % upper
}

fn pixel(x: usize, y: usize) -> bool {
    y < HEIGHT && ((BITS[y * BYTES_PER_ROW + x / 8] >> (x % 8)) & 1) == 0
}

fn build_final_grid() -> Vec<Vec<char>> {
    (0..DISPLAY_HEIGHT)
        .map(|row| {
            (0..WIDTH)
                .map(|x| match (pixel(x, row * 2), pixel(x, row * 2 + 1)) {
                    (true, true) => '█',
                    (true, false) => '▀',
                    (false, true) => '▄',
                    (false, false) => ' ',
                })
                .collect()
        })
        .collect()
}

fn empty_grid() -> Vec<Vec<char>> {
    vec![vec![' '; WIDTH]; DISPLAY_HEIGHT]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scanline_reveals_the_frozen_xbm_and_stops() {
        let mut armin = ArminComponent::with_effect(Effect::Scanline, 0);
        assert_eq!(armin.render(32)[0], " ".repeat(32));
        for _ in 0..DISPLAY_HEIGHT {
            assert!(!armin.tick());
        }
        assert!(armin.tick());
        assert!(!armin.is_running());
        assert_eq!(
            armin.render(32)[0],
            format!(" {}", armin.final_grid[0].iter().collect::<String>())
        );
        assert_eq!(
            armin.render(32)[DISPLAY_HEIGHT],
            " ARMIN SAYS HI                  "
        );
    }

    #[test]
    fn half_blocks_follow_xbm_bit_order() {
        assert!(!pixel(0, 0));
        assert!(pixel(8, 1));
        assert_eq!(build_final_grid()[0].len(), WIDTH);
    }
}
