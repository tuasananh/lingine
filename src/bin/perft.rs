use clap::Parser;
use lingine::benchmark::Perft;

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    #[arg(short, long)]
    fen: String,
    #[arg(short, long)]
    depth: u32,
}

fn main() -> anyhow::Result<()> {
    let args = Args::parse();

    let mut perft = Perft::new();
    perft.perft(&args.fen, args.depth as u64)?;
    Ok(())
}
