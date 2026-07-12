fn main() {
    if let Err(error) = golden_codegen_support::run_cli() {
        eprintln!("{error}");
        std::process::exit(1);
    }
}
