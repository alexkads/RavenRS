//! `TextField`, `InputOtp`, `FormField` and `Label`, every feature: a search, a password, values with
//! a unit, sizes, a field that lines up with a column, a rename that starts selected, a console that
//! brings back what was sent, a one-time code and a sign-up form that validates as you type. A line
//! says what the last field did.

use std::rc::Rc;

use ravenrs::core::{Key, Modifiers};
use ravenrs::prelude::*;

/// The control's name, as the session knows it.
pub const ID: &str = "TextFieldsDemo";
/// The page's name, as the navigation shows it.
pub const PAGE: &str = "Campos de texto";

/// The user name that is already taken, as a database would say.
pub const TAKEN: &str = "ana";
/// The code the one-time boxes accept.
pub const CODE: &str = "123456";

fn heading(text: &str) -> Text {
    Text::new(text.to_owned()).style(TextStyle::Headline)
}

fn email_rule(value: &str) -> Result<(), String> {
    let ok = value.split_once('@').is_some_and(|(user, domain)| !user.is_empty() && domain.contains('.'));
    if ok { Ok(()) } else { Err("Escreva um e-mail como nome@exemplo.com".to_string()) }
}

fn length_rule(value: &str) -> Result<(), String> {
    if value.chars().count() >= 3 { Ok(()) } else { Err("Use pelo menos 3 letras".to_string()) }
}

/// The page: the fields, the code, the form, and a line with the last event.
#[derive(Debug, Default)]
pub struct TextFieldsPage;

impl View for TextFieldsPage {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let narrow = cx.viewport().width < 720.0;
        let said = cx.use_state(|| "Digite num campo e pressione Enter.".to_string());
        let query = cx.use_state(String::new);
        let password = cx.use_state(String::new);
        let price = cx.use_state(|| "49,90".to_string());
        let filter = cx.use_state(String::new);
        let big = cx.use_state(String::new);
        let cell = cx.use_state(|| "1.250,00".to_string());
        let name = cx.use_state(|| "relatorio-final.pdf".to_string());
        let console = cx.use_state(String::new);
        let sent = cx.use_state(Vec::<String>::new);
        let code = cx.use_state(String::new);
        let email = cx.use_state(String::new);
        let user = cx.use_state(String::new);
        let coupon = cx.use_state(|| "RAVEN10".to_string());
        let tried = cx.use_state(|| false);
        let width = if narrow { 200.0 } else { 280.0 };

        let (searched, cancelled) = (said.clone(), said.clone());
        let search = TextField::new(query.binding())
            .placeholder("Procurar clientes")
            .icon(IconName::Search)
            .clearable()
            .autofocus()
            .on_submit(move |text| searched.set(format!("Procurou: {text}")))
            .on_cancel(move || cancelled.set("Busca cancelada".to_string()));
        let secret = TextField::new(password.binding())
            .placeholder("Senha")
            .icon(IconName::Lock)
            .secure()
            .width(width);
        // A field with no placeholder takes its name from the FormField around it.
        let money = FormField::new(
            "Mensalidade",
            TextField::new(price.binding())
                .prefix(Text::new("R$").foreground(Semantic::InkMuted))
                .suffix(Text::new("/mês").foreground(Semantic::InkMuted))
                .end_aligned()
                .width(width),
        );
        let sizes = HStack::new((
            TextField::new(filter.binding()).placeholder("Filtro compacto").compact().width(160.0),
            TextField::new(big.binding()).placeholder("Campo grande").size(ButtonSize::Large).width(width),
        ))
        .spacing(8.0)
        .wrap()
        .align(Align::Center);
        let column = FormField::new(
            "Valor da coluna",
            TextField::new(cell.binding()).flat().end_aligned().width(160.0),
        );
        let left = said.clone();
        let rename = FormField::new(
            "Nome do arquivo",
            TextField::new(name.binding())
                .select_on_open()
                .width(width)
                .on_leave(move || left.set("Saiu do nome do arquivo".to_string())),
        );

        // The console takes the keyboard back whenever nothing has it, and Page Up asks for help
        // before the field edits anything.
        let (ran, helped) = (said.clone(), said.clone());
        let help: KeyHandler = Rc::new(move |key: &Key, _: Modifiers| {
            if *key == Key::PageUp {
                helped.set("Ajuda do console pedida".to_string());
                return true;
            }
            false
        });
        let prompt = TextField::new(console.binding())
            .placeholder("Comando do console")
            .icon(IconName::Terminal)
            .recall(sent.clone())
            .keeps_focus()
            .intercept_keys(help)
            .on_submit(move |text| ran.set(format!("Rodou: {text}")));
        let history = format!("{} comandos enviados", sent.get().len());

        let off = HStack::new((
            FormField::new(
                "Mensalidade (só leitura)",
                TextField::new(price.binding()).read_only(true).width(160.0),
            ),
            TextField::new(filter.binding()).placeholder("Desligado").disabled(true).width(160.0),
        ))
        .spacing(8.0)
        .wrap();

        let checked = said.clone();
        let otp = InputOtp::new(code.binding(), 6).on_complete(move |typed| {
            if typed == CODE {
                checked.set(format!("Código {typed} conferido"));
            } else {
                checked.set(format!("Código {typed} não confere"));
            }
        });

        let mail_field =
            FormField::new("E-mail", TextField::new(email.binding()).placeholder("nome@exemplo.com"))
                .required()
                .hint("Nunca mostramos o seu e-mail.")
                .validate(email.binding(), email_rule)
                .show_errors(tried.get());
        let taken = (user.get() == TAKEN).then(|| format!("O nome {TAKEN} já está em uso"));
        let user_field = FormField::new("Usuário", TextField::new(user.binding()))
            .validate(user.binding(), length_rule)
            .error_text(taken.clone())
            .accessible_description("Como os outros veem você")
            .show_errors(tried.get());
        let coupon_field = FormField::new("Cupom", TextField::new(coupon.binding()))
            .accessible_name("Cupom de desconto")
            .label_ink(Semantic::Accent)
            .tint(Semantic::Surface)
            .note_text(Some("Vale 10% na primeira compra".to_string()), Tone::Success);
        let problem = mail_field.error().or_else(|| user_field.error()).or(taken);
        let (attempt, outcome) = (tried.clone(), said.clone());
        let verdict = problem.clone();
        let submit = Button::new("Criar conta").on_click(move || {
            attempt.set(true);
            match &verdict {
                Some(why) => outcome.set(format!("Recusado: {why}")),
                None => outcome.set("Conta criada".to_string()),
            }
        });

        let label = Label::new("Telefone").required().hint("Com DDD").ink(Semantic::Ink);
        let label_line = format!("O rótulo diz: {}", label.text());

        cx.child(
            &VStack::new((
                VStack::new((heading("Busca e senha"), search, secret)).spacing(8.0).align(Align::Stretch),
                VStack::new((heading("Valores e tamanhos"), money, sizes, column))
                    .spacing(8.0)
                    .align(Align::Start),
                VStack::new((heading("Renomear"), rename)).spacing(8.0).align(Align::Start),
                VStack::new((heading("Console"), prompt, Text::new(history)))
                    .spacing(8.0)
                    .align(Align::Stretch),
                VStack::new((heading("Só leitura e desligado"), off)).spacing(8.0),
                VStack::new((heading("Código de uso único"), otp)).spacing(8.0).align(Align::Start),
                VStack::new((heading("Cadastro"), mail_field, user_field, coupon_field, submit))
                    .spacing(8.0)
                    .width(if narrow { 300.0 } else { 420.0 })
                    .align(Align::Stretch),
                VStack::new((heading("Rótulo"), label, Text::new(label_line)))
                    .spacing(8.0)
                    .align(Align::Start),
                Text::new(said.get()),
            ))
            .spacing(16.0)
            .padding(if narrow { 0.0 } else { 16.0 })
            .align(Align::Stretch),
        )
    }
}
