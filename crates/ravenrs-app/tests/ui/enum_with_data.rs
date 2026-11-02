use ravenrs_app::BusinessEnum;

#[derive(BusinessEnum)]
enum Kind {
    Plain,
    Holding(i64),
}

fn main() {}
