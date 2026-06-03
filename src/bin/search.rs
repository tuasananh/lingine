use clap::Parser;
use lingine::core::Position;
use lingine::search::{TranspositionTable, search};
use lingine::uci::GoParameters;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::time::Instant;

#[derive(Parser, Debug)]
#[command(version, about = "Profile Lingine Negamax search performance", long_about = None)]
struct Args {
    /// Board position in FEN notation.
    #[arg(
        short,
        long,
        default_value = "rheakaehr/9/1c5c1/p1p1p1p1p/9/9/P1P1P1P1P/1C5C1/9/RHEAKAEHR w - - 0 1"
    )]
    fen: String,

    /// Search depth in plies.
    #[arg(short, long, default_value_t = 6)]
    depth: i32,

    /// Transposition table size in MB.
    #[arg(short, long, default_value_t = 16)]
    hash: usize,
}

fn main() -> anyhow::Result<()> {
    let args = Args::parse();

    let mut pos = Position::new();
    pos.set(&args.fen)?;

    let mut transposition_table = TranspositionTable::new(args.hash);
    let stop = Arc::new(AtomicBool::new(false));

    println!("Board FEN:    {}", args.fen);
    println!("Search Depth: {}", args.depth);
    println!("Table Size:   {} MB", args.hash);
    println!("Searching...\n");

    let (tx, _rx) = std::sync::mpsc::channel();
    let params = GoParameters {
        depth: Some(args.depth as u32),
        stop: stop.clone(),
        ..Default::default()
    };

    let start = Instant::now();
    let (best_move, score, nodes) = search(pos, params, &mut transposition_table, 1, tx, None);
    let duration = start.elapsed();

    let nps = if duration.as_secs_f64() > 0.0001 {
        (nodes as f64 / duration.as_secs_f64()) as u64
    } else {
        0
    };

    println!("========================================");
    println!("   SEARCH RESULTS                       ");
    println!("========================================");
    println!("Best Move: {}", best_move.to_uci_string());
    println!("Score:     {} cp", score);
    println!("Nodes:     {}", nodes);
    println!("Time:      {:.3} s", duration.as_secs_f64());
    println!("NPS:       {} nodes/sec", nps);
    println!("========================================");

    Ok(())
}
