//! Files and pictures, every feature (#579): the attachments of an order with their rules, a zone
//! that imports spreadsheets and says why it refused one, a whole quiet area, and the photo of a
//! product. A line says what arrived last.

use ravenrs::prelude::*;

/// The control's name, as the session knows it.
pub const ID: &str = "FilesDemo";
/// The page's name, as the navigation shows it.
pub const PAGE: &str = "Arquivos e imagens";

/// The name of the zone that imports spreadsheets, for screen readers and the tour.
pub const IMPORT: &str = "Importar planilhas";
/// The name of the quiet zone.
pub const QUIET: &str = "Área de soltar";
/// The most spreadsheets one drop imports.
pub const MOST_SHEETS: usize = 2;

const MIB: u64 = 1024 * 1024;

fn names(files: &[PickedFile]) -> String {
    files.iter().map(|f| f.name.as_str()).collect::<Vec<_>>().join(", ")
}

/// The page: the attachments, the zones and the photo, and a line with the last event.
#[derive(Debug, Default)]
pub struct FilesPage;

impl View for FilesPage {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let narrow = cx.viewport().width < 720.0;
        let width = if narrow { 280.0 } else { 420.0 };
        let said = cx.use_state(|| "Solte arquivos numa das áreas ou escolha com o botão.".to_string());
        let attachments = cx.use_state(Vec::<PickedFile>::new);
        let photo = cx.use_state(|| None::<PickedFile>);

        let upload = VStack::new((
            Text::new("Anexos do pedido").style(TextStyle::Headline),
            FileUpload::new(attachments.binding())
                .accept(["pdf", "png", "jpg"])
                .most_files(3)
                .most_bytes(5 * MIB)
                .label("Anexos do pedido")
                .width(width),
            Text::new(format!("{} anexos: {}", attachments.with(Vec::len), attachments.with(|f| names(f))))
                .foreground(Semantic::InkMuted),
            FileUpload::new(Binding::constant(vec![PickedFile::new("contrato.pdf", vec![0_u8; 2048])]))
                .label("Contrato assinado, só para ler")
                .read_only(true)
                .width(width),
            FileUpload::new(Binding::constant(Vec::new()))
                .label("Notas fiscais, desligado")
                .disabled(true)
                .width(width),
        ))
        .spacing(8.0)
        .align(Align::Start);

        let (took, refused, quiet) = (said.clone(), said.clone(), said.clone());
        let zones = VStack::new((
            Text::new("Zonas de soltar").style(TextStyle::Headline),
            DropZone::new(
                VStack::new((
                    Text::new("Planilhas de preços").style(TextStyle::Headline),
                    Text::new(format!("CSV de até 1 MB, {MOST_SHEETS} por vez"))
                        .foreground(Semantic::InkMuted),
                ))
                .spacing(4.0),
            )
            .accept(["csv"])
            .most_files(MOST_SHEETS)
            .most_bytes(MIB)
            .label(IMPORT)
            .hint("Solte os CSV aqui")
            .choosable()
            .padding(24.0)
            .min_height(120.0)
            .on_files(move |files| took.set(format!("Importadas: {}", names(&files))))
            .on_refused(move |why| refused.set(format!("Recusado: {}", why.join(" · ")))),
            VStack::new((DropZone::new(
                Text::new("Uma área inteira, sem bordas até chegarem arquivos")
                    .foreground(Semantic::InkMuted),
            )
            .label(QUIET)
            .no_hint()
            .quiet()
            .fill()
            .on_files(move |files| quiet.set(format!("Na área: {}", names(&files)))),))
            .height(100.0)
            .align(Align::Stretch),
            DropZone::new(Text::new("Arquivo morto, não recebe nada")).label("Arquivo morto").disabled(true),
        ))
        .spacing(8.0)
        .align(Align::Stretch);

        let picture = if narrow { (120.0, 90.0) } else { (200.0, 150.0) };
        let photos = VStack::new((
            Text::new("Foto do produto").style(TextStyle::Headline),
            HStack::new((
                ImageField::new(photo.binding())
                    .size(picture.0, picture.1)
                    .label("Foto do produto")
                    .most_bytes(2 * MIB),
                ImageField::new(Binding::constant(None)).label("Foto da nota, só para ler").read_only(true),
                ImageField::new(Binding::constant(None)).label("Foto antiga, desligada").disabled(true),
            ))
            .spacing(12.0)
            .wrap(),
            Text::new(photo.get().map_or("Sem foto".to_string(), |f| format!("Foto: {}", f.name)))
                .foreground(Semantic::InkMuted),
        ))
        .spacing(8.0)
        .align(Align::Start);

        cx.child(
            &VStack::new((upload, zones, photos, Text::new(said.get())))
                .spacing(16.0)
                .padding(if narrow { 0.0 } else { 16.0 })
                .align(Align::Stretch),
        )
    }
}
