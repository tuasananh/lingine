use clap::Parser;
use lingine::core::{Position, Side};
use lingine::eval::{EvalParams, evaluate_with_params};
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::PathBuf;

#[derive(Parser, Debug)]
#[command(author, version, about = "Texel tuning tool for Lingine")]
struct Args {
    #[arg(short, long, default_value = "tools/texel_data.epd")]
    file: PathBuf,

    #[arg(short, long, default_value_t = 0)]
    limit: usize,

    #[arg(short, long, default_value_t = 10)]
    iterations: usize,
}

struct Entry {
    pos: Position,
    result: f64, // 1.0 (Win), 0.5 (Draw), 0.0 (Loss)
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

    let mut entries = Vec::new();
    for (line_idx, line) in reader.lines().enumerate() {
        if args.limit > 0 && entries.len() >= args.limit {
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

        // Parse FEN and result. E.g.
        // rheakaehr/9/1c5c1/p1p1p1p1p/9/9/P1P1P1P1P/1C5C1/9/RHEAKAEHR w c9 "0.5";
        let parts: Vec<&str> = line.split(" c9 ").collect();
        if parts.len() < 2 {
            continue;
        }

        let fen = parts[0].trim();
        let result_part = parts[1].trim();

        // Extract result value inside quotes
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
            Ok(pos) => entries.push(Entry { pos, result }),
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

    println!("Loaded {} positions.", entries.len());
    if entries.is_empty() {
        eprintln!("ERROR: No valid positions loaded. Exiting.");
        std::process::exit(1);
    }

    let num_threads = std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(4);
    println!("Running tuner with {} parallel worker threads.", num_threads);

    let mut params = EvalParams::default();
    let mut best_k = optimize_k(&entries, &params);
    let mut best_mse = calculate_mse(&entries, &params, best_k);

    println!("Initial K: {:.6}", best_k);
    println!("Initial MSE: {:.10}", best_mse);

    let mut params_vec = params.to_vector();
    let param_count = params_vec.len();
    println!("Tuning {} active parameters...", param_count);

    for iter in 1..=args.iterations {
        if !running.load(std::sync::atomic::Ordering::SeqCst) {
            break;
        }
        let mut improved = false;
        println!("--- Iteration {}/{} ---", iter, args.iterations);
        let iter_start = std::time::Instant::now();

        for i in 0..param_count {
            if !running.load(std::sync::atomic::Ordering::SeqCst) {
                break;
            }
            // Try +1
            params_vec[i] += 1;
            params.update_from_vector(&params_vec);
            let score_plus = calculate_mse(&entries, &params, best_k);

            // Try -1
            params_vec[i] -= 2;
            params.update_from_vector(&params_vec);
            let score_minus = calculate_mse(&entries, &params, best_k);

            if score_plus < best_mse && score_plus < score_minus {
                params_vec[i] += 2; // Keep +1
                best_mse = score_plus;
                improved = true;
            } else if score_minus < best_mse {
                // Keep -1 (already set to params_vec[i] - 1)
                best_mse = score_minus;
                improved = true;
            } else {
                params_vec[i] += 1; // Revert to original
            }

            // Print progress and ETA every 100 parameters
            if (i + 1) % 100 == 0 || i + 1 == param_count {
                let elapsed = iter_start.elapsed().as_secs_f64();
                let progress = (i + 1) as f64 / param_count as f64;
                let total_est = elapsed / progress;
                let eta = total_est - elapsed;
                print!(
                    "\r  Progress: {:>4}/{:<4} ({:>5.1}%) | Elapsed: {:>5.1}s | ETA: {:>5.1}s | Current MSE: {:.10}",
                    i + 1,
                    param_count,
                    progress * 100.0,
                    elapsed,
                    eta,
                    best_mse
                );
                use std::io::Write;
                std::io::stdout().flush().unwrap();
            }
        }
        println!(); // Clear the carriage return line

        if !running.load(std::sync::atomic::Ordering::SeqCst) {
            break;
        }

        params.update_from_vector(&params_vec);
        best_k = optimize_k(&entries, &params);
        let current_mse = calculate_mse(&entries, &params, best_k);
        println!(
            "MSE after iteration {}: {:.10} (K: {:.6})",
            iter, current_mse, best_k
        );

        if !improved {
            println!("Convergence reached. Stopping.");
            break;
        }
    }

    println!("\nTuning completed!");
    println!("Optimized K: {:.6}", best_k);
    println!("Final MSE: {:.10}", best_mse);

    // Print copy-pasteable weights
    print_optimized_parameters(&params);
}

fn calculate_mse(entries: &[Entry], params: &EvalParams, k: f64) -> f64 {
    let num_threads = std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(4);

    let chunk_size = (entries.len() + num_threads - 1) / num_threads;

    let total_sq_error: f64 = std::thread::scope(|s| {
        let mut handles = Vec::new();
        for chunk in entries.chunks(chunk_size) {
            let handle = s.spawn(move || {
                let mut local_error = 0.0;
                for entry in chunk {
                    let eval = evaluate_with_params(&entry.pos, params);
                    let side_to_move = entry.pos.side_to_move();
                    let score = match side_to_move {
                        Side::Red => eval,
                        Side::Black => -eval,
                    } as f64;

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

// Find K that minimizes MSE using ternary search
fn optimize_k(entries: &[Entry], params: &EvalParams) -> f64 {
    let mut low = 0.0;
    let mut high = 10.0;

    for _ in 0..100 {
        let m1 = low + (high - low) / 3.0;
        let m2 = high - (high - low) / 3.0;

        let err1 = calculate_mse(entries, params, m1);
        let err2 = calculate_mse(entries, params, m2);

        if err1 < err2 {
            high = m2;
        } else {
            low = m1;
        }
    }
    (low + high) / 2.0
}

fn print_optimized_parameters(params: &EvalParams) {
    println!("\n=====================================================================");
    println!("  COPY-PASTEABLE PARAMETERS FOR src/eval/mod.rs or piece_square_tables.rs");
    println!("=====================================================================");

    println!("\n--- Material Values ---");
    println!("Rook:    {:?}", params.material[0]);
    println!("Advisor: {:?}", params.material[1]);
    println!("Cannon:  {:?}", params.material[2]);
    println!("Pawn (Uncrossed): {:?}", params.material[3]);
    println!("Knight:  {:?}", params.material[4]);
    println!("Bishop:  {:?}", params.material[5]);
    println!("Pawn (Crossed):   {:?}", params.pawn_crossed);

    println!("\n--- Advisor Count Bonus ---");
    println!("advisor_count_bonus = {:?}", params.advisor_count_bonus);

    println!("\n--- Bishop Count Bonus ---");
    println!("bishop_count_bonus = {:?}", params.bishop_count_bonus);

    println!("\n--- Knight Mobility Bonus ---");
    print!("pub(crate) const KNIGHT_MOBILITY_BONUS: [PackedScore; 9] = packed![\n   ");
    for (i, m) in params.knight_mobility.iter().enumerate() {
        print!(" ({}, {})", m.mg, m.eg);
        if i < 8 {
            print!(",");
        }
    }
    println!("\n];");

    println!("\n--- Rook Mobility Bonus ---");
    print!("pub(crate) const ROOK_MOBILITY_BONUS: [PackedScore; 18] = packed![\n   ");
    for (i, m) in params.rook_mobility.iter().enumerate() {
        print!(" ({}, {})", m.mg, m.eg);
        if i < 17 {
            print!(",");
        }
    }
    println!("\n];");

    println!("\n--- Cannon Mobility Bonus ---");
    print!("pub(crate) const CANNON_MOBILITY_BONUS: [PackedScore; 18] = packed![\n   ");
    for (i, m) in params.cannon_mobility.iter().enumerate() {
        print!(" ({}, {})", m.mg, m.eg);
        if i < 17 {
            print!(",");
        }
    }
    println!("\n];");
}
