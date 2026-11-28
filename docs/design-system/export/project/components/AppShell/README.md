# AppShell

The window of an application made from its model: navigation by module, lists, forms and the theme menu.

**Family:** Application · **Rust:** `ravenrs::prelude::AppShell`

## Settings

| Setting | Control | Default | What it does |
|---|---|---|---|
| `name` | text | `Raven Contas` | The application's name, at the top of the navigation. |
| `subtitle` | text | `` | A line under the name; empty, none. |
| `theme_menu` | on/off | `true` | The theme menu at the foot of the navigation. |
| `section` | text | `` | The section that gathers the group Cadastros, which then is one item with its lists in a submenu; empty, none. |

## Stories

- **Lists**: 728 × 441 pt, settings `{"name":"Raven Contas","section":"","subtitle":"","theme_menu":true}`
- **With subtitle**: 728 × 441 pt, settings `{"name":"Raven Contas","section":"","subtitle":"Finanças da família","theme_menu":true}`
- **No theme menu**: 728 × 441 pt, settings `{"name":"Raven Contas","section":"","subtitle":"","theme_menu":false}`
- **In sections**: 728 × 441 pt, settings `{"name":"Raven Contas","section":"Finanças","subtitle":"","theme_menu":true}`

## Sizing

Hugs its content: 248 × 441 pt in a parent of 760 pt; its height follows the width, as its content wraps.

## Interaction

The pointer over the first control, the focus brought to it with Tab and the button held down on it, measured in Raven Dark on the story Lists:

- **Hover** on "Bancos": fill transparent → ink@5; color ink-muted → ink.
- **Focus** on "Contas": adds a rect (border 2.0 focus-ring); color mix(ink, accent, 35) → ink-muted.
- **Pressed** on "Bancos": fill transparent → ink@5; color ink-muted → ink.

## Behavior

- **Keys** its first control handles once focused: `ArrowDown`, `ArrowUp`, `Escape`, `End`.
- **Accessibility**: role `Navigation` "Navegação".
- **Focus order** (Tab): Button "Contas" → Button "Bancos" → Button "Tema" → Button "Novo" → Button "Atualizar" → TextInput "Buscar em todas as colunas" → Button "Agrupar" → Button "Colunas" → Button "Filtro" → Button "Exportar" → TextInput "Filtrar Nome" → TextInput "Filtrar Tipo" → TextInput "Filtrar Banco" → TextInput "Filtrar Saldo inicial" → TextInput "Filtrar Aberta em" → TextInput "Filtrar Ativa" → Grid "Grade de dados".

## Anatomy (Raven Dark, every story)

Measured from what RavenRS paints, in points. Colors are tokens; `token@N` is the token at N% opacity. Parts that measure the same are listed once, with the story they come from.

| Story | Part | Size | Radius | Fill | Border | Text | Icon | Left padding |
|---|---|---|---|---|---|---|---|---|
| Lists | Navigation "Navegação" | 248.0 × 441.0 | 0.0 | bg-100 | — | Inter 600 17.0px ink | wallet 16.0px mix(ink, accent, 35) | space-3 |
| Lists | Heading "Raven Contas" | 224.0 × 24.0 | — | — | — | Inter 600 17.0px ink | — | 0.0 |
| Lists | Label "CADASTROS" | 79.0 × 16.0 | — | — | — | Inter 600 12.0px ink-muted | — | 0.0 |
| Lists | Button "Contas" | 224.0 × 36.0 | radius-md | bg-300 | — | Inter 600 15.0px ink | wallet 16.0px mix(ink, accent, 35) | 10.0 |
| Lists | Button "Bancos" | 224.0 × 36.0 | radius-md | transparent | — | Inter 400 15.0px ink-muted | file 16.0px ink-muted | 10.0 |
| Lists | Button "Tema" | 103.0 × 32.0 | radius-md | bg-300 | 1.0 border | Inter 600 13.0px ink | moon 14.0px ink | space-3 |
| Lists | Button "Novo" | 79.0 × 32.0 | radius-md | accent | — | Inter 600 13.0px on-accent | plus 14.0px on-accent | space-3 |
| Lists | Button "Excluir" | 89.0 × 32.0 | radius-md | transparent | — | Inter 600 13.0px ink@45 | trash-2 14.0px ink@45 | space-3 |
| Lists | Button "Atualizar" | 102.0 × 32.0 | radius-md | transparent | — | Inter 600 13.0px ink | refresh-cw 14.0px ink | space-3 |
| Lists | TextInput "Buscar em todas as colunas" | 240.0 × 32.0 | radius-md | bg-300 | 1.0 control-border | Inter 400 13.0px ink-muted | search 16.0px ink-muted | space-2 |
| Lists | Button "Agrupar" | 120.0 × 32.0 | radius-md | bg-300 | 1.0 border | Inter 600 13.0px ink | layers 14.0px ink | space-3 |
| Lists | Button "Colunas" | 120.0 × 32.0 | radius-md | bg-300 | 1.0 border | Inter 600 13.0px ink | table 14.0px ink | space-3 |
| Lists | Label "3 de 3 linhas" | 79.0 × 18.0 | — | — | — | Inter 400 13.0px ink-muted | — | 0.0 |
| Lists | Button "Filtro" | 79.0 × 32.0 | radius-md | bg-300 | 1.0 border | Inter 600 13.0px ink | filter 14.0px ink | space-3 |
| Lists | Button "Exportar" | 122.0 × 32.0 | radius-md | bg-300 | 1.0 border | Inter 600 13.0px ink | download 14.0px ink | space-3 |
| Lists | CheckBox "Selecionar linha" | 44.0 × 38.0 | — | — | — | — | — | — |
| Lists | ColumnHeader "Nome" | 154.0 × 38.0 | — | — | — | Inter 600 12.0px ink-muted | — | space-3 |
| Lists | ColumnHeader "Saldo inicial" | 114.0 × 38.0 | — | — | — | — | — | — |
| Lists | TextInput "Filtrar Nome" | 160.0 × 38.0 | radius-sm | transparent | — | Inter 400 13.0px ink-muted | — | space-3 |
| Lists | TextInput "Filtrar Saldo inicial" | 120.0 × 38.0 | — | — | — | — | — | — |
| Lists | Row | 864.0 × 36.0 | 0.0 | transparent | — | Inter 400 13.0px ink | — | 56.0 |
| Lists | CheckBox "Selecionar Conta corrente" | 44.0 × 36.0 | — | — | — | — | — | — |
| Lists | Cell "Conta corrente" | 160.0 × 36.0 | — | — | — | Inter 400 13.0px ink | — | space-3 |
| Lists | Cell "R$ 1.250,50" | 120.0 × 36.0 | — | — | — | — | — | — |
| Lists | Row | 864.0 × 36.0 | 0.0 | bg-300@40 | — | Inter 400 13.0px ink | — | 56.0 |
| Lists | Status "Mostrando Contas" | 0.0 × 0.0 | — | — | — | — | — | — |
| In sections | Navigation "Navegação" | 248.0 × 441.0 | 0.0 | bg-100 | — | Inter 600 17.0px ink | folder 16.0px ink-muted | space-3 |
| In sections | Button "Cadastros" | 224.0 × 36.0 | radius-md | transparent | — | Inter 400 15.0px ink-muted | folder 16.0px ink-muted | 10.0 |

## Example

```rust
fn app_shell() -> impl View {
    use ravenrs_app::{
        Application, EntityInfo, EnumOption, FieldInfo, FieldKind, MemoryDatabase, ObjectSpace,
    };
    use ravenrs_app_ui::{AppShell, EditorRegistry, SystemModule, Words};

    let bank = EntityInfo::new("Bank")
        .caption("Bancos")
        .navigation("Cadastros")
        .icon("landmark")
        .field(FieldInfo::new("code", FieldKind::Text).caption("Código").typed_key())
        .field(FieldInfo::new("name", FieldKind::Text).caption("Nome").default_property());
    let account = EntityInfo::new("Account")
        .caption("Contas")
        .navigation("Cadastros")
        .icon("wallet")
        .field(FieldInfo::new("id", FieldKind::Integer).key())
        .field(FieldInfo::new("name", FieldKind::Text).caption("Nome").default_property())
        .field(
            FieldInfo::new(
                "kind",
                FieldKind::Enum(vec![
                    EnumOption::new("checking", "Corrente"),
                    EnumOption::new("savings", "Poupança"),
                ]),
            )
            .caption("Tipo")
            .default_value("checking"),
        )
        .field(FieldInfo::new("bank", FieldKind::Reference("Bank".into())).caption("Banco"))
        .field(FieldInfo::new("opening", FieldKind::Money).caption("Saldo inicial").default_value(0.0))
        .field(FieldInfo::new("opened", FieldKind::Date).caption("Aberta em"))
        .field(FieldInfo::new("active", FieldKind::Boolean).caption("Ativa").default_value(true))
        .field(FieldInfo::new("notes", FieldKind::LongText).caption("Observações"));
    let entities = vec![account, bank];
    let Ok(database) = MemoryDatabase::new(entities.clone()) else {
        unreachable!("the entities are declared right above");
    };
    let mut setup = database.space();
    for (code, name) in [("001", "Banco do Brasil"), ("341", "Itaú")] {
        if let Ok(mut object) = setup.new_object("Bank") {
            object.set("code", code);
            object.set("name", name);
            let _ = setup.save(object);
        }
    }
    for (name, cents) in [("Conta corrente", 125_050.0), ("Poupança", 980_000.0), ("Carteira", 12_000.0)] {
        if let Ok(mut object) = setup.new_object("Account") {
            object.set("name", name);
            object.set("opening", cents);
            object.set("bank", "341");
            let _ = setup.save(object);
        }
    }
    let _ = setup.commit();
    let builder = Application::builder("Raven Contas").module(SystemModule::with_words(Words::pt_br()));
    let application = entities.into_iter().fold(builder, |builder, entity| builder.entity(entity)).build();
    application.ok().map(|application| {
        Localized::new(
            Labels::pt_br(),
            AppShell::with_options(
                application,
                move || database.space(),
                Words::pt_br(),
                EditorRegistry::default(),
            ),
        )
    })
}
```

Every number above is in `AppShell.json` beside this card.
