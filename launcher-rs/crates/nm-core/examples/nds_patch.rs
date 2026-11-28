//! The native NDS patcher with hails_patcher.py's command line, for comparing the two:
//! nds_patch --here <patcher folder> [--dry-run] [--restore] [--key FILE] targets...
use nm_core::nds_patch::{download_table, Args, Line, Patcher};

fn main() {
    let mut a = Args::default();
    let mut here = std::env::current_dir().unwrap();
    let mut it = std::env::args().skip(1);
    while let Some(s) = it.next() {
        match s.as_str() {
            "--here" => here = it.next().unwrap_or_default().into(),
            "--dry-run" => a.dry_run = true,
            "--restore" => a.restore = true,
            "--key" => a.key = it.next(),
            _ => a.targets.push(s),
        }
    }
    let temp = std::env::temp_dir();
    let mut dl = move |out: &mut dyn FnMut(Line)| download_table(&temp, out);
    let mut p = Patcher { here, download: &mut dl };
    let code = p.run(&a, &mut |l: Line| if l.err { eprintln!("{}", l.text) } else { println!("{}", l.text) });
    std::process::exit(code);
}
