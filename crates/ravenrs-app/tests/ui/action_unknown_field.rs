use ravenrs_app::BusinessObject;

#[derive(BusinessObject)]
#[action(caption = "Concluir", set = "state = 'done'")]
struct Task {
    #[field(key)]
    id: i64,
}

fn main() {}
