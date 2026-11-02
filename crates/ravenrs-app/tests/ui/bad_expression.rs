use ravenrs_app::BusinessObject;

#[derive(BusinessObject)]
struct Account {
    #[field(key)]
    id: i64,
    #[field(calculated = "[id] +")]
    double: i64,
}

fn main() {}
