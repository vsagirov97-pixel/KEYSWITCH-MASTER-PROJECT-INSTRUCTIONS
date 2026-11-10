//! Spike measurements: model load time and conversion latency. Run with --release.
#![allow(clippy::print_stdout)]

use std::time::Instant;

fn main() {
    let t = Instant::now();
    let c = ks_core::builtin().expect("builtin");
    println!("load: {:?}", t.elapsed());

    let phrase = "Ghbdtn, rfr ltkf? Z ctujlyz gjpdjy. ";
    for target in [500usize, 5000] {
        let text: String = phrase.chars().cycle().take(target).collect();
        let t = Instant::now();
        let n = 20;
        for _ in 0..n {
            std::hint::black_box(c.convert(&text, "en", "ru").expect("convert"));
        }
        println!("convert {target} chars: {:?} avg", t.elapsed() / n);
    }
    // Worst case: one long token full of ambiguous punctuation.
    let text: String = "a,.;'[]".chars().cycle().take(5000).collect();
    let t = Instant::now();
    std::hint::black_box(c.convert(&text, "en", "ru").expect("convert"));
    println!(
        "worst-case 5000-char token, letters + ambiguous: {:?}",
        t.elapsed()
    );
}
