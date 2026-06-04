use clap::Parser;
use lingine::core::Position;
use lingine::search::{Search, SearchParameters, TranspositionTable};
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
    let mut history_table = [[[0i32; 90]; 90]; 2];

    let start = Instant::now();
    let (score, best_move, nodes) = Search::start_search(SearchParameters {
        pos,
        stop: stop.clone(),
        max_depth: args.depth as i8,
        allocated_time: None,
        transposition_table: &mut transposition_table,
        history_table: &mut history_table,
        tx,
        age: 0,
    });
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
