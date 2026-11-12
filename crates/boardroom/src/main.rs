//! The `boardroom` binary. Every command is a function in a library crate; see `cli.rs`.

fn main() {
    std::process::exit(boardroom::run(std::env::args().skip(1).collect()));
}
