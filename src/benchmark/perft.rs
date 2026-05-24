use anyhow::Result;

use crate::{
    movegen::generate_moves,
    position::Position,
    types::{MAX_MOVES, Move, MoveGenType},
};

#[derive(Debug)]
struct Perft {
    nodes: u64,
    checks: u64,
    captures: u64,
    mates: u64,
}

#[allow(dead_code)]
impl Perft {
    fn new() -> Self {
        Self {
            nodes: 0,
            checks: 0,
            captures: 0,
            mates: 0,
        }
    }

    pub fn perft(&mut self, fen: &str, depth: u64) -> Result<()> {
        let mut pos = Position::new();
        pos.set(fen)?;
        self.perft_helper::<true>(&mut pos, depth as u32);

        dbg!("Perft results for depth {}:\n{:?}", depth, self);
        Ok(())
    }

    fn perft_helper<const ROOT: bool>(&mut self, pos: &mut Position, depth: u32) -> u64 {
        let mut cnt;
        let mut sub_nodes = 0;

        let mut moves = [Move::none(); MAX_MOVES];
        let moves_count = generate_moves(pos, MoveGenType::Legal, &mut moves);

        let start_timepoint = if ROOT {
            Some(std::time::Instant::now())
        } else {
            None
        };

        let leaf = depth == 2;

        for i in 0..moves_count {
            let m = moves[i];
            if ROOT && depth <= 1 {
                cnt = 1;
                self.nodes += 1;
                sub_nodes += 1;

                if !pos.is_empty(m.square_to()) {
                    self.captures += 1;
                }
                if pos.gives_check(m) {
                    self.checks += 1;
                }
            } else {
                pos.do_move(m);

                if leaf {
                    let mut next_moves = [Move::none(); MAX_MOVES];
                    let next_count = generate_moves(pos, MoveGenType::Legal, &mut next_moves);

                    cnt = next_count as u64;

                    if cnt > 0 {
                        for j in 0..next_count {
                            let nm = next_moves[j];
                            if !pos.is_empty(nm.square_to()) {
                                self.captures += 1;
                            }

                            if pos.gives_check(nm) {
                                self.checks += 1;
                            }
                        }
                    } else {
                        self.mates += 1;
                    }

                    self.nodes += cnt;
                    sub_nodes += cnt;
                } else {
                    cnt = self.perft_helper::<false>(pos, depth - 1);
                    sub_nodes += cnt;
                }

                pos.undo_move(m);
            }

            if ROOT {
                println!("Move {:?}: {}", m, cnt);
            }
        }

        if ROOT {
            let elapsed = start_timepoint.unwrap().elapsed();
            println!("Time taken: {:.2?}", elapsed);
        }

        sub_nodes
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn assert_perft(
        fen: &str,
        depth: u64,
        nodes: u64,
        checks: u64,
        captures: u64,
        mates: u64,
    ) -> Result<()> {
        let mut perft = Perft::new();
        perft.perft(fen, depth)?;
        assert_eq!(
            perft.nodes, nodes,
            "Nodes mismatch at depth {} for FEN {}",
            depth, fen
        );
        assert_eq!(
            perft.checks, checks,
            "Checks mismatch at depth {} for FEN {}",
            depth, fen
        );
        assert_eq!(
            perft.captures, captures,
            "Captures mismatch at depth {} for FEN {}",
            depth, fen
        );
        assert_eq!(
            perft.mates, mates,
            "Mates mismatch at depth {} for FEN {}",
            depth, fen
        );
        Ok(())
    }

    #[test]
    fn test_perft_position_1() -> Result<()> {
        let fen = "rheakaehr/9/1c5c1/p1p1p1p1p/9/9/P1P1P1P1P/1C5C1/9/RHEAKAEHR w - - 0 1";
        assert_perft(fen, 1, 44, 0, 2, 0)?;
        assert_perft(fen, 2, 1920, 6, 72, 0)?;
        assert_perft(fen, 3, 79666, 384, 3159, 0)?;
        assert_perft(fen, 4, 3290240, 19380, 115365, 0)?;
        assert_perft(fen, 5, 133312995, 953251, 4917734, 0)?;
        Ok(())
    }

    #[test]
    fn test_perft_position_2() -> Result<()> {
        let fen = "r1ea1a3/4kh3/2h1e4/pHp1p1p1p/4c4/6P2/P1P2R2P/1CcC5/9/2EAKAE2 w - - 0 1";
        assert_perft(fen, 1, 38, 1, 1, 0)?;
        assert_perft(fen, 2, 1128, 12, 10, 0)?;
        assert_perft(fen, 3, 43929, 1190, 2105, 0)?;
        assert_perft(fen, 4, 1339047, 21299, 31409, 23)?;
        assert_perft(fen, 5, 53112976, 1496697, 3262495, 1537)?;
        Ok(())
    }

    #[test]
    fn test_perft_position_3() -> Result<()> {
        let fen = "1ceak4/9/h2a5/2p1p3p/5cp2/2h2H3/6PCP/3AE4/2C6/3A1K1H1 w - - 0 1";
        assert_perft(fen, 1, 7, 0, 0, 0)?;
        assert_perft(fen, 2, 281, 9, 10, 0)?;
        assert_perft(fen, 3, 8620, 64, 540, 0)?;
        assert_perft(fen, 4, 326201, 10121, 11730, 0)?;
        assert_perft(fen, 5, 10369923, 162542, 660479, 3)?;
        Ok(())
    }

    #[test]
    fn test_perft_position_4() -> Result<()> {
        let fen = "5a3/3k5/3aR4/9/5r3/5h3/9/3A1A3/5K3/2EC2E2 w - - 0 1";
        assert_perft(fen, 1, 25, 3, 2, 0)?;
        assert_perft(fen, 2, 424, 9, 6, 0)?;
        assert_perft(fen, 3, 9850, 963, 771, 0)?;
        assert_perft(fen, 4, 202884, 8699, 9179, 12)?;
        assert_perft(fen, 5, 4739553, 380858, 326535, 161)?;
        Ok(())
    }

    #[test]
    fn test_perft_position_5() -> Result<()> {
        let fen = "CRH1k1e2/3ca4/4ea3/9/2hr5/9/9/4E4/4A4/4KA3 w - - 0 1";
        assert_perft(fen, 1, 28, 12, 1, 0)?;
        assert_perft(fen, 2, 516, 15, 7, 0)?;
        assert_perft(fen, 3, 14808, 3041, 675, 0)?;
        assert_perft(fen, 4, 395483, 12444, 11485, 162)?;
        assert_perft(fen, 5, 11842230, 1254024, 590801, 495)?;
        Ok(())
    }

    #[test]
    fn test_perft_position_6() -> Result<()> {
        let fen = "R1H1k1e2/9/3aea3/9/2hr5/2E6/9/4E4/4A4/4KA3 w - - 0 1";
        assert_perft(fen, 1, 21, 4, 1, 0)?;
        assert_perft(fen, 2, 364, 16, 22, 0)?;
        assert_perft(fen, 3, 7626, 784, 502, 0)?;
        assert_perft(fen, 4, 162837, 5865, 10683, 17)?;
        assert_perft(fen, 5, 3500505, 236182, 241214, 24)?;
        Ok(())
    }

    #[test]
    fn test_perft_position_7() -> Result<()> {
        let fen = "C1hHk4/9/9/9/9/9/h1pp5/E3C4/9/3A1K3 w - - 0 1";
        assert_perft(fen, 1, 28, 2, 0, 0)?;
        assert_perft(fen, 2, 222, 0, 12, 0)?;
        assert_perft(fen, 3, 6241, 464, 40, 0)?;
        assert_perft(fen, 4, 64971, 105, 4966, 0)?;
        assert_perft(fen, 5, 1914306, 116319, 17459, 0)?;
        Ok(())
    }

    #[test]
    fn test_perft_position_8() -> Result<()> {
        let fen = "4ka3/4a4/9/9/4H4/p8/9/4C3c/7h1/2EK5 w - - 0 1";
        assert_perft(fen, 1, 23, 8, 1, 0)?;
        assert_perft(fen, 2, 345, 4, 6, 0)?;
        assert_perft(fen, 3, 8124, 1524, 179, 0)?;
        assert_perft(fen, 4, 149272, 3201, 2857, 32)?;
        assert_perft(fen, 5, 3513104, 390765, 57928, 274)?;
        Ok(())
    }

    #[test]
    fn test_perft_position_9() -> Result<()> {
        let fen = "2e1ka3/9/e3H4/4h4/9/9/9/4C4/2p6/2EK5 w - - 0 1";
        assert_perft(fen, 1, 21, 6, 1, 0)?;
        assert_perft(fen, 2, 195, 28, 30, 0)?;
        assert_perft(fen, 3, 3883, 411, 135, 0)?;
        assert_perft(fen, 4, 48060, 6419, 6087, 0)?;
        assert_perft(fen, 5, 933096, 71759, 26841, 0)?;
        Ok(())
    }

    #[test]
    fn test_perft_position_10() -> Result<()> {
        let fen = "1C2ka3/9/C1Hae1h2/p3p3p/6p2/9/P3P3P/3AE4/3p2c2/c1EAK4 w - - 0 1";
        assert_perft(fen, 1, 30, 2, 3, 0)?;
        assert_perft(fen, 2, 830, 55, 64, 0)?;
        assert_perft(fen, 3, 22787, 858, 2163, 0)?;
        assert_perft(fen, 4, 649866, 39719, 57076, 65)?;
        assert_perft(fen, 5, 17920736, 518816, 1625453, 44)?;
        Ok(())
    }

    #[test]
    fn test_perft_position_11() -> Result<()> {
        let fen = "ChH1k1e2/c3a4/4ea3/9/2hr5/9/9/4C4/4A4/4KA3 w - - 0 1";
        assert_perft(fen, 1, 19, 3, 2, 0)?;
        assert_perft(fen, 2, 583, 15, 14, 0)?;
        assert_perft(fen, 3, 11714, 1680, 880, 0)?;
        assert_perft(fen, 4, 376467, 12434, 14459, 2)?;
        assert_perft(fen, 5, 8148177, 988563, 537908, 362)?;
        Ok(())
    }
}
