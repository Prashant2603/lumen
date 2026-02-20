use std::env;
use std::process;
use std::time::Instant;

use flash_core::{FileMap, LineIndex, LineReader};

fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        eprintln!("Usage: flash-cli <file>");
        process::exit(1);
    }
    let path = &args[1];

    // Phase 1: Memory-map the file
    let t0 = Instant::now();
    let file_map = match FileMap::open(path) {
        Ok(fm) => fm,
        Err(e) => {
            eprintln!("Error opening file: {}", e);
            process::exit(1);
        }
    };
    let mmap_time = t0.elapsed();
    println!(
        "Opened {} ({} bytes) via mmap in {:.3}ms",
        path,
        file_map.len(),
        mmap_time.as_secs_f64() * 1000.0
    );

    // Phase 2: Build line index
    let t1 = Instant::now();
    let line_index = LineIndex::build(file_map.as_bytes());
    let index_time = t1.elapsed();
    let total_lines = line_index.line_count();
    println!(
        "Indexed {} lines in {:.3}ms",
        total_lines,
        index_time.as_secs_f64() * 1000.0
    );

    let reader = LineReader::new(file_map.as_bytes(), &line_index);
    let head_count = 100.min(total_lines);
    let tail_count = 100.min(total_lines);

    // Phase 3: Print first 100 lines
    let t2 = Instant::now();
    println!("\n=== HEAD (first {} lines) ===", head_count);
    for (line_num, text) in reader.get_lines(0, head_count) {
        println!("{:>6} | {}", line_num + 1, text);
    }
    let head_time = t2.elapsed();

    // Phase 4: Print last 100 lines
    let t3 = Instant::now();
    let tail_start = total_lines.saturating_sub(tail_count);
    println!("\n=== TAIL (last {} lines) ===", tail_count);
    for (line_num, text) in reader.get_lines(tail_start, tail_count) {
        println!("{:>6} | {}", line_num + 1, text);
    }
    let tail_time = t3.elapsed();

    // Summary
    println!("\n=== Timing Summary ===");
    println!("  mmap open:   {:.3}ms", mmap_time.as_secs_f64() * 1000.0);
    println!("  index build: {:.3}ms", index_time.as_secs_f64() * 1000.0);
    println!("  head read:   {:.3}ms", head_time.as_secs_f64() * 1000.0);
    println!("  tail read:   {:.3}ms", tail_time.as_secs_f64() * 1000.0);
    println!(
        "  total:       {:.3}ms",
        t0.elapsed().as_secs_f64() * 1000.0
    );
}
