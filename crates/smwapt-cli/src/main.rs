fn main() {
    if let Err(err) = smwapt::run() {
        eprintln!("error: {err:#}");
        std::process::exit(1);
    }
}
