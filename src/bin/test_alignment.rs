use std::num::NonZero;

use lingine::core::{Position, StateInfo};

fn main() {
    println!(
        "Sizeof Option<Square>: {}",
        std::mem::size_of::<Option<lingine::core::Square>>()
    );
    println!(
        "Sizeof Position::board: {}",
        std::mem::size_of::<lingine::core::Position>()
    );
    println!(
        "Sizeof Option<Duration>: {}",
        std::mem::size_of::<Option<std::time::Duration>>()
    );
    println!(
        "Sizeof Duration: {}",
        std::mem::size_of::<std::time::Duration>()
    );
    println!(
        "Sizeof Option<NonZero<u32>>: {}",
        std::mem::size_of::<Option<NonZero<u32>>>()
    );
    println!("Sideof Position: {}", std::mem::size_of::<Position>());
    println!("Sideof StateInfo: {}", std::mem::size_of::<StateInfo>());
}
