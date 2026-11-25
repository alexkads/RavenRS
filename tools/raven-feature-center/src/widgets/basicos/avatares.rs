//! Avatar: initials on a color picked from the name, sizes, a picture in place of the initials, a
//! ring for the selected one of a row, and the circle or rounded square it is drawn in.
use ravenrs::prelude::*;

/// The control registered in the center.
pub const ID: &str = "AvatarDemo";
/// The page's navigation caption.
pub const PAGE: &str = "Avatar";

const TEAM: [&str; 3] = ["Ana Lima", "Bruno Reis", "Carla Dias"];

/// A gradient standing in for a profile picture.
fn portrait() -> ImageData {
    ImageData::from_fn(64, 64, |x, y| {
        let (u, v) = (x as f32 / 64.0, y as f32 / 64.0);
        Color::rgb(0.25 + 0.5 * u, 0.35 + 0.3 * v, 0.85 - 0.4 * u)
    })
    .unwrap_or_else(|| unreachable!("the size is not zero"))
}

/// Every public feature of Avatar on one page.
#[derive(Debug, Default)]
pub struct AvatarsPage;

impl View for AvatarsPage {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let chosen = cx.use_state(|| 0_usize);
        let team: Vec<AnyView> = TEAM
            .iter()
            .enumerate()
            .map(|(i, name)| {
                let mut avatar = Avatar::new(*name);
                if chosen.get() == i {
                    avatar = avatar.border(2.0, Semantic::Accent);
                }
                let pick = chosen.clone();
                Box::new(
                    HStack::new((
                        avatar,
                        Button::new(*name)
                            .style(ButtonStyle::Ghost)
                            .size(ButtonSize::Small)
                            .on_click(move || pick.set(i)),
                    ))
                    .spacing(4.0)
                    .align(Align::Center),
                ) as AnyView
            })
            .collect();
        cx.child(
            &VStack::new((
                Text::new(PAGE).style(TextStyle::Title),
                Text::new("As iniciais sobre uma cor tirada do nome, ou a foto da pessoa, num círculo ou num quadrado de cantos arredondados.")
                    .foreground(Semantic::InkMuted),
                VStack::new((
                    Text::new("Tamanhos").style(TextStyle::Headline),
                    HStack::new((
                        Avatar::new("Ana Lima").size(24.0),
                        Avatar::new("Ana Lima"),
                        Avatar::new("Ana Lima").size(48.0),
                    ))
                    .spacing(12.0)
                    .align(Align::Center),
                ))
                .spacing(12.0)
                .align(Align::Start),
                VStack::new((
                    Text::new("Forma e foto").style(TextStyle::Headline),
                    Text::new("O quadrado arredondado é como a foto do perfil costuma aparecer; a foto é recortada na mesma forma.")
                        .foreground(Semantic::InkMuted),
                    HStack::new((
                        Avatar::new("Conta Redonda").image(Image::new(portrait())),
                        Avatar::new("Conta Quadrada").shape(AvatarShape::Rounded),
                        Avatar::new("Foto Quadrada").shape(AvatarShape::Rounded).image(Image::new(portrait())),
                    ))
                    .spacing(12.0)
                    .align(Align::Center),
                ))
                .spacing(12.0)
                .align(Align::Start),
                VStack::new((
                    Text::new("O escolhido de uma fileira").style(TextStyle::Headline),
                    HStack::new(team).spacing(8.0).wrap(),
                    Text::new(format!("Escolhida: {}", TEAM[chosen.get()])),
                ))
                .spacing(12.0)
                .align(Align::Start),
            ))
            .spacing(12.0)
            .padding(16.0)
            .max_width(720.0)
            .align(Align::Start),
        )
    }
}
