use ravenrs_app::BusinessEnum;

#[derive(BusinessEnum)]
enum Situation {
    #[option(tint = "red")]
    Active,
}

fn main() {}
