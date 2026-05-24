use anyhow::Result;

use crate::{
    movegen::generate_moves,
    position::Position,
    types::MoveGenType,
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

    fn perft_helper<const ROOT: bool>(&mut self, pos: &mut Position, depth: u32) {
        let mut cnt = 0;

        let moves = generate_moves(pos, MoveGenType::Legal);
        let start_timepoint = if ROOT {
            Some(std::time::Instant::now())
        } else {
            None
        };

        let leaf = depth == 2;

        for m in moves {
            if ROOT && depth <= 1 {
                cnt = 1;
                self.nodes += 1;
            } else {
                pos.do_move(m);

                if leaf {
                    let next_moves = generate_moves(pos, MoveGenType::Legal);

                    cnt = next_moves.len() as u64;

                    if cnt > 0 {
                        for nm in next_moves {
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
                } else {
                    self.perft_helper::<false>(pos, depth - 1);
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
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const STARTPOS_FEN: &str =
        "rnbakabnr/9/1c5c1/p1p1p1p1p/9/9/P1P1P1P1P/1C5C1/9/RNBAKABNR w - 0 1";

    #[test]
    fn test_perft_startpos_depth_1() -> Result<()> {
        let mut perft = Perft::new();
        perft.perft(STARTPOS_FEN, 1)?;
        assert_eq!(perft.nodes, 44);
        Ok(())
    }

    #[test]
    fn test_perft_startpos_depth_2() -> Result<()> {
        let mut perft = Perft::new();
        perft.perft(STARTPOS_FEN, 2)?;
        assert_eq!(perft.nodes, 1920);
        assert_eq!(perft.captures, 72);
        assert_eq!(perft.checks, 6);
        assert_eq!(perft.mates, 0);
        Ok(())
    }

    #[test]
    fn test_perft_startpos_depth_3() -> Result<()> {
        let mut perft = Perft::new();
        perft.perft(STARTPOS_FEN, 3)?;
        assert_eq!(perft.nodes, 79666);
        assert_eq!(perft.captures, 3159);
        assert_eq!(perft.checks, 384);
        assert_eq!(perft.mates, 0);
        Ok(())
    }

    #[test]
    fn test_perft_startpos_depth_4() -> Result<()> {
        let mut perft = Perft::new();
        perft.perft(STARTPOS_FEN, 4)?;
        assert_eq!(perft.nodes, 3290240);
        assert_eq!(perft.captures, 115365);
        assert_eq!(perft.checks, 19380);
        assert_eq!(perft.mates, 0);
        Ok(())
    }

    #[test]
    fn test_perft_startpos_depth_5() -> Result<()> {
        let mut perft = Perft::new();
        perft.perft(STARTPOS_FEN, 5)?;
        assert_eq!(perft.nodes, 133312995);
        assert_eq!(perft.captures, 4917734);
        assert_eq!(perft.checks, 953251);
        assert_eq!(perft.mates, 0);
        Ok(())
    }
}
