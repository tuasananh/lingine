use clap::Parser;
use lingine::core::{Move, Position, Value};
use lingine::search::{SearchContext, SearchExtension, SearchWindow, TranspositionTable, negamax};
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
    let mut nodes = 0u64;

    println!("Board FEN:    {}", args.fen);
    println!("Search Depth: {}", args.depth);
    println!("Table Size:   {} MB", args.hash);
    println!("Searching...\n");

    let start = Instant::now();
    let mut killers = [[Move::null(); 2]; 128];
    let mut history_table = [[[0; 90]; 90]; 2];
    let mut ctx = SearchContext {
        stop: &stop,
        nodes: &mut nodes,
        start_time: start,
        time_limit: None,
        transposition_table: &mut transposition_table,
        age: 1,
        killers: &mut killers,
        history_table: &mut history_table,
    };

    let score = negamax(
        &mut pos,
        args.depth as u8,
        1,
        SearchWindow::new(-Value::INFINITY, Value::INFINITY),
        SearchExtension::default(),
        &mut ctx,
    );
    let duration = start.elapsed();

    let nps = if duration.as_secs_f64() > 0.0001 {
        (nodes as f64 / duration.as_secs_f64()) as u64
    } else {
        0
    };

    println!("========================================");
    println!("   SEARCH RESULTS                       ");
    println!("========================================");
    println!("Score:     {} cp", score);
    println!("Nodes:     {}", nodes);
    println!("Time:      {:.3} s", duration.as_secs_f64());
    println!("NPS:       {} nodes/sec", nps);
    println!("========================================");

    Ok(())
}
