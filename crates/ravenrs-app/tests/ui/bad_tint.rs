use ravenrs_app::BusinessObject;

#[derive(BusinessObject)]
#[appearance(names = "id", font = "red")]
struct Account {
    #[field(key)]
    id: i64,
}

fn main() {}
