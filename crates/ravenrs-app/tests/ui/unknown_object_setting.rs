use ravenrs_app::BusinessObject;

#[derive(BusinessObject)]
#[object(captions = "x")]
struct Account {
    #[field(key)]
    id: i64,
}

fn main() {}
