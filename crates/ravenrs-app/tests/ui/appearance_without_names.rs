use ravenrs_app::BusinessObject;

#[derive(BusinessObject)]
#[appearance(font = "danger")]
struct Account {
    #[field(key)]
    id: i64,
}

fn main() {}
