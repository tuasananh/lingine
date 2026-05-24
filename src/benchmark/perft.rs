use anyhow::Result;

use crate::{
    movegen::generate_moves,
    position::Position,
    types::{MAX_MOVES, Move, MoveGenType},
};

#[derive(Debug)]
pub struct Perft {
    nodes: u64,
    checks: u64,
    captures: u64,
    mates: u64,
}

impl Default for Perft {
    fn default() -> Self {
        Self::new()
    }
}

impl Perft {
    pub fn new() -> Self {
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

        println!("Perft results for depth {depth}:\n{self:?}");
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

        for m in moves.iter().copied().take(moves_count) {
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
                        for nm in next_moves.iter().copied().take(next_count) {
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
                println!("Move {}: {}", m, cnt);
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

    #[test]
    #[ignore]
    fn test_perft_position_1_depth_6() -> Result<()> {
        let fen = "rheakaehr/9/1c5c1/p1p1p1p1p/9/9/P1P1P1P1P/1C5C1/9/RHEAKAEHR w - - 0 1";
        assert_perft(fen, 6, 5_392_831_844, 39_288_662, 185_194_510, 584)?;
        Ok(())
    }

    #[test]
    #[ignore]
    fn test_perft_position_1_depth_7() -> Result<()> {
        let fen = "rheakaehr/9/1c5c1/p1p1p1p1p/9/9/P1P1P1P1P/1C5C1/9/RHEAKAEHR w - - 0 1";
        assert_perft(
            fen,
            7,
            217_154_523_878,
            1_793_957_429,
            7_806_689_172,
            29_150,
        )?;
        Ok(())
    }

    #[test]
    #[ignore]
    fn test_perft_position_1_depth_8() -> Result<()> {
        let fen = "rheakaehr/9/1c5c1/p1p1p1p1p/9/9/P1P1P1P1P/1C5C1/9/RHEAKAEHR w - - 0 1";
        assert_perft(
            fen,
            8,
            8_737_663_095_907,
            71_125_910_451,
            303_251_726_428,
            4_083_622,
        )?;
        Ok(())
    }

    #[test]
    #[ignore]
    fn test_perft_position_1_depth_9() -> Result<()> {
        let fen = "rheakaehr/9/1c5c1/p1p1p1p1p/9/9/P1P1P1P1P/1C5C1/9/RHEAKAEHR w - - 0 1";
        assert_perft(
            fen,
            9,
            352_226_653_990_275,
            3_155_476_718_855,
            12_781_706_774_202,
            182_581_992,
        )?;
        Ok(())
    }

    #[test]
    #[ignore]
    fn test_perft_position_1_depth_10() -> Result<()> {
        let fen = "rheakaehr/9/1c5c1/p1p1p1p1p/9/9/P1P1P1P1P/1C5C1/9/RHEAKAEHR w - - 0 1";
        assert_perft(
            fen,
            10,
            14_187_549_274_751_861,
            125_057_982_988_020,
            509_498_168_281_284,
            13_831_340_296,
        )?;
        Ok(())
    }

    #[test]
    #[ignore]
    fn test_perft_position_2_depth_6() -> Result<()> {
        let fen = "r1ea1a3/4kh3/2h1e4/pHp1p1p1p/4c4/6P2/P1P2R2P/1CcC5/9/2EAKAE2 w - - 0 1";
        assert_perft(fen, 6, 1_663_640_378, 29_697_750, 55_945_766, 41_673)?;
        Ok(())
    }

    #[test]
    #[ignore]
    fn test_perft_position_2_depth_7() -> Result<()> {
        let fen = "r1ea1a3/4kh3/2h1e4/pHp1p1p1p/4c4/6P2/P1P2R2P/1CcC5/9/2EAKAE2 w - - 0 1";
        assert_perft(
            fen,
            7,
            66_742_966_316,
            1_960_700_294,
            4_740_705_145,
            2_702_397,
        )?;
        Ok(())
    }

    #[test]
    #[ignore]
    fn test_perft_position_2_depth_8() -> Result<()> {
        let fen = "r1ea1a3/4kh3/2h1e4/pHp1p1p1p/4c4/6P2/P1P2R2P/1CcC5/9/2EAKAE2 w - - 0 1";
        assert_perft(
            fen,
            8,
            2_142_014_636_456,
            39_524_028_541,
            88_410_505_039,
            62_159_428,
        )?;
        Ok(())
    }

    #[test]
    #[ignore]
    fn test_perft_position_2_depth_9() -> Result<()> {
        let fen = "r1ea1a3/4kh3/2h1e4/pHp1p1p1p/4c4/6P2/P1P2R2P/1CcC5/9/2EAKAE2 w - - 0 1";
        assert_perft(
            fen,
            9,
            86_413_551_364_229,
            2_638_908_733_541,
            6_769_016_991_326,
            3_645_360_756,
        )?;
        Ok(())
    }

    #[test]
    #[ignore]
    fn test_perft_position_2_depth_10() -> Result<()> {
        let fen = "r1ea1a3/4kh3/2h1e4/pHp1p1p1p/4c4/6P2/P1P2R2P/1CcC5/9/2EAKAE2 w - - 0 1";
        assert_perft(
            fen,
            10,
            2_830_006_985_783_112,
            52_300_489_303_217,
            134_151_779_490_661,
            88_186_803_110,
        )?;
        Ok(())
    }

    #[test]
    #[ignore]
    fn test_perft_position_3_depth_6() -> Result<()> {
        let fen = "1ceak4/9/h2a5/2p1p3p/5cp2/2h2H3/6PCP/3AE4/2C6/3A1K1H1 w - - 0 1";
        assert_perft(fen, 6, 380_156_340, 11_748_261, 14_627_168, 2_302)?;
        Ok(())
    }

    #[test]
    #[ignore]
    fn test_perft_position_3_depth_7() -> Result<()> {
        let fen = "1ceak4/9/h2a5/2p1p3p/5cp2/2h2H3/6PCP/3AE4/2C6/3A1K1H1 w - - 0 1";
        assert_perft(fen, 7, 12_345_505_147, 218_725_663, 806_263_565, 37_989)?;
        Ok(())
    }

    #[test]
    #[ignore]
    fn test_perft_position_3_depth_8() -> Result<()> {
        let fen = "1ceak4/9/h2a5/2p1p3p/5cp2/2h2H3/6PCP/3AE4/2C6/3A1K1H1 w - - 0 1";
        assert_perft(
            fen,
            8,
            445_309_747_371,
            13_751_134_332,
            18_559_293_496,
            4_801_845,
        )?;
        Ok(())
    }

    #[test]
    #[ignore]
    fn test_perft_position_3_depth_9() -> Result<()> {
        let fen = "1ceak4/9/h2a5/2p1p3p/5cp2/2h2H3/6PCP/3AE4/2C6/3A1K1H1 w - - 0 1";
        assert_perft(
            fen,
            9,
            14_614_113_572_802,
            266_274_473_825,
            977_827_445_317,
            86_519_165,
        )?;
        Ok(())
    }

    #[test]
    #[ignore]
    fn test_perft_position_3_depth_10() -> Result<()> {
        let fen = "1ceak4/9/h2a5/2p1p3p/5cp2/2h2H3/6PCP/3AE4/2C6/3A1K1H1 w - - 0 1";
        assert_perft(
            fen,
            10,
            521_995_912_658_933,
            16_061_905_823_721,
            23_454_619_449_916,
            7_310_094_390,
        )?;
        Ok(())
    }

    #[test]
    #[ignore]
    fn test_perft_position_4_depth_6() -> Result<()> {
        let fen = "5a3/3k5/3aR4/9/5r3/5h3/9/3A1A3/5K3/2EC2E2 w - - 0 1";
        assert_perft(fen, 6, 100_055_401, 4_582_930, 5_001_139, 4_345)?;
        Ok(())
    }

    #[test]
    #[ignore]
    fn test_perft_position_4_depth_7() -> Result<()> {
        let fen = "5a3/3k5/3aR4/9/5r3/5h3/9/3A1A3/5K3/2EC2E2 w - - 0 1";
        assert_perft(fen, 7, 2_447_759_037, 166_331_869, 143_760_961, 163_653)?;
        Ok(())
    }

    #[test]
    #[ignore]
    fn test_perft_position_4_depth_8() -> Result<()> {
        let fen = "5a3/3k5/3aR4/9/5r3/5h3/9/3A1A3/5K3/2EC2E2 w - - 0 1";
        assert_perft(
            fen,
            8,
            52_307_997_357,
            2_408_154_816,
            2_668_348_453,
            2_325_098,
        )?;
        Ok(())
    }

    #[test]
    #[ignore]
    fn test_perft_position_4_depth_9() -> Result<()> {
        let fen = "5a3/3k5/3aR4/9/5r3/5h3/9/3A1A3/5K3/2EC2E2 w - - 0 1";
        assert_perft(
            fen,
            9,
            1_343_160_831_754,
            80_132_288_711,
            67_079_153_812,
            97_462_707,
        )?;
        Ok(())
    }

    #[test]
    #[ignore]
    fn test_perft_position_4_depth_10() -> Result<()> {
        let fen = "5a3/3k5/3aR4/9/5r3/5h3/9/3A1A3/5K3/2EC2E2 w - - 0 1";
        assert_perft(
            fen,
            10,
            28_838_171_664_689,
            1_332_698_893_123,
            1_476_550_279_562,
            1_203_847_678,
        )?;
        Ok(())
    }

    #[test]
    #[ignore]
    fn test_perft_position_5_depth_6() -> Result<()> {
        let fen = "CRH1k1e2/3ca4/4ea3/9/2hr5/9/9/4E4/4A4/4KA3 w - - 0 1";
        assert_perft(fen, 6, 367_168_327, 11_779_169, 14_499_871, 116_750)?;
        Ok(())
    }

    #[test]
    #[ignore]
    fn test_perft_position_5_depth_7() -> Result<()> {
        let fen = "CRH1k1e2/3ca4/4ea3/9/2hr5/9/9/4E4/4A4/4KA3 w - - 0 1";
        assert_perft(fen, 7, 11_194_690_506, 825_591_992, 583_666_802, 476_743)?;
        Ok(())
    }

    #[test]
    #[ignore]
    fn test_perft_position_5_depth_8() -> Result<()> {
        let fen = "CRH1k1e2/3ca4/4ea3/9/2hr5/9/9/4E4/4A4/4KA3 w - - 0 1";
        assert_perft(
            fen,
            8,
            366_768_760_741,
            12_248_499_805,
            16_860_361_852,
            85_997_356,
        )?;
        Ok(())
    }

    #[test]
    #[ignore]
    fn test_perft_position_5_depth_9() -> Result<()> {
        let fen = "CRH1k1e2/3ca4/4ea3/9/2hr5/9/9/4E4/4A4/4KA3 w - - 0 1";
        assert_perft(
            fen,
            9,
            11_316_892_742_588,
            678_308_656_892,
            599_101_339_831,
            437_397_863,
        )?;
        Ok(())
    }

    #[test]
    #[ignore]
    fn test_perft_position_5_depth_10() -> Result<()> {
        let fen = "CRH1k1e2/3ca4/4ea3/9/2hr5/9/9/4E4/4A4/4KA3 w - - 0 1";
        assert_perft(
            fen,
            10,
            380_911_906_724_558,
            13_152_214_083_387,
            18_848_190_024_072,
            64_245_967_343,
        )?;
        Ok(())
    }

    #[test]
    #[ignore]
    fn test_perft_position_6_depth_6() -> Result<()> {
        let fen = "R1H1k1e2/9/3aea3/9/2hr5/2E6/9/4E4/4A4/4KA3 w - - 0 1";
        assert_perft(fen, 6, 81_195_154, 2_753_081, 5_470_269, 5_585)?;
        Ok(())
    }

    #[test]
    #[ignore]
    fn test_perft_position_6_depth_7() -> Result<()> {
        let fen = "R1H1k1e2/9/3aea3/9/2hr5/2E6/9/4E4/4A4/4KA3 w - - 0 1";
        assert_perft(fen, 7, 1_765_627_003, 93_795_634, 120_586_840, 14_385)?;
        Ok(())
    }

    #[test]
    #[ignore]
    fn test_perft_position_6_depth_8() -> Result<()> {
        let fen = "R1H1k1e2/9/3aea3/9/2hr5/2E6/9/4E4/4A4/4KA3 w - - 0 1";
        assert_perft(
            fen,
            8,
            42_424_719_146,
            1_426_561_933,
            2_828_476_506,
            1_967_775,
        )?;
        Ok(())
    }

    #[test]
    #[ignore]
    fn test_perft_position_6_depth_9() -> Result<()> {
        let fen = "R1H1k1e2/9/3aea3/9/2hr5/2E6/9/4E4/4A4/4KA3 w - - 0 1";
        assert_perft(
            fen,
            9,
            929_895_135_908,
            43_484_217_259,
            61_439_386_737,
            10_745_724,
        )?;
        Ok(())
    }

    #[test]
    #[ignore]
    fn test_perft_position_6_depth_10() -> Result<()> {
        let fen = "R1H1k1e2/9/3aea3/9/2hr5/2E6/9/4E4/4A4/4KA3 w - - 0 1";
        assert_perft(
            fen,
            10,
            22_656_480_355_626,
            767_893_400_007,
            1_473_581_068_609,
            803_291_479,
        )?;
        Ok(())
    }

    #[test]
    #[ignore]
    fn test_perft_position_7_depth_6() -> Result<()> {
        let fen = "C1hHk4/9/9/9/9/9/h1pp5/E3C4/9/3A1K3 w - - 0 1";
        assert_perft(fen, 6, 23_496_493, 134_225, 1_895_759, 1_089)?;
        Ok(())
    }

    #[test]
    #[ignore]
    fn test_perft_position_7_depth_7() -> Result<()> {
        let fen = "C1hHk4/9/9/9/9/9/h1pp5/E3C4/9/3A1K3 w - - 0 1";
        assert_perft(fen, 7, 713_048_593, 34_857_153, 7_808_522, 0)?;
        Ok(())
    }

    #[test]
    #[ignore]
    fn test_perft_position_7_depth_8() -> Result<()> {
        let fen = "C1hHk4/9/9/9/9/9/h1pp5/E3C4/9/3A1K3 w - - 0 1";
        assert_perft(fen, 8, 9_711_548_664, 75_280_993, 768_711_235, 726_865)?;
        Ok(())
    }

    #[test]
    #[ignore]
    fn test_perft_position_7_depth_9() -> Result<()> {
        let fen = "C1hHk4/9/9/9/9/9/h1pp5/E3C4/9/3A1K3 w - - 0 1";
        assert_perft(
            fen,
            9,
            300_586_365_694,
            12_340_552_491,
            3_663_066_815,
            24_061,
        )?;
        Ok(())
    }

    #[test]
    #[ignore]
    fn test_perft_position_7_depth_10() -> Result<()> {
        let fen = "C1hHk4/9/9/9/9/9/h1pp5/E3C4/9/3A1K3 w - - 0 1";
        assert_perft(
            fen,
            10,
            4_378_452_618_313,
            44_181_673_494,
            331_908_254_483,
            352_702_439,
        )?;
        Ok(())
    }

    #[test]
    #[ignore]
    fn test_perft_position_8_depth_6() -> Result<()> {
        let fen = "4ka3/4a4/9/9/4H4/p8/9/4C3c/7h1/2EK5 w - - 0 1";
        assert_perft(fen, 6, 71_287_903, 1_922_146, 1_212_926, 17_907)?;
        Ok(())
    }

    #[test]
    #[ignore]
    fn test_perft_position_8_depth_7() -> Result<()> {
        let fen = "4ka3/4a4/9/9/4H4/p8/9/4C3c/7h1/2EK5 w - - 0 1";
        assert_perft(fen, 7, 1_657_573_114, 132_843_535, 28_345_589, 153_992)?;
        Ok(())
    }

    #[test]
    #[ignore]
    fn test_perft_position_8_depth_8() -> Result<()> {
        let fen = "4ka3/4a4/9/9/4H4/p8/9/4C3c/7h1/2EK5 w - - 0 1";
        assert_perft(
            fen,
            8,
            35_087_900_502,
            1_045_205_635,
            558_444_756,
            8_611_930,
        )?;
        Ok(())
    }

    #[test]
    #[ignore]
    fn test_perft_position_8_depth_9() -> Result<()> {
        let fen = "4ka3/4a4/9/9/4H4/p8/9/4C3c/7h1/2EK5 w - - 0 1";
        assert_perft(
            fen,
            9,
            805_901_318_220,
            52_908_764_098,
            15_107_162_709,
            71_785_076,
        )?;
        Ok(())
    }

    #[test]
    #[ignore]
    fn test_perft_position_8_depth_10() -> Result<()> {
        let fen = "4ka3/4a4/9/9/4H4/p8/9/4C3c/7h1/2EK5 w - - 0 1";
        assert_perft(
            fen,
            10,
            17_446_699_531_702,
            539_614_961_239,
            273_248_855_042,
            4_209_601_416,
        )?;
        Ok(())
    }

    #[test]
    #[ignore]
    fn test_perft_position_9_depth_6() -> Result<()> {
        let fen = "2e1ka3/9/e3H4/4h4/9/9/9/4C4/2p6/2EK5 w - - 0 1";
        assert_perft(fen, 6, 12_250_386, 1_409_925, 1_290_768, 1_136)?;
        Ok(())
    }

    #[test]
    #[ignore]
    fn test_perft_position_9_depth_7() -> Result<()> {
        let fen = "2e1ka3/9/e3H4/4h4/9/9/9/4C4/2p6/2EK5 w - - 0 1";
        assert_perft(fen, 7, 235_622_620, 14_168_904, 5_848_140, 1_312)?;
        Ok(())
    }

    #[test]
    #[ignore]
    fn test_perft_position_9_depth_8() -> Result<()> {
        let fen = "2e1ka3/9/e3H4/4h4/9/9/9/4C4/2p6/2EK5 w - - 0 1";
        assert_perft(fen, 8, 3_223_442_295, 330_779_266, 288_286_382, 195_357)?;
        Ok(())
    }

    #[test]
    #[ignore]
    fn test_perft_position_9_depth_9() -> Result<()> {
        let fen = "2e1ka3/9/e3H4/4h4/9/9/9/4C4/2p6/2EK5 w - - 0 1";
        assert_perft(
            fen,
            9,
            61_976_856_602,
            3_213_463_588,
            1_424_784_285,
            3_564_547,
        )?;
        Ok(())
    }

    #[test]
    #[ignore]
    fn test_perft_position_9_depth_10() -> Result<()> {
        let fen = "2e1ka3/9/e3H4/4h4/9/9/9/4C4/2p6/2EK5 w - - 0 1";
        assert_perft(
            fen,
            10,
            856_769_945_175,
            79_527_026_317,
            66_165_224_144,
            44_257_146,
        )?;
        Ok(())
    }

    #[test]
    #[ignore]
    fn test_perft_position_10_depth_6() -> Result<()> {
        let fen = "1C2ka3/9/C1Hae1h2/p3p3p/6p2/9/P3P3P/3AE4/3p2c2/c1EAK4 w - - 0 1";
        assert_perft(fen, 6, 517_687_990, 29_340_751, 48_536_896, 43_720)?;
        Ok(())
    }

    #[test]
    #[ignore]
    fn test_perft_position_10_depth_7() -> Result<()> {
        let fen = "1C2ka3/9/C1Hae1h2/p3p3p/6p2/9/P3P3P/3AE4/3p2c2/c1EAK4 w - - 0 1";
        assert_perft(fen, 7, 14_455_679_002, 383_843_427, 1_261_613_671, 58_555)?;
        Ok(())
    }

    #[test]
    #[ignore]
    fn test_perft_position_10_depth_8() -> Result<()> {
        let fen = "1C2ka3/9/C1Hae1h2/p3p3p/6p2/9/P3P3P/3AE4/3p2c2/c1EAK4 w - - 0 1";
        assert_perft(
            fen,
            8,
            421_689_225_752,
            22_355_099_771,
            40_836_967_120,
            28_351_366,
        )?;
        Ok(())
    }

    #[test]
    #[ignore]
    fn test_perft_position_10_depth_9() -> Result<()> {
        let fen = "1C2ka3/9/C1Hae1h2/p3p3p/6p2/9/P3P3P/3AE4/3p2c2/c1EAK4 w - - 0 1";
        assert_perft(
            fen,
            9,
            11_955_334_228_633,
            309_121_766_222,
            1_009_093_177_933,
            58_507_090,
        )?;
        Ok(())
    }

    #[test]
    #[ignore]
    fn test_perft_position_10_depth_10() -> Result<()> {
        let fen = "1C2ka3/9/C1Hae1h2/p3p3p/6p2/9/P3P3P/3AE4/3p2c2/c1EAK4 w - - 0 1";
        assert_perft(
            fen,
            10,
            352_462_159_702_536,
            17_646_344_479_723,
            34_623_825_090_952,
            19_782_310_723,
        )?;
        Ok(())
    }

    #[test]
    #[ignore]
    fn test_perft_position_11_depth_6() -> Result<()> {
        let fen = "ChH1k1e2/c3a4/4ea3/9/2hr5/9/9/4C4/4A4/4KA3 w - - 0 1";
        assert_perft(fen, 6, 270_587_571, 9_963_307, 12_300_582, 1_424)?;
        Ok(())
    }

    #[test]
    #[ignore]
    fn test_perft_position_11_depth_7() -> Result<()> {
        let fen = "ChH1k1e2/c3a4/4ea3/9/2hr5/9/9/4C4/4A4/4KA3 w - - 0 1";
        assert_perft(fen, 7, 6_347_480_650, 622_266_982, 380_559_237, 391_244)?;
        Ok(())
    }

    #[test]
    #[ignore]
    fn test_perft_position_11_depth_8() -> Result<()> {
        let fen = "ChH1k1e2/c3a4/4ea3/9/2hr5/9/9/4C4/4A4/4KA3 w - - 0 1";
        assert_perft(
            fen,
            8,
            218_080_917_174,
            8_558_789_381,
            10_793_188_064,
            1_402_277,
        )?;
        Ok(())
    }

    #[test]
    #[ignore]
    fn test_perft_position_11_depth_9() -> Result<()> {
        let fen = "ChH1k1e2/c3a4/4ea3/9/2hr5/9/9/4C4/4A4/4KA3 w - - 0 1";
        assert_perft(
            fen,
            9,
            5_477_201_520_455,
            435_125_936_179,
            301_126_140_779,
            390_573_055,
        )?;
        Ok(())
    }

    #[test]
    #[ignore]
    fn test_perft_position_11_depth_10() -> Result<()> {
        let fen = "ChH1k1e2/c3a4/4ea3/9/2hr5/9/9/4C4/4A4/4KA3 w - - 0 1";
        assert_perft(
            fen,
            10,
            193_881_133_546_122,
            7_872_885_590_188,
            9_988_415_709_190,
            1_533_023_114,
        )?;
        Ok(())
    }
}
