use clap::Parser;
use lingine::benchmark::Perft;

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    /// Board position in FEN notation.
    #[arg(
        short,
        long,
        default_value = "rheakaehr/9/1c5c1/p1p1p1p1p/9/9/P1P1P1P1P/1C5C1/9/RHEAKAEHR w - - 0 1"
    )]
    fen: String,

    #[arg(short, long, default_value_t = 5)]
    depth: u32,
}

fn main() -> anyhow::Result<()> {
    let args = Args::parse();

    let mut perft = Perft::new();
    perft.perft(&args.fen, args.depth as u64)?;
    Ok(())
}
