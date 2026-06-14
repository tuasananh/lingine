use clap::Parser;
use lingine::core::{
    PackedScore, Piece, PieceType, Position, Side, Square, cannon_captures, knight_attacks,
    rook_attacks,
};
use lingine::eval::{EvalParams, evaluate_with_params};
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::PathBuf;

#[derive(Parser, Debug)]
#[command(
    author,
    version,
    about = "Texel tuning tool for Lingine using Adam Optimizer"
)]
struct Args {
    #[arg(short, long, default_value = "tools/texel_data_large.epd")]
    file: PathBuf,

    #[arg(short, long, default_value_t = 0)]
    limit: usize,

    #[arg(short, long, default_value_t = 200)]
    iterations: usize,

    #[arg(short, long, default_value_t = 1e-7)]
    convergence: f64,

    #[arg(short = 'r', long, default_value_t = 1.0)]
    learning_rate: f64,
}

struct Entry {
    pos: Position,
    result: f64, // 1.0 (Win), 0.5 (Draw), 0.0 (Loss)
}

struct SparsePosition {
    features: Vec<(usize, f64)>, // (parameter_index, coefficient)
    phase: f64,
    result: f64,
}

const PARAM_PAWN: usize = 3;
const PARAM_PAWN_CROSSED: usize = 7;

const PST_OFFSET: usize = 8;
const MOBILITY_OFFSET: usize = 358;
const DEFENDER_OFFSET: usize = 403;

fn add_coeff(features: &mut Vec<(usize, f64)>, idx: usize, val: f64) {
    if let Some(pos) = features.iter_mut().position(|(i, _)| *i == idx) {
        features[pos].1 += val;
    } else {
        features.push((idx, val));
    }
}

fn add_mobility_features(pos: &Position, features: &mut Vec<(usize, f64)>) {
    let occupied = pos.bitboard_occupied();

    for side in [Side::Red, Side::Black] {
        let us_sign = side.signum() as f64;
        let friendly = pos.bitboard_by_color(side);
        let enemy = pos.bitboard_by_color(side.opposite());

        // Knights
        let mut knights = pos.bitboard_by_type(PieceType::Knight) & friendly;
        while let Some(from) = knights.pop_lsb() {
            let attacks = knight_attacks(from, occupied) & !friendly;
            let count = attacks.count_ones() as usize;
            add_coeff(features, MOBILITY_OFFSET + count, us_sign);
        }

        // Rooks
        let mut rooks = pos.bitboard_by_type(PieceType::Rook) & friendly;
        while let Some(from) = rooks.pop_lsb() {
            let attacks = rook_attacks(from, occupied) & !friendly;
            let count = attacks.count_ones() as usize;
            add_coeff(features, MOBILITY_OFFSET + 9 + count, us_sign);
        }

        // Cannons
        let mut cannons = pos.bitboard_by_type(PieceType::Cannon) & friendly;
        while let Some(from) = cannons.pop_lsb() {
            let attacks = (rook_attacks(from, occupied) & !occupied)
                | (cannon_captures(from, occupied) & enemy);
            let count = attacks.count_ones() as usize;
            add_coeff(features, MOBILITY_OFFSET + 9 + 18 + count, us_sign);
        }
    }
}

fn add_defender_features(pos: &Position, features: &mut Vec<(usize, f64)>) {
    // Red defenders
    let red_advisors = pos.piece_count(Piece::RedAdvisor) as usize;
    let red_bishops = pos.piece_count(Piece::RedBishop) as usize;
    add_coeff(features, DEFENDER_OFFSET + red_advisors.min(2), 1.0);
    add_coeff(features, DEFENDER_OFFSET + 3 + red_bishops.min(2), 1.0);

    // Black defenders
    let black_advisors = pos.piece_count(Piece::BlackAdvisor) as usize;
    let black_bishops = pos.piece_count(Piece::BlackBishop) as usize;
    add_coeff(features, DEFENDER_OFFSET + black_advisors.min(2), -1.0);
    add_coeff(features, DEFENDER_OFFSET + 3 + black_bishops.min(2), -1.0);
}

fn calculate_phase(pos: &Position) -> f64 {
    pos.calculate_board_phase() as f64
}

fn compile_features(pos: &Position, result: f64) -> SparsePosition {
    let mut features = Vec::new();

    for sq in Square::all() {
        if let Some(piece) = pos.piece_at(sq) {
            let pt = piece.piece_type();
            let pc = piece.color();
            let us_sign = pc.signum() as f64;

            // 1. Material
            if pt == PieceType::Pawn {
                let crossed = match pc {
                    Side::Red => sq.rank() as u8 >= 5,
                    Side::Black => sq.rank() as u8 <= 4,
                };
                if crossed {
                    add_coeff(&mut features, PARAM_PAWN_CROSSED, us_sign);
                } else {
                    add_coeff(&mut features, PARAM_PAWN, us_sign);
                }
            } else if pt == PieceType::King {
                // King material is fixed to 0
            } else {
                add_coeff(&mut features, pt as usize, us_sign);
            }

            // 2. Piece-Square Tables (PST)
            let sq_mirrored = match pc {
                Side::Red => sq,
                Side::Black => {
                    let file = sq.file() as usize;
                    let rank = sq.rank() as usize;
                    let mirrored_rank = 9 - rank;
                    let mirrored_file = 8 - file;
                    Square::from_repr((mirrored_rank * 9 + mirrored_file) as u8).unwrap()
                }
            };
            let rank = sq_mirrored.rank() as usize;
            let file = sq_mirrored.file() as usize;
            let file_indep = if file <= 4 { file } else { 8 - file };
            let pst_idx = rank * 5 + file_indep;

            add_coeff(
                &mut features,
                PST_OFFSET + (pt as usize) * 50 + pst_idx,
                us_sign,
            );
        }
    }

    add_mobility_features(pos, &mut features);
    add_defender_features(pos, &mut features);

    let phase = calculate_phase(pos);

    // Score is from side-to-move's perspective
    let side_sign = pos.side_to_move().signum() as f64;
    for f in &mut features {
        f.1 *= side_sign;
    }

    SparsePosition {
        features,
        phase,
        result,
    }
}

fn calculate_score(features: &SparsePosition, weights: &[f64]) -> f64 {
    let mut mg_sum = 0.0;
    let mut eg_sum = 0.0;
    for &(idx, coeff) in &features.features {
        mg_sum += weights[idx * 2] * coeff;
        eg_sum += weights[idx * 2 + 1] * coeff;
    }
    let phase = features.phase;
    (mg_sum * phase + eg_sum * (32.0 - phase)) / 32.0
}

fn calculate_mse_features(entries: &[SparsePosition], weights: &[f64], k: f64) -> f64 {
    let num_threads = std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(4);
    let chunk_size = entries.len().div_ceil(num_threads);

    let total_sq_error: f64 = std::thread::scope(|s| {
        let mut handles = Vec::new();
        for chunk in entries.chunks(chunk_size) {
            let handle = s.spawn(move || {
                let mut local_error = 0.0;
                for entry in chunk {
                    let score = calculate_score(entry, weights);
                    let sigmoid = 1.0 / (1.0 + 10.0f64.powf(-k * score / 400.0));
                    let err = entry.result - sigmoid;
                    local_error += err * err;
                }
                local_error
            });
            handles.push(handle);
        }
        handles.into_iter().map(|h| h.join().unwrap()).sum()
    });

    total_sq_error / entries.len() as f64
}

fn optimize_k_features(entries: &[SparsePosition], weights: &[f64]) -> f64 {
    let mut low = 0.0;
    let mut high = 10.0;

    for _ in 0..100 {
        let m1 = low + (high - low) / 3.0;
        let m2 = high - (high - low) / 3.0;

        let err1 = calculate_mse_features(entries, weights, m1);
        let err2 = calculate_mse_features(entries, weights, m2);

        if err1 < err2 {
            high = m2;
        } else {
            low = m1;
        }
    }
    (low + high) / 2.0
}

fn compute_gradients(entries: &[SparsePosition], weights: &[f64], k: f64) -> Vec<f64> {
    let num_threads = std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(4);
    let chunk_size = entries.len().div_ceil(num_threads);

    std::thread::scope(|s| {
        let mut handles = Vec::new();
        for chunk in entries.chunks(chunk_size) {
            let handle = s.spawn(move || {
                let mut local_gradients = vec![0.0; weights.len()];
                for entry in chunk {
                    let score = calculate_score(entry, weights);
                    let sigmoid = 1.0 / (1.0 + 10.0f64.powf(-k * score / 400.0));
                    let diff = sigmoid - entry.result;

                    let deriv = sigmoid * (1.0 - sigmoid) * std::f64::consts::LN_10 * (k / 400.0);
                    let scale = 2.0 * diff * deriv;

                    let phase = entry.phase;
                    for &(idx, coeff) in &entry.features {
                        local_gradients[idx * 2] += scale * coeff * (phase / 32.0);
                        local_gradients[idx * 2 + 1] += scale * coeff * ((32.0 - phase) / 32.0);
                    }
                }
                local_gradients
            });
            handles.push(handle);
        }

        let mut total_gradients = vec![0.0; weights.len()];
        for h in handles {
            let local_grads = h.join().unwrap();
            for (i, val) in local_grads.into_iter().enumerate() {
                total_gradients[i] += val;
            }
        }

        let n = entries.len() as f64;
        for val in &mut total_gradients {
            *val /= n;
        }

        total_gradients
    })
}

fn calculate_score_exact(features: &SparsePosition, weights: &[f64]) -> f64 {
    let mut mg_sum = 0.0;
    let mut eg_sum = 0.0;
    for &(idx, coeff) in &features.features {
        mg_sum += weights[idx * 2] * coeff;
        eg_sum += weights[idx * 2 + 1] * coeff;
    }
    let mg = mg_sum.round() as i32;
    let eg = eg_sum.round() as i32;
    let phase = features.phase.round() as i32;
    let sum = mg * phase + eg * (32 - phase);
    let val = if sum >= 0 {
        (sum + 16) / 32
    } else {
        (sum - 16) / 32
    };
    val as f64
}

fn main() {
    let args = Args::parse();

    let running = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(true));
    let r = running.clone();
    ctrlc::set_handler(move || {
        println!("\nCtrl+C detected! Gracefully shutting down and printing best parameters...");
        r.store(false, std::sync::atomic::Ordering::SeqCst);
    })
    .expect("Error setting Ctrl-C handler");

    println!("Loading EPD file from: {:?}", args.file);
    let file = match File::open(&args.file) {
        Ok(f) => f,
        Err(e) => {
            eprintln!("ERROR: Failed to open EPD file {:?}: {:?}", args.file, e);
            std::process::exit(1);
        }
    };
    let reader = BufReader::new(file);

    let mut raw_entries = Vec::new();
    for (line_idx, line) in reader.lines().enumerate() {
        if args.limit > 0 && raw_entries.len() >= args.limit {
            break;
        }

        let line = match line {
            Ok(l) => l,
            Err(e) => {
                eprintln!("ERROR: Failed to read line {}: {:?}", line_idx + 1, e);
                break;
            }
        };
        if line.trim().is_empty() {
            continue;
        }

        let parts: Vec<&str> = line.split(" c9 ").collect();
        if parts.len() < 2 {
            continue;
        }

        let fen = parts[0].trim();
        let result_part = parts[1].trim();

        let result_str = result_part
            .trim_start_matches('"')
            .trim_end_matches(';')
            .trim_end_matches('"');
        let result = match result_str.parse::<f64>() {
            Ok(r) => r,
            Err(_) => {
                eprintln!(
                    "Warning: Failed to parse result from: {} at line {}, skipping",
                    result_str,
                    line_idx + 1
                );
                continue;
            }
        };

        match Position::from_fen(fen) {
            Ok(pos) => raw_entries.push(Entry { pos, result }),
            Err(e) => {
                eprintln!(
                    "Warning: invalid FEN at line {}: {} ({:?})",
                    line_idx + 1,
                    fen,
                    e
                );
            }
        }
    }

    println!("Loaded {} positions.", raw_entries.len());
    if raw_entries.is_empty() {
        eprintln!("ERROR: No valid positions loaded. Exiting.");
        std::process::exit(1);
    }

    // Initialize parameters
    let params = EvalParams::default();
    let params_vec_i32 = params.to_vector();
    let mut weights: Vec<f64> = params_vec_i32.iter().map(|&x| x as f64).collect();

    // Verify feature extraction against the engine's evaluate_with_params
    println!("Verifying feature extraction consistency...");
    let mut verified = true;
    for (idx, entry) in raw_entries.iter().take(100).enumerate() {
        let eval = evaluate_with_params(&entry.pos, &params);
        let side_to_move = entry.pos.side_to_move();
        let expected_score = match side_to_move {
            Side::Red => eval,
            Side::Black => -eval,
        } as f64;

        let sparse = compile_features(&entry.pos, entry.result);
        let computed_score = calculate_score_exact(&sparse, &weights);
        let diff = (expected_score - computed_score).abs();
        if diff > 1e-2 {
            println!(
                "WARNING: Verification failed at index {}: expected {}, computed {}",
                idx, expected_score, computed_score
            );
            verified = false;
        }
    }
    if verified {
        println!("Verification successful! Feature extraction is 100% consistent with evaluation.");
    } else {
        println!("ERROR: Feature extraction is inconsistent. Please check the mapping.");
        std::process::exit(1);
    }

    println!("Compiling sparse features for all positions...");
    let entries: Vec<SparsePosition> = raw_entries
        .into_iter()
        .map(|entry| compile_features(&entry.pos, entry.result))
        .collect();

    let mut best_k = optimize_k_features(&entries, &weights);
    let mut best_mse = calculate_mse_features(&entries, &weights, best_k);

    println!("Initial K: {:.6}", best_k);
    println!("Initial MSE: {:.10}", best_mse);

    // Adam configuration
    let alpha = args.learning_rate;
    let beta1 = 0.9;
    let beta2 = 0.999;
    let epsilon = 1e-8;

    let mut m = vec![0.0; weights.len()];
    let mut v = vec![0.0; weights.len()];
    let mut beta1_pow = beta1;
    let mut beta2_pow = beta2;

    println!("Tuning {} active parameters using Adam...", weights.len());

    for iter in 1..=args.iterations {
        if !running.load(std::sync::atomic::Ordering::SeqCst) {
            break;
        }

        let iter_start = std::time::Instant::now();
        let prev_mse = best_mse;

        // Compute gradients
        let gradients = compute_gradients(&entries, &weights, best_k);

        // Update parameters
        for i in 0..weights.len() {
            // King material (indices 12 & 13) is fixed at 0
            if i == 12 || i == 13 {
                continue;
            }

            m[i] = beta1 * m[i] + (1.0 - beta1) * gradients[i];
            v[i] = beta2 * v[i] + (1.0 - beta2) * gradients[i] * gradients[i];

            let m_hat = m[i] / (1.0 - beta1_pow);
            let v_hat = v[i] / (1.0 - beta2_pow);

            weights[i] -= alpha * m_hat / (v_hat.sqrt() + epsilon);
        }

        beta1_pow *= beta1;
        beta2_pow *= beta2;

        best_k = optimize_k_features(&entries, &weights);
        best_mse = calculate_mse_features(&entries, &weights, best_k);
        let improvement = prev_mse - best_mse;

        let elapsed = iter_start.elapsed().as_secs_f64();
        println!(
            "Iteration {:>3}/{:<3} | MSE: {:.10} | K: {:.6} | Improvement: {:.10} | Time: {:.2}s",
            iter, args.iterations, best_mse, best_k, improvement, elapsed
        );

        // Auto-save progress
        let mut final_params = EvalParams::default();
        let mut final_params_vec = vec![0; weights.len()];
        for (i, &w) in weights.iter().enumerate() {
            final_params_vec[i] = w.round() as i32;
        }
        final_params.update_from_vector(&final_params_vec);
        let output_str = format_optimized_parameters(&final_params);
        if let Err(e) = std::fs::write("tuner_results.txt", &output_str) {
            eprintln!("WARNING: Failed to auto-save tuner results: {:?}", e);
        }

        if improvement.abs() < args.convergence {
            println!(
                "Convergence reached (improvement {:.10} < {:.10}). Stopping.",
                improvement, args.convergence
            );
            break;
        }
    }

    println!("\nTuning completed!");
    println!("Optimized K: {:.6}", best_k);
    println!("Final MSE: {:.10}", best_mse);

    // Sync params with the best vector before printing
    let mut final_params = EvalParams::default();
    let mut final_params_vec = vec![0; weights.len()];
    for (i, &w) in weights.iter().enumerate() {
        final_params_vec[i] = w.round() as i32;
    }
    final_params.update_from_vector(&final_params_vec);
    let output_str = format_optimized_parameters(&final_params);

    // Print to stdout
    println!("{}", output_str);

    // Save to tuner_results.txt
    match std::fs::write("tuner_results.txt", &output_str) {
        Ok(_) => println!("Successfully saved copy-pasteable results to tuner_results.txt"),
        Err(e) => eprintln!("WARNING: Failed to save tuner results to file: {:?}", e),
    }
}

fn format_optimized_parameters(params: &EvalParams) -> String {
    let mut s = String::new();
    use std::fmt::Write;

    writeln!(
        s,
        "\n====================================================================="
    )
    .unwrap();
    writeln!(s, "  COPY-PASTEABLE PARAMETERS FOR src/eval/ FEATURE FILES").unwrap();
    writeln!(
        s,
        "====================================================================="
    )
    .unwrap();

    writeln!(s, "\n// --- Paste this into src/eval/defender_bonus.rs ---").unwrap();
    writeln!(s, "// Tapered bonuses for having 0, 1, or 2 Advisors").unwrap();
    writeln!(
        s,
        "pub const ADVISOR_COUNT_BONUS: [PackedScore; 3] = packed![({}, {}), ({}, {}), ({}, {})];",
        params.advisor_count_bonus[0].mg,
        params.advisor_count_bonus[0].eg,
        params.advisor_count_bonus[1].mg,
        params.advisor_count_bonus[1].eg,
        params.advisor_count_bonus[2].mg,
        params.advisor_count_bonus[2].eg,
    )
    .unwrap();

    writeln!(
        s,
        "\n// Tapered bonuses for having 0, 1, or 2 Bishops (Elephants)"
    )
    .unwrap();
    writeln!(
        s,
        "pub const BISHOP_COUNT_BONUS: [PackedScore; 3] = packed![({}, {}), ({}, {}), ({}, {})];",
        params.bishop_count_bonus[0].mg,
        params.bishop_count_bonus[0].eg,
        params.bishop_count_bonus[1].mg,
        params.bishop_count_bonus[1].eg,
        params.bishop_count_bonus[2].mg,
        params.bishop_count_bonus[2].eg,
    )
    .unwrap();

    writeln!(
        s,
        "\n// --- Paste this into src/eval/piece_material_value.rs ---"
    )
    .unwrap();
    writeln!(s, "pub(in crate::eval) struct PieceMaterialValue;\n").unwrap();
    writeln!(s, "impl PieceMaterialValue {{").unwrap();
    writeln!(
        s,
        "    pub const ROOK: PackedScore = packed!({}, {});",
        params.material[0].mg, params.material[0].eg
    )
    .unwrap();
    writeln!(
        s,
        "    pub const ADVISOR: PackedScore = packed!({}, {});",
        params.material[1].mg, params.material[1].eg
    )
    .unwrap();
    writeln!(
        s,
        "    pub const CANNON: PackedScore = packed!({}, {});",
        params.material[2].mg, params.material[2].eg
    )
    .unwrap();
    writeln!(
        s,
        "    pub const PAWN: PackedScore = packed!({}, {});",
        params.material[3].mg, params.material[3].eg
    )
    .unwrap();
    writeln!(
        s,
        "    pub const KNIGHT: PackedScore = packed!({}, {});",
        params.material[4].mg, params.material[4].eg
    )
    .unwrap();
    writeln!(
        s,
        "    pub const BISHOP: PackedScore = packed!({}, {});",
        params.material[5].mg, params.material[5].eg
    )
    .unwrap();
    writeln!(
        s,
        "    pub const PAWN_CROSSED: PackedScore = packed!({}, {});",
        params.pawn_crossed.mg, params.pawn_crossed.eg
    )
    .unwrap();
    writeln!(s, "}}").unwrap();

    writeln!(
        s,
        "\n// --- Paste this into src/eval/mobility_tables.rs ---"
    )
    .unwrap();
    writeln!(
        s,
        "pub(in crate::eval) const KNIGHT_MOBILITY_BONUS: [PackedScore; 9] = packed!["
    )
    .unwrap();
    for (i, m) in params.knight_mobility.iter().enumerate() {
        if i < 8 {
            writeln!(s, "    ({}, {}),", m.mg, m.eg).unwrap();
        } else {
            writeln!(s, "    ({}, {})", m.mg, m.eg).unwrap();
        }
    }
    writeln!(s, "];").unwrap();

    writeln!(
        s,
        "\npub(in crate::eval) const ROOK_MOBILITY_BONUS: [PackedScore; 18] = packed!["
    )
    .unwrap();
    for (i, m) in params.rook_mobility.iter().enumerate() {
        if i < 17 {
            writeln!(s, "    ({}, {}),", m.mg, m.eg).unwrap();
        } else {
            writeln!(s, "    ({}, {})", m.mg, m.eg).unwrap();
        }
    }
    writeln!(s, "];").unwrap();

    writeln!(
        s,
        "\npub(in crate::eval) const CANNON_MOBILITY_BONUS: [PackedScore; 18] = packed!["
    )
    .unwrap();
    for (i, m) in params.cannon_mobility.iter().enumerate() {
        if i < 17 {
            writeln!(s, "    ({}, {}),", m.mg, m.eg).unwrap();
        } else {
            writeln!(s, "    ({}, {})", m.mg, m.eg).unwrap();
        }
    }
    writeln!(s, "];").unwrap();

    writeln!(
        s,
        "\n// --- Paste this into src/eval/piece_square_tables.rs ---"
    )
    .unwrap();
    let pst_order = [
        (6, "PIECE_SQUARE_TABLE_KING_TAPERED"),
        (1, "PIECE_SQUARE_TABLE_ADVISOR_TAPERED"),
        (5, "PIECE_SQUARE_TABLE_BISHOP_TAPERED"),
        (4, "PIECE_SQUARE_TABLE_KNIGHT_TAPERED"),
        (0, "PIECE_SQUARE_TABLE_ROOK_TAPERED"),
        (2, "PIECE_SQUARE_TABLE_CANNON_TAPERED"),
        (3, "PIECE_SQUARE_TABLE_PAWN_TAPERED"),
    ];
    for &(idx, name) in pst_order.iter() {
        format_pst(&mut s, name, &params.psts[idx]);
    }

    s
}

fn format_pst(s: &mut String, name: &str, table: &[PackedScore; 90]) {
    use std::fmt::Write;
    writeln!(s, "\n#[rustfmt::skip]").unwrap();
    writeln!(
        s,
        "pub(in crate::eval) const {}: [PackedScore; 90] = packed![",
        name
    )
    .unwrap();

    let mut max_w = 0;
    for &m in table {
        let len = format!("({}, {}),", m.mg, m.eg).len();
        if len > max_w {
            max_w = len;
        }
    }

    for rank in 0..10 {
        writeln!(s, "    // Rank {}", rank).unwrap();
        write!(s, "    ").unwrap();
        for file in 0..9 {
            let idx = rank * 9 + file;
            let m = table[idx];
            let cell = format!("({}, {}),", m.mg, m.eg);
            write!(s, "{:width$}", cell, width = max_w + 1).unwrap();
        }
        writeln!(s).unwrap();
    }
    writeln!(s, "];").unwrap();
}
