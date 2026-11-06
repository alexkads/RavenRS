//! The devices of the counter: which driver each one uses, by its exact model, its version, how far it
//! was tested and what can be done with it. Drivers are data, never code: see `docs/PERIFERICOS.md`.

use std::cell::RefCell;
use std::rc::Rc;

use ravenrs_app_ui::Session;
use ravenrs_layout::Align;
use ravenrs_perifericos::{Manifesto, Registro, Situacao, TipoPeriferico, Version};
use ravenrs_reactive::State;
use ravenrs_report_label::{LabelJob, Language};
use ravenrs_style::{Semantic, TextStyle, Weight};
use ravenrs_widgets::{
    Alert, AnyView, BuildCx, Button, ButtonStyle, Card, HStack, NodeId, ScrollView, Spacer, Text, Tone,
    VStack, View, ViewExt,
};

use crate::caixa::configuracao;

/// The drivers of a checkout. A clone is the same.
#[derive(Clone, Debug)]
pub struct Perifericos(Rc<RefCell<Registro>>);

impl Default for Perifericos {
    fn default() -> Self {
        let programa = Version::parse(env!("CARGO_PKG_VERSION")).unwrap_or_else(|_| Version::new(0, 0, 0));
        Self(Rc::new(RefCell::new(Registro::novo(programa))))
    }
}

impl Perifericos {
    /// Over a registry the application keeps.
    pub fn com_registro(registro: Registro) -> Self {
        Self(Rc::new(RefCell::new(registro)))
    }

    /// The registry.
    pub fn registro(&self) -> std::cell::Ref<'_, Registro> {
        self.0.borrow()
    }

    /// The active drivers of this kind, by model.
    pub fn do_tipo(&self, tipo: TipoPeriferico) -> Vec<Manifesto> {
        let mut drivers: Vec<Manifesto> =
            self.0.borrow().ativos().filter(|driver| driver.tipo == tipo).cloned().collect();
        drivers.sort_by(|a, b| a.nome_do_modelo().cmp(b.nome_do_modelo()));
        drivers
    }

    /// The active driver with this id.
    pub fn driver(&self, id: &str) -> Option<Manifesto> {
        self.0.borrow().ativo(id).cloned()
    }

    /// What a printer takes, from its driver, or from the language alone when it has none.
    pub fn trabalho(&self, driver_id: &str, linguagem: Language) -> LabelJob {
        self.driver(driver_id)
            .and_then(|driver| LabelJob::do_driver(&driver).ok())
            .unwrap_or_else(|| LabelJob::new(linguagem, 203))
    }

    /// Goes back to the version used before.
    pub fn voltar(&self, id: &str) -> Result<Version, String> {
        self.0.borrow_mut().voltar(id).map_err(|error| error.to_string())
    }
}

/// How far a driver was tested, for people.
pub fn situacao_em_texto(situacao: &Situacao) -> (String, Tone) {
    match situacao {
        Situacao::TestadoNoAparelho { data, por, firmware } => {
            (format!("Testado no aparelho em {data} por {por}, firmware {firmware}"), Tone::Success)
        }
        Situacao::SoPelaDocumentacao { .. } => {
            ("Só pela documentação do fabricante".to_owned(), Tone::Warning)
        }
        _ => ("Presumido, sem documento nem teste".to_owned(), Tone::Danger),
    }
}

fn nome_do_tipo(tipo: TipoPeriferico) -> &'static str {
    match tipo {
        TipoPeriferico::ImpressoraTermica => "Impressoras de cupom",
        TipoPeriferico::Etiquetadora => "Impressoras de etiqueta",
        TipoPeriferico::Pinpad => "Pinpads",
        TipoPeriferico::LeitorCodigo => "Leitores de código",
        TipoPeriferico::Balanca => "Balanças",
        TipoPeriferico::Gaveta => "Gavetas",
    }
}

/// The screen of devices.
pub(crate) struct TelaPerifericos {
    pub(crate) perifericos: Perifericos,
    pub(crate) session: Session,
}

impl View for TelaPerifericos {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let aviso: State<Option<(Tone, String)>> = cx.use_state(|| None);
        let em_uso =
            [configuracao(&self.session, "driver_cupom"), configuracao(&self.session, "driver_etiqueta")];
        let mut linhas: Vec<AnyView> = vec![
            Box::new(Text::new("Periféricos").style(TextStyle::Title)),
            Box::new(
                Text::new(
                    "Cada driver é de um modelo exato. Atualizações chegam por catálogo assinado e sempre dá para voltar.",
                )
                .foreground(Semantic::InkMuted),
            ),
        ];
        if let Some((tone, texto)) = aviso.get() {
            linhas.push(Box::new(Alert::new("Drivers", texto).tone(tone)));
        }
        let tipos = [
            TipoPeriferico::ImpressoraTermica,
            TipoPeriferico::Etiquetadora,
            TipoPeriferico::Pinpad,
            TipoPeriferico::LeitorCodigo,
        ];
        for tipo in tipos {
            let drivers = self.perifericos.do_tipo(tipo);
            if drivers.is_empty() {
                continue;
            }
            linhas.push(Box::new(Text::new(nome_do_tipo(tipo)).style(TextStyle::Headline)));
            for driver in drivers {
                let (situacao, tone) = situacao_em_texto(&driver.situacao);
                let versoes = self.perifericos.registro().versoes(&driver.id).len();
                let mut acoes: Vec<AnyView> = Vec::new();
                if versoes > 1 {
                    let (perifericos, aviso, id) =
                        (self.perifericos.clone(), aviso.clone(), driver.id.clone());
                    acoes.push(Box::new(
                        Button::new("Voltar à versão anterior").style(ButtonStyle::Secondary).on_click(
                            move || {
                                aviso.set(Some(match perifericos.voltar(&id) {
                                    Ok(versao) => (Tone::Success, format!("{id} voltou para {versao}.")),
                                    Err(why) => (Tone::Danger, why),
                                }));
                            },
                        ),
                    ));
                }
                let mut titulo: Vec<AnyView> = vec![Box::new(
                    Text::new(format!("{} {}", driver.fabricante, driver.nome_do_modelo()))
                        .style(TextStyle::Headline),
                )];
                if em_uso.contains(&driver.id) {
                    titulo.push(Box::new(
                        Text::new("em uso").font_weight(Weight::SEMIBOLD).foreground(Semantic::Accent),
                    ));
                }
                let variacoes: Vec<String> =
                    driver.modelos.iter().flat_map(|modelo| modelo.variacoes.iter().cloned()).collect();
                let mut detalhes: Vec<AnyView> = vec![
                    Box::new(HStack::new(titulo).spacing(12.0).align(Align::Center)),
                    Box::new(
                        Text::new(format!(
                            "Driver {} versão {}, protocolo {:?}",
                            driver.id, driver.versao, driver.protocolo.nome
                        ))
                        .style(TextStyle::Callout)
                        .foreground(Semantic::InkMuted),
                    ),
                ];
                if !variacoes.is_empty() {
                    detalhes.push(Box::new(
                        Text::new(format!("Também: {}", variacoes.join(", ")))
                            .style(TextStyle::Callout)
                            .foreground(Semantic::InkMuted),
                    ));
                }
                detalhes.push(Box::new(Text::new(situacao).style(TextStyle::Callout).foreground(
                    match tone {
                        Tone::Success => Semantic::Success,
                        Tone::Danger => Semantic::Danger,
                        _ => Semantic::Warning,
                    },
                )));
                linhas.push(Box::new(Card::new(
                    HStack::new(vec![
                        Box::new(VStack::new(detalhes).spacing(4.0).align(Align::Stretch).grow(1.0))
                            as AnyView,
                        Box::new(Spacer::new()),
                        Box::new(HStack::new(acoes).spacing(8.0)),
                    ])
                    .align(Align::Center),
                )));
            }
        }
        let aviso_catalogo = aviso.clone();
        linhas.push(Box::new(Button::new("Procurar atualizações").style(ButtonStyle::Secondary).on_click(
            move || {
                aviso_catalogo.set(Some((
                    Tone::Warning,
                    "Nenhum catálogo de drivers está configurado nesta versão beta.".to_owned(),
                )));
            },
        )));
        cx.child(
            &ScrollView::new(vec![Box::new(
                VStack::new(linhas).spacing(12.0).align(Align::Stretch).padding(16.0),
            ) as AnyView])
            .grow(1.0),
        )
    }
}
