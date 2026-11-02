use ravenrs_app::BusinessObject;

#[derive(BusinessObject)]
#[rule(unique = "id,nope")]
struct Account {
    #[field(key)]
    id: i64,
}

fn main() {}
