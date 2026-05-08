use anyhow::{Error, Result, ensure};

#[derive(Clone, Debug)]
pub struct Move(u32);

impl Move {
    pub fn as_u32(&self) -> u32 {
        self.0
    }

    pub fn is_null(&self) -> bool {
        self.0 == 0
    }
}

impl PartialEq for Move {
    fn eq(&self, other: &Self) -> bool {
        self.0 == other.0
    }
}

impl Eq for Move {}

impl TryFrom<&str> for Move {
    type Error = Error;
    fn try_from(value: &str) -> Result<Self> {
        let value = value.as_bytes();
        ensure!(
            value.len() == 4,
            "A move must be 4 characters, got {} characters",
            value.len()
        );

        if value[0] == value[1] && value[1] == value[2] && value[2] == value[3] && value[3] == b'0'
        {
            return Ok(Self(0));
        }
        ensure!(
            b'a' <= value[0] && value[0] <= b'i',
            "File must be between 'a' to 'i'"
        );
        ensure!(
            b'0' <= value[1] && value[1] <= b'9',
            "Rank must be between '0' to '9'"
        );
        ensure!(
            b'a' <= value[2] && value[2] <= b'i',
            "File must be between 'a' to 'i'"
        );
        ensure!(
            b'0' <= value[3] && value[3] <= b'9',
            "Rank must be between '0' to '9'"
        );
        let r1 = value[0] - b'a';
        let f1 = value[1] - b'0';
        let r2 = value[2] - b'a';
        let f2 = value[3] - b'0';
        Ok(Self(
            (r1 as u32) | ((f1 as u32) << 8) | ((r2 as u32) << 16) | ((f2 as u32) << 24),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_valid_move_parsing() {
        // Test a standard move
        let m = Move::try_from("a0b1").expect("Should parse valid move");

        // Manual verification of the bit packing:
        // r1: 'a' - 'a' = 0  (0x00)
        // f1: '0' - '0' = 0  (0x00)
        // r2: 'b' - 'a' = 1  (0x01)
        // f2: '1' - '0' = 1  (0x01)
        // Result: 0x01010000 (in hex)
        assert_eq!(m.as_u32(), 0x01010000);
    }

    #[test]
    fn test_boundary_moves() {
        // Test the maximum bounds allowed by your ensures ('i' and '9')
        let m = Move::try_from("i9i9").expect("Should parse boundary move");

        let r_max = (b'i' - b'a') as u32; // 8
        let f_max = (b'9' - b'0') as u32; // 9
        let expected = r_max | (f_max << 8) | (r_max << 16) | (f_max << 24);

        assert_eq!(m.as_u32(), expected);
    }

    #[test]
    fn test_invalid_length() {
        assert!(Move::try_from("a0b").is_err()); // Too short
        assert!(Move::try_from("a0b1c").is_err()); // Too long
    }

    #[test]
    fn test_invalid_characters() {
        // Test out of range ranks
        assert!(Move::try_from("j0b1").is_err());
        // Test out of range files
        assert!(Move::try_from("a:b1").is_err()); // ':' is b'0' + 10
        // Test completely wrong characters
        assert!(Move::try_from("????").is_err());
    }

    #[test]
    fn test_equality() {
        let m1 = Move::try_from("a1c3").unwrap();
        let m2 = Move::try_from("a1c3").unwrap();
        let m3 = Move::try_from("c3a1").unwrap();

        assert_eq!(m1, m2);
        assert_ne!(m1, m3);
    }
}
