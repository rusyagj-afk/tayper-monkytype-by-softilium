use rand::Rng;
use std::time::{Duration, Instant};

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum TypingMode {
    Legit,
    Smooth,
    Flat,
}

#[derive(Clone, Debug)]
pub struct TimingProfile {
    pub base_char_delay: Duration,
    pub wave_phase_primary: f64,
    pub wave_phase_secondary: f64,
    pub typo_probability: f64,
    pub mode: TypingMode,
}

impl TimingProfile {
    pub fn new(target_wpm: f64, mode: TypingMode) -> Self {
        let mut rng = rand::thread_rng();
        let base_delay_us = (60.0 / (target_wpm * 5.0)) * 1_000_000.0;

        let typo_probability = match mode {
            TypingMode::Legit => 0.016,
            TypingMode::Smooth | TypingMode::Flat => 0.0,
        };

        Self {
            base_char_delay: Duration::from_micros(base_delay_us as u64),
            wave_phase_primary: rng.gen_range(0.0..std::f64::consts::TAU),
            wave_phase_secondary: rng.gen_range(0.0..std::f64::consts::TAU),
            typo_probability,
            mode,
        }
    }

    pub fn calculate_keystroke_delay(
        &self,
        elapsed: Duration,
        char_idx: usize,
        prev_char: Option<char>,
        current_char: char,
    ) -> (Duration, u64) {
        let mut rng = rand::thread_rng();

        if self.mode == TypingMode::Flat {
            let jitter_factor = rng.gen_range(-0.04..=0.04);
            let delay_us =
                (self.base_char_delay.as_micros() as f64 * (1.0 + jitter_factor)).max(10_000.0);
            return (Duration::from_micros(delay_us as u64), 0);
        }

        let elapsed_secs = elapsed.as_secs_f64();
        let ramp = if elapsed_secs < 1.4 {
            0.78 + (elapsed_secs / 1.4) * 0.22
        } else {
            1.0
        };

        let macro_wave = 0.09 * (0.32 * elapsed_secs + self.wave_phase_primary).sin()
            + 0.05 * (0.75 * elapsed_secs + self.wave_phase_secondary).sin();

        let mut speed_multiplier = ramp * (1.0 + macro_wave);

        if let Some(prev) = prev_char {
            if is_fast_digraph(prev, current_char) {
                speed_multiplier *= 1.22;
            } else if prev.eq_ignore_ascii_case(&current_char) {
                speed_multiplier *= 1.12;
            }
        }

        if char_idx > 0 {
            speed_multiplier *= 1.05;
        }

        let clamped_multiplier = speed_multiplier.clamp(0.65, 1.45);
        let modulated_us = self.base_char_delay.as_micros() as f64 / clamped_multiplier;

        let micro_jitter = rng.gen_range(-0.16..=0.20);
        let final_us = (modulated_us * (1.0 + micro_jitter)).max(15_000.0);

        let dwell_ms = rng.gen_range(38..=68);

        (Duration::from_micros(final_us as u64), dwell_ms)
    }

    pub fn calculate_space_delay(&self) -> (Duration, u64) {
        let mut rng = rand::thread_rng();

        if self.mode == TypingMode::Flat {
            let delay = self.base_char_delay;
            return (delay, 0);
        }

        let multiplier = rng.gen_range(1.28..=1.58);
        let space_us = self.base_char_delay.as_micros() as f64 * multiplier;
        let dwell_ms = rng.gen_range(42..=76);

        (Duration::from_micros(space_us as u64), dwell_ms)
    }

    pub fn should_generate_typo(&self, char_idx: usize) -> bool {
        if self.typo_probability <= 0.0 || char_idx == 0 {
            return false;
        }
        let mut rng = rand::thread_rng();
        rng.gen_bool(self.typo_probability)
    }
}

pub fn get_adjacent_typo(c: char) -> char {
    let lower = c.to_ascii_lowercase();
    match lower {
        'a' => 's',
        'b' => 'v',
        'c' => 'x',
        'd' => 'e',
        'e' => 'r',
        'f' => 'd',
        'g' => 'f',
        'h' => 'g',
        'i' => 'u',
        'j' => 'h',
        'k' => 'j',
        'l' => 'k',
        'm' => 'n',
        'n' => 'b',
        'o' => 'i',
        'p' => 'o',
        'q' => 'w',
        'r' => 'e',
        's' => 'a',
        't' => 'r',
        'u' => 'y',
        'v' => 'c',
        'w' => 'q',
        'x' => 'z',
        'y' => 't',
        'z' => 'x',
        'й' => 'ц',
        'ц' => 'у',
        'у' => 'к',
        'к' => 'е',
        'е' => 'н',
        'н' => 'г',
        'г' => 'ш',
        'ш' => 'щ',
        'щ' => 'з',
        'з' => 'х',
        'х' => 'з',
        'ф' => 'ы',
        'ы' => 'в',
        'в' => 'а',
        'а' => 'п',
        'п' => 'р',
        'р' => 'о',
        'о' => 'л',
        'л' => 'д',
        'д' => 'ж',
        'ж' => 'э',
        'э' => 'ж',
        'я' => 'ч',
        'ч' => 'с',
        'с' => 'м',
        'м' => 'и',
        'и' => 'т',
        'т' => 'ь',
        'ь' => 'б',
        'б' => 'ю',
        'ю' => 'б',
        _ => {
            if c.is_alphabetic() {
                'e'
            } else {
                ' '
            }
        }
    }
}

fn is_fast_digraph(prev: char, cur: char) -> bool {
    let p = prev.to_ascii_lowercase();
    let c = cur.to_ascii_lowercase();
    matches!(
        (p, c),
        ('t', 'h')
            | ('h', 'e')
            | ('i', 'n')
            | ('e', 'r')
            | ('a', 'n')
            | ('r', 'e')
            | ('e', 'd')
            | ('o', 'n')
            | ('e', 's')
            | ('s', 't')
            | ('e', 'n')
            | ('a', 't')
            | ('t', 'o')
            | ('n', 't')
            | ('h', 'a')
            | ('n', 'd')
            | ('o', 'u')
            | ('e', 'a')
            | ('n', 'g')
            | ('a', 's')
            | ('o', 'r')
            | ('t', 'i')
            | ('i', 's')
            | ('e', 't')
            | ('i', 't')
            | ('a', 'r')
            | ('t', 'e')
            | ('s', 'e')
            | ('h', 'i')
            | ('o', 'f')
            | ('с', 'т')
            | ('н', 'о')
            | ('т', 'о')
            | ('н', 'а')
            | ('е', 'н')
            | ('о', 'в')
            | ('р', 'а')
            | ('в', 'о')
            | ('к', 'о')
            | ('р', 'о')
            | ('п', 'р')
            | ('н', 'и')
            | ('е', 'р')
            | ('л', 'и')
            | ('р', 'е')
            | ('о', 'с')
            | ('п', 'о')
            | ('о', 'р')
            | ('н', 'е')
            | ('а', 'л')
            | ('и', 'т')
            | ('е', 'л')
            | ('т', 'а')
            | ('о', 'м')
            | ('о', 'л')
    )
}

pub struct Pacer {
    start: Option<Instant>,
    target_accumulated: Duration,
}

impl Pacer {
    pub fn new() -> Self {
        Self {
            start: None,
            target_accumulated: Duration::ZERO,
        }
    }

    pub fn start(&mut self) {
        if self.start.is_none() {
            self.start = Some(Instant::now());
        }
    }

    pub fn elapsed(&self) -> Duration {
        self.start.map(|s| s.elapsed()).unwrap_or(Duration::ZERO)
    }

    pub async fn pace(&mut self, delay: Duration) {
        self.target_accumulated += delay;

        if let Some(start) = self.start {
            let target_instant = start + self.target_accumulated;
            let now = Instant::now();
            if target_instant > now {
                tokio::time::sleep(target_instant - now).await;
            }
        }
    }
}
