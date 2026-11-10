//! Prints the commands the description carries and the core does not answer yet (the table of `DESIGN.md`).
//!
//! Pattern: none (example).

fn main() {
    let mut last = String::new();
    for (t, c) in fs_files_types::unsupported() {
        if t != last {
            print!("\n{t}:");
            last = t;
        }
        print!(" {c}");
    }
    println!();
}
