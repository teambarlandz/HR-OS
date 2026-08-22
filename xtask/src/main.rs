fn main() {
    let task = std::env::args().nth(1).unwrap_or_else(|| "help".to_string());
    match task.as_str() {
        "qemu-arm" => println!("cargo run --target thumbv7em-none-eabihf --release"),
        "qemu-riscv" => println!("cargo run --target riscv32imac-unknown-none-elf --release"),
        "fuzz" => println!("cargo fuzz run uart_mutator -- -max_total_time=60"),
        "bench" => println!("cargo bench --target thumbv7em-none-eabihf"),
        _ => println!("xtask: qemu-arm | qemu-riscv | fuzz | bench"),
    }
}
