fn main() {
    if let Err(e) = logsure::cli::main() {
        eprintln!("logsure: {e:#}");
        std::process::exit(1);
    }
}
