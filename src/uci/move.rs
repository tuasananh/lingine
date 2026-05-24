use anyhow::{Error, Result, ensure};

/// A chess move as represented in the UCI protocol.
///
/// Stored as a `u32` with four byte-sized fields:
/// - bits  0– 7: source file (`src_file`, 0 = `'a'`, …, 8 = `'i'`)
/// - bits  8–15: source rank (`src_rank`, 0–9)
/// - bits 16–23: destination file (`dst_file`)
/// - bits 24–31: destination rank (`dst_rank`)
///
/// A null move is encoded as `0x00000000` (the string `"0000"`).
///
/// # Converting to an internal `engine::Move`
/// When the real engine processes a `position` command it will convert each
/// `UciMove` to its internal `u16` move encoding using the four accessors:
/// ```rust,ignore
/// engine::Move::from_squares(uci_mv.src_file(), uci_mv.src_rank(),
///                            uci_mv.dst_file(), uci_mv.dst_rank())
/// ```
///
/// # Limitations
/// - No [`fmt::Display`] implementation — moves cannot currently be formatted
///   back to UCI notation (e.g. for the `pv` field of [`UciInfo`]). Add
///   `Display` when the search layer produces `UciMove` values to report.
/// - Xiangqi has no promotion, so no promotion piece field is encoded.
#[derive(Clone, Debug)]
pub struct UciMove(u32);

impl UciMove {
    /// Source file index (0 = 'a', …, 8 = 'i').
    #[allow(dead_code)]
    pub fn src_file(&self) -> u8 {
        (self.0 & 0xFF) as u8
    }

    /// Source rank index (0–9).
    #[allow(dead_code)]
    pub fn src_rank(&self) -> u8 {
        ((self.0 >> 8) & 0xFF) as u8
    }

    /// Destination file index (0 = 'a', …, 8 = 'i').
    #[allow(dead_code)]
    pub fn dst_file(&self) -> u8 {
        ((self.0 >> 16) & 0xFF) as u8
    }

    /// Destination rank index (0–9).
    #[allow(dead_code)]
    pub fn dst_rank(&self) -> u8 {
        ((self.0 >> 24) & 0xFF) as u8
    }

    /// Returns `true` if this is the null move (`"0000"`).
    #[allow(dead_code)]
    pub fn is_null(&self) -> bool {
        self.0 == 0
    }
}

impl PartialEq for UciMove {
    fn eq(&self, other: &Self) -> bool {
        self.0 == other.0
    }
}

impl Eq for UciMove {}

impl TryFrom<&str> for UciMove {
    type Error = Error;
    fn try_from(value: &str) -> Result<Self> {
        let value = value.as_bytes();
        ensure!(
            value.len() == 4,
            "A move must be 4 characters, got {} characters",
            value.len()
        );

        // Null move: the string "0000".
        if value == b"0000" {
            return Ok(Self(0));
        }

        ensure!(
            b'a' <= value[0] && value[0] <= b'i',
            "Source file must be between 'a' and 'i'"
        );
        ensure!(
            b'0' <= value[1] && value[1] <= b'9',
            "Source rank must be between '0' and '9'"
        );
        ensure!(
            b'a' <= value[2] && value[2] <= b'i',
            "Destination file must be between 'a' and 'i'"
        );
        ensure!(
            b'0' <= value[3] && value[3] <= b'9',
            "Destination rank must be between '0' and '9'"
        );

        let from_file = (value[0] - b'a') as u32;
        let from_rank = (value[1] - b'0') as u32;
        let to_file = (value[2] - b'a') as u32;
        let to_rank = (value[3] - b'0') as u32;

        Ok(Self(
            from_file | (from_rank << 8) | (to_file << 16) | (to_rank << 24),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_valid_move_parsing() {
        let m = UciMove::try_from("a0b1").expect("Should parse valid move");
        // from: file 'a' = 0, rank '0' = 0
        // to:   file 'b' = 1, rank '1' = 1
        assert_eq!(m.src_file(), 0);
        assert_eq!(m.src_rank(), 0);
        assert_eq!(m.dst_file(), 1);
        assert_eq!(m.dst_rank(), 1);
    }

    #[test]
    fn test_boundary_moves() {
        let m = UciMove::try_from("i9i9").expect("Should parse boundary move");
        assert_eq!(m.src_file(), 8); // 'i' - 'a' = 8
        assert_eq!(m.src_rank(), 9); // '9' - '0' = 9
        assert_eq!(m.dst_file(), 8);
        assert_eq!(m.dst_rank(), 9);
    }

    #[test]
    fn test_null_move() {
        let m = UciMove::try_from("0000").expect("Should parse null move");
        assert!(m.is_null());
    }

    #[test]
    fn test_invalid_length() {
        assert!(UciMove::try_from("a0b").is_err()); // Too short
        assert!(UciMove::try_from("a0b1c").is_err()); // Too long
    }

    #[test]
    fn test_invalid_characters() {
        assert!(UciMove::try_from("j0b1").is_err()); // Source file out of range
        assert!(UciMove::try_from("a:b1").is_err()); // Source rank out of range
        assert!(UciMove::try_from("????").is_err()); // Completely wrong
    }

    #[test]
    fn test_equality() {
        let m1 = UciMove::try_from("a1c3").unwrap();
        let m2 = UciMove::try_from("a1c3").unwrap();
        let m3 = UciMove::try_from("c3a1").unwrap();

        assert_eq!(m1, m2);
        assert_ne!(m1, m3);
    }
}
