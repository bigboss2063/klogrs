use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use termcolor::Color;

/// Color generator for assigning colors to pods
pub struct ColorGenerator {
    /// Current color index
    current_index: usize,
    /// Available colors
    colors: Vec<Color>,
}

impl ColorGenerator {
    /// Create a new color generator
    pub fn new() -> Self {
        Self {
            current_index: 0,
            colors: vec![
                Color::Red,
                Color::Green,
                Color::Blue,
                Color::Cyan,
                Color::Magenta,
                Color::Yellow,
                Color::White,
            ],
        }
    }

    /// Get the next color in the sequence
    pub fn next_color(&mut self) -> Color {
        let color = self.colors[self.current_index];
        self.current_index = (self.current_index + 1) % self.colors.len();
        color
    }

    /// Get a color based on a string hash
    pub fn color_for_string(&self, s: &str) -> Color {
        let mut hasher = DefaultHasher::new();
        s.hash(&mut hasher);
        let hash = hasher.finish();

        self.colors[(hash % self.colors.len() as u64) as usize]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_next_color() {
        let mut generator = ColorGenerator::new();

        // Check that colors cycle
        let first_color = generator.next_color();

        // Get all remaining colors
        for _ in 1..generator.colors.len() {
            generator.next_color();
        }

        // Check that we're back to the first color
        let next_color = generator.next_color();
        assert_eq!(first_color, next_color);
    }

    #[test]
    fn test_color_for_string() {
        let generator = ColorGenerator::new();

        // Same string should get same color
        let color1 = generator.color_for_string("test");
        let color2 = generator.color_for_string("test");
        assert_eq!(color1, color2);

        // Different strings likely get different colors
        // (not guaranteed due to hash collisions, but very likely)
        let color1 = generator.color_for_string("test1");
        let color2 = generator.color_for_string("test2");
        // We don't assert inequality here because hash collisions are possible
        assert_ne!(color1, color2);
    }
}
