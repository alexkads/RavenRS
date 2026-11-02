use ravenrs_app::BusinessObject;

#[derive(BusinessObject)]
struct Account {
    #[field(key)]
    id: i64,
    #[field(money, long)]
    name: String,
}

fn main() {}
