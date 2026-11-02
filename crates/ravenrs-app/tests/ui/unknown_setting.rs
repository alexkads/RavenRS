use ravenrs_app::BusinessObject;

#[derive(BusinessObject)]
struct Account {
    #[field(key)]
    id: i64,
    #[field(captoin = "Nome")]
    name: String,
}

fn main() {}
