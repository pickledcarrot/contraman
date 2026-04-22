fn main() {
    if let Err(err) = contraman::run() {
        eprintln!("Error: {err}");
        std::process::exit(1);
    }
}
