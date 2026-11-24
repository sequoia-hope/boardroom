//! `short_pads BOARD...`: for each board, how many pads its pours have fewer spokes for than
//! they need (and at least one), each on one layer.
use std::collections::HashMap;
fn main() {
    for path in std::env::args().skip(1) {
        let Ok((b, _)) = board::Board::load(std::path::Path::new(&path)) else {
            println!("{path}\tunread");
            continue;
        };
        let mut of: HashMap<(usize, usize), (usize, usize)> = HashMap::new();
        for (_, it) in b.items() {
            if let board::Kind::Spoke { pad, need, .. } = it.kind {
                for l in (0..b.layers.len()).filter(|&l| it.on(l)) {
                    let e = of.entry((pad, l)).or_insert((0, need as usize));
                    e.0 += 1;
                }
            }
        }
        println!("{path}\t{}", of.values().filter(|(have, need)| have < need).count());
    }
}
