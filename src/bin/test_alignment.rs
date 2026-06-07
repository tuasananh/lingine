fn main() {
    println!(
        "Sizeof Option<Square>: {}",
        std::mem::size_of::<Option<lingine::core::Square>>()
    );
    println!(
        "Sizeof Position::board: {}",
        std::mem::size_of::<lingine::core::Position>()
    );
}
