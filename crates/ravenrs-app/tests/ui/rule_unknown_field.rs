use ravenrs_app::BusinessObject;

#[derive(BusinessObject)]
#[rule(criteria = "[nope] > 1")]
struct Account {
    #[field(key)]
    id: i64,
}

fn main() {}
