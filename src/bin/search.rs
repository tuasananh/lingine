use clap::Parser;
use lingine::core::Position;
use lingine::search::{HistoryMoves, Searcher, SharedContext, TranspositionTable};
use lingine::uci::RunningStatus;
use std::sync::Arc;
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
    let stop = Arc::new(RunningStatus::default());

    println!("Board FEN:    {}", args.fen);
    println!("Search Depth: {}", args.depth);
    println!("Table Size:   {} MB", args.hash);
    println!("Searching...\n");

    let mut history_moves = HistoryMoves::default();

    let start = Instant::now();
    let time_manager = lingine::search::TimeManager::new(
        &lingine::uci::GoParameters {
            depth: Some(args.depth as u32),
            ..Default::default()
        },
        pos.side_to_move(),
    );
    let best_move = Searcher::start_search(
        pos,
        time_manager,
        SharedContext {
            keep_running: stop.clone(),
            transposition_table: &mut transposition_table,
            history_moves: &mut history_moves,
        },
    );
    let duration = start.elapsed();

    // let nps = if duration.as_secs_f64() > 0.0001 {
    //     (nodes as f64 / duration.as_secs_f64()) as u64
    // } else {
    //     0
    // };

    println!("========================================");
    println!("   SEARCH RESULTS                       ");
    println!("========================================");
    println!("Best Move: {}", best_move.to_uci_string());
    // println!("Score:     {} cp", score);
    // println!("Nodes:     {}", nodes);
    println!("Time:      {:.3} s", duration.as_secs_f64());
    // println!("NPS:       {} nodes/sec", nps);
    println!("========================================");

    Ok(())
}
