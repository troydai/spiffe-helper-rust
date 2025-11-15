use clap::Parser;
use anyhow::Result;

/// A CLI application
#[derive(Parser, Debug)]
#[command(name = "spiffe-helper-rust")]
#[command(about = "A CLI application", long_about = None)]
struct Args {
    /// Optional name to greet
    #[arg(short, long)]
    name: Option<String>,

    /// Number of times to greet
    #[arg(short, long, default_value_t = 1)]
    count: u8,
}

fn main() -> Result<()> {
    let args = Args::parse();

    let name = args.name.as_deref().unwrap_or("World");
    
    for _ in 0..args.count {
        println!("Hello, {}!", name);
    }

    Ok(())
}

