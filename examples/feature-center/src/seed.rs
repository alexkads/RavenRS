//! What is in the center the first time it opens, and the three people who can sign in.

use ravenrs_app::{
    ObjectSpace, Query, SECURITY_ROLE, SECURITY_USER, SECURITY_USER_ROLE, Value, hash_password, local_now,
};
use ravenrs_app_report::{ANALYTICS_DASHBOARD, ANALYTICS_TILE, REPORT_DEFINITION};
use ravenrs_criteria::DateTime;
use ravenrs_report::barcode::Symbology;
use ravenrs_report::labels::{LabelStock, ShelfLabelFields, common_stocks, shelf_label};
use ravenrs_report::{Band, Control, PrinterAction, PrinterSettings, Report, Style};

/// The password of everyone here. It is a demonstration: never do this with real people.
pub const PASSWORD: &str = "raven-demo";

fn wrong(error: ravenrs_app::SpaceError) -> String {
    error.to_string()
}

/// Adds a record with these values.
fn add(space: &mut dyn ObjectSpace, entity: &str, values: &[(&str, Value)]) -> Result<Value, String> {
    let mut record = space.new_object(entity).map_err(wrong)?;
    for (field, value) in values {
        record.set(field, value.clone());
    }
    let key = space
        .entities()
        .iter()
        .find(|info| info.name == entity)
        .and_then(|info| info.key_field().map(|key| record.get(&key.name)))
        .unwrap_or(Value::Null);
    space.save(record).map_err(wrong)?;
    Ok(key)
}

fn t(text: &str) -> Value {
    Value::Text(text.to_owned())
}

fn n(number: f64) -> Value {
    Value::Number(number)
}

/// This many days and hours from the start of this month.
fn in_month(day: i64, hour: u32) -> Value {
    let now = local_now();
    let date = day_of_month(now.days);
    Value::Date(DateTime { days: now.days - i64::from(date) + day, seconds: hour * 3600 })
}

/// Which day of its month this is.
fn day_of_month(days: i64) -> u8 {
    ravenrs::prelude::Date::from_days(days).day()
}

fn soon(seconds: i64) -> Value {
    let now = local_now();
    let at = now.days * 86_400 + i64::from(now.seconds) + seconds;
    Value::Date(DateTime { days: at.div_euclid(86_400), seconds: at.rem_euclid(86_400) as u32 })
}

fn people(space: &mut dyn ObjectSpace) -> Result<(), String> {
    let roles = space.query(SECURITY_ROLE, &Query::all()).map_err(wrong)?;
    let role = |name: &str| {
        roles
            .iter()
            .find(|role| role.get("name") == t(name))
            .map(|role| role.get("id"))
            .ok_or_else(|| format!("no role {name}"))
    };
    let hash = hash_password(PASSWORD)?;
    for (name, role_name) in [("ana", "Administradores"), ("bia", "Vendas"), ("caio", "Leitura")] {
        let key = add(
            space,
            SECURITY_USER,
            &[("name", t(name)), ("password_hash", t(&hash)), ("must_change_password", Value::Bool(false))],
        )?;
        add(space, SECURITY_USER_ROLE, &[("user_id", key), ("role_id", role(role_name)?)])?;
    }
    Ok(())
}

fn demos(space: &mut dyn ObjectSpace) -> Result<(), String> {
    add(
        space,
        "ValidationRules",
        &[
            ("id", n(1.0)),
            ("name", t("Registro certo")),
            ("code", t("A1")),
            ("age", n(30.0)),
            ("email", t("ana@exemplo.com")),
        ],
    )?;
    add(
        space,
        "ValidationRules",
        &[("id", n(2.0)), ("name", t("Outro")), ("code", t("B2")), ("age", n(40.0))],
    )?;
    add(
        space,
        "ValidationSoft",
        &[("id", n(1.0)), ("name", t("Pedido")), ("discount", n(10.0)), ("amount", n(500.0))],
    )?;
    add(space, "ValidationOnLeave", &[("id", n(1.0)), ("name", t("Recife")), ("city", t("Recife"))])?;
    for (id, name, state, amount, notes) in [
        (1.0, "Em dia", "ok", 10_000.0, "Tudo certo"),
        (2.0, "Atrasado", "late", 25_000.0, "Vencido há dias"),
        (3.0, "Bloqueado", "locked", 5_000.0, "Valor preso"),
        (4.0, "Sigiloso", "secret", 1_000.0, "Não deve aparecer"),
    ] {
        add(
            space,
            "AppearanceRules",
            &[
                ("id", n(id)),
                ("name", t(name)),
                ("state", t(state)),
                ("amount", n(amount)),
                ("notes", t(notes)),
            ],
        )?;
    }
    for (id, name, state, amount) in
        [(1.0, "Primeiro", "open", 100.0), (2.0, "Segundo", "open", 200.0), (3.0, "Fechado", "closed", 300.0)]
    {
        add(
            space,
            "ActionDemo",
            &[("id", n(id)), ("name", t(name)), ("state", t(state)), ("amount", n(amount))],
        )?;
    }
    add(
        space,
        "EditorsDemo",
        &[
            ("id", n(1.0)),
            ("name", t("Um de cada")),
            ("whole", n(21.0)),
            ("decimal", n(1.625)),
            ("money", n(123_456.0)),
            ("day", in_month(5, 0)),
            ("moment", in_month(5, 14)),
            ("kind", t("b")),
            ("tags", t("x,z")),
            ("color", t("#3f57db")),
            ("phone", t("(81) 98888-7777")),
            ("size", t("Médio")),
            ("progress", n(60.0)),
            ("rate", n(12.5)),
            ("rule", t("[whole] > 10")),
        ],
    )?;
    add(
        space,
        "CustomFieldsDemo",
        &[("id", n(1.0)), ("name", t("Com campos do modelo")), ("amount", n(21.0)), ("segment", t("Varejo"))],
    )?;
    for (id, name) in [(1.0, ("Ana", "Lima")), (2.0, ("Bia", "Rocha"))] {
        // People come from the library of ready-made records.
        add(space, "Person", &[("id", n(id)), ("first_name", t(name.0)), ("last_name", t(name.1))])?;
    }
    for (id, name, owner, parent, from, to, cost, state) in [
        (10.0, "Obra", 1.0, None, 1, 20, 500_000.0, "open"),
        (11.0, "Fundação", 1.0, Some(10.0), 2, 8, 200_000.0, "late"),
        (12.0, "Paredes", 2.0, Some(10.0), 9, 15, 150_000.0, "open"),
        (13.0, "Telhado", 2.0, Some(10.0), 16, 20, 150_000.0, "open"),
    ] {
        add(
            space,
            "ListEditorsDemo",
            &[
                ("id", n(id)),
                ("name", t(name)),
                ("owner", n(owner)),
                ("parent", parent.map_or(Value::Null, n)),
                ("starts", in_month(from, 9)),
                ("ends", in_month(to, 9)),
                ("cost", n(cost)),
                ("state", t(state)),
            ],
        )?;
    }
    for (id, name, city, amount, active) in [
        (1.0, "Alfa", "Recife", 50.0, true),
        (2.0, "Beta", "Olinda", 120.0, true),
        (3.0, "Gama", "Recife", 300.0, false),
        (4.0, "Delta", "Caruaru", 75.0, false),
    ] {
        add(
            space,
            "VariantsDemo",
            &[
                ("id", n(id)),
                ("name", t(name)),
                ("city", t(city)),
                ("amount", n(amount)),
                ("active", Value::Bool(active)),
            ],
        )?;
    }
    for (id, name, amount) in [(1.0, "Clique no valor", 10.0), (2.0, "Tente negativo", 20.0)] {
        add(
            space,
            "EditableList",
            &[("id", n(id)), ("name", t(name)), ("amount", n(amount)), ("state", t("a"))],
        )?;
    }
    for at in 1..=1500 {
        add(space, "ManyRows", &[("name", t(&format!("Linha {at}"))), ("position", n(f64::from(at)))])?;
    }
    add(space, "RelationCategory", &[("id", n(1.0)), ("name", t("Só do tipo X")), ("kind", t("x"))])?;
    add(space, "RelationCategory", &[("id", n(2.0)), ("name", t("Só do tipo Y")), ("kind", t("y"))])?;
    add(
        space,
        "RelationOwner",
        &[("id", n(1.0)), ("name", t("Pedido 1")), ("kind", t("x")), ("category", n(1.0))],
    )?;
    add(
        space,
        "RelationItem",
        &[("id", n(1.0)), ("name", t("Parafuso")), ("owner_id", n(1.0)), ("quantity", n(100.0))],
    )?;
    add(space, "RelationLabel", &[("id", n(1.0)), ("name", t("Ligada")), ("owner_id", n(1.0))])?;
    add(space, "RelationLabel", &[("id", n(2.0)), ("name", t("Solta"))])?;
    add(space, "StateDemo", &[("id", n(1.0)), ("name", t("Barato"))])?;
    add(space, "StateDemo", &[("id", n(2.0)), ("name", t("Caro")), ("cost", n(5000.0))])?;
    add(
        space,
        "ReminderDemo",
        &[("id", n(1.0)), ("name", t("Ligar para o cliente")), ("remind_at", soon(30))],
    )?;
    add(
        space,
        "ReminderDemo",
        &[("id", n(2.0)), ("name", t("Daqui a um mês")), ("remind_at", soon(30 * 86_400))],
    )?;
    for (id, name, owner, cost, subject) in [
        (1.0, "Da ana", "ana", 90_000.0, "Contrato"),
        (2.0, "Da bia", "bia", 50_000.0, "Proposta"),
        (3.0, "Do caio", "caio", 10_000.0, "Recibo"),
    ] {
        add(
            space,
            "SecuredDocument",
            &[
                ("id", n(id)),
                ("name", t(name)),
                ("owner", t(owner)),
                ("cost", n(cost)),
                ("subject", t(subject)),
            ],
        )?;
    }
    for entity in ["AuditFull", "AuditLight", "AuditCreation", "AuditFields", "AuditOwner"] {
        add(
            space,
            entity,
            &[("id", n(1.0)), ("name", t("Altere-me")), ("amount", n(1.0)), ("ignored", t("Sem rastro"))],
        )?;
    }
    add(space, "AuditPart", &[("id", n(1.0)), ("name", t("Item")), ("owner_id", n(1.0))])?;
    Ok(())
}

/// The name of the sheet of labels in columns.
pub const LABELS_REPORT: &str = "Etiquetas em folha, 3 colunas";
/// The name of the receipt on a roll, with commands for its printer.
pub const RECEIPT_REPORT: &str = "Cupom em bobina de 80 mm";

/// Labels from the catalog, in the columns of a sheet of paper.
fn labels_on_a_sheet() -> Report {
    let on_a_sheet = |stock: &LabelStock| stock.columns.is_some();
    let stock =
        common_stocks().into_iter().find(on_a_sheet).unwrap_or(LabelStock::roll("60 x 40 mm", 60.0, 40.0));
    let fields = ShelfLabelFields {
        description: "[name]".into(),
        price: "[money]".into(),
        price_format: "C2".into(),
        barcode: "[gtin]".into(),
        symbology: Symbology::Ean13,
    };
    shelf_label(&stock, LABELS_REPORT, &fields)
}

/// A receipt on a roll: one long page, with where it prints kept in the report and commands for the
/// printer at their places. The window "Impressora direta" of the designer sends it.
fn receipt_on_a_roll() -> Report {
    let roll = LabelStock::receipt("Bobina 80 mm", 72.0);
    let mut report = roll.report(RECEIPT_REPORT);
    report.printer = Some(PrinterSettings::default());
    report.detail = Band::new(roll.height)
        .control(Control::field("[name]").at(0.0, 0.0, roll.width, 14.0).style(Style::new().font_size(9.0)))
        .control(Control::printer_command(PrinterAction::Beep).at(0.0, 0.0, 40.0, 12.0));
    report.report_footer = Some(
        Band::new(14.0)
            .control(Control::printer_command(PrinterAction::Cut).at(0.0, 0.0, 40.0, 12.0))
            .control(Control::printer_command(PrinterAction::OpenDrawer).at(44.0, 0.0, 40.0, 12.0)),
    );
    report
}

fn reports(space: &mut dyn ObjectSpace) -> Result<(), String> {
    add(
        space,
        REPORT_DEFINITION,
        &[
            ("name", t("Custos por tarefa")),
            ("target_entity", t("ListEditorsDemo")),
            ("description", t("Primeiro layout gerado da lista; abra em Desenhar.")),
        ],
    )?;
    add(
        space,
        REPORT_DEFINITION,
        &[
            ("name", t(LABELS_REPORT)),
            ("target_entity", t("EditorsDemo")),
            ("description", t("Colunas numa folha A4, do catálogo de etiquetas.")),
            ("layout", t(&labels_on_a_sheet().to_json())),
        ],
    )?;
    add(
        space,
        REPORT_DEFINITION,
        &[
            ("name", t(RECEIPT_REPORT)),
            ("target_entity", t("EditableList")),
            (
                "description",
                t(
                    "Página contínua, com impressora e comandos de corte, gaveta e bipe; abra em Desenhar e use Impressora direta.",
                ),
            ),
            ("layout", t(&receipt_on_a_roll().to_json())),
        ],
    )?;
    let panel = add(space, ANALYTICS_DASHBOARD, &[("name", t("Visão geral"))])?;
    let tiles = [
        ("Custo total", "number", "", "cost:Sum"),
        ("Tarefas", "number", "", ""),
        ("Custo por responsável", "bar", "owner", "cost:Sum"),
        ("Custo por situação", "doughnut", "state", "cost:Sum"),
    ];
    for (at, (title, kind, argument, series)) in tiles.into_iter().enumerate() {
        add(
            space,
            ANALYTICS_TILE,
            &[
                ("dashboard_id", panel.clone()),
                ("title", t(title)),
                ("position", n(at as f64)),
                ("target_entity", t("ListEditorsDemo")),
                ("kind", t(kind)),
                ("argument", t(argument)),
                ("series", t(series)),
            ],
        )?;
    }
    Ok(())
}

/// Fills an empty center. Opened again over a database that has it all, it does nothing.
pub fn seed(space: &mut dyn ObjectSpace) -> Result<(), String> {
    if !space.query(SECURITY_USER, &Query::all()).map_err(wrong)?.is_empty() {
        return Ok(());
    }
    people(space)?;
    demos(space)?;
    reports(space)
}
