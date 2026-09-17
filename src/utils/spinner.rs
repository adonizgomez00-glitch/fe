#[allow(dead_code)]
pub struct Spinner {
    message: String,
    chars: Vec<char>,
    position: usize,
}

#[allow(dead_code)]
impl Spinner {
    pub fn new(message: &str) -> Self {
        Self {
            message: message.to_string(),
            chars: vec!['⠋', '⠙', '⠹', '⠸', '⠼', '⠴', '⠦', '⠧', '⠇', '⠏'],
            position: 0,
        }
    }

    pub fn tick(&mut self) {
        let c = self.chars[self.position % self.chars.len()];
        print!("\r{} {}", c, self.message);
        use std::io::Write;
        std::io::stdout().flush().expect("flush");
        self.position += 1;
    }

    pub fn done(&self, message: &str) {
        println!("\r✅ {}", message);
    }

    pub fn fail(&self, message: &str) {
        println!("\r❌ {}", message);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_spinner_advances() {
        let mut s = Spinner::new("testing");
        let pos1 = s.position;
        s.tick();
        assert_eq!(s.position, pos1 + 1);
    }
}
