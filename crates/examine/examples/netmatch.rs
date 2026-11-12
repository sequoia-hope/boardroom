//! `netmatch NETLIST.net BOARD.kicad_pcb`: whether the netlist is the board's, as JSON
//! (`examine::netmatch`). Exits 0 whether or not it matches; 2 when either file cannot be read.

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let [netlist, board] = args.as_slice() else {
        eprintln!("usage: netmatch NETLIST.net BOARD.kicad_pcb");
        std::process::exit(2);
    };
    let netlist = intent::netlist::Netlist::load(std::path::Path::new(netlist)).unwrap_or_else(|e| {
        eprintln!("netmatch: {e}");
        std::process::exit(2);
    });
    let board = board::pcb::Board::load(std::path::Path::new(board)).unwrap_or_else(|e| {
        eprintln!("netmatch: {e}");
        std::process::exit(2);
    });
    println!("{}", serde_json::to_string(&examine::netmatch::compare(&netlist, &board)).unwrap());
}
