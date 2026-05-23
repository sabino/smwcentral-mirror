fn main() {
    if let Err(err) = smwapt_cli::run() {
        eprintln!("error: {err:#}");
        std::process::exit(1);
    }
}
