//! The old router's own binary: [`ultraroute::cli::run`] on the words given.

fn main() {
    ultraroute::cli::run(std::env::args().skip(1).collect());
}
