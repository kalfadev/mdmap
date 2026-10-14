fn main() {
    let code = mdmap::main(std::env::args().skip(1).collect(), mdmap::Env::from_process(), &mut std::io::stdout());
    std::process::exit(code);
}
