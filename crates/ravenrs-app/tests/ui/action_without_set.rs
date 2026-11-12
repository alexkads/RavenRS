use ravenrs_app::BusinessObject;

#[derive(BusinessObject)]
#[action(caption = "Concluir")]
struct Task {
    #[field(key)]
    id: i64,
}

fn main() {}
