use ravenrs_app::BusinessObject;

#[derive(BusinessObject)]
struct Account {
    #[field(key)]
    id: i64,
    #[field(typed_key)]
    code: String,
}

fn main() {}
