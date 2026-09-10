use clap::Parser;
#[derive(Parser)]
struct Args {
    input: String,
}
fn main() {
    let args = Args::parse();
    let result = xcli::input::post(&args.input).and_then(|id| {
        xcli::transport::Http::new().and_then(|http| xcli::providers::fx::read(&http, &id))
    });
    match result {
        Ok(out) => println!(
            "{}",
            serde_json::to_string_pretty(&out).expect("serializable output")
        ),
        Err(e) => {
            eprintln!("{}", serde_json::to_string(&e).expect("serializable error"));
            std::process::exit(i32::from(e.exit_code()));
        }
    }
}
