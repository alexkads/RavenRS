# EntityForm

A form made from an entity of the model, in a window: each field with the editor its kind asks for, the rules checked on save.

**Family:** Application · **Rust:** `ravenrs::prelude::EntityForm`

## Settings

| Setting | Control | Default | What it does |
|---|---|---|---|
| `title` | text | `Novo lançamento` |  |
| `save` | text | `Salvar lançamento` | The button that keeps the record; it carries a check. |
| `description` | text | `` | A line under the title; empty, none. |
| `width` | number 320–880 by 1 | `440` | The width of the window; two columns of fields, `wide()` ones across. |
| `show_errors` | on/off | `false` | Saved with the fields empty, so the rules show their messages. |

## Stories

- **New record**: 584 × 794 pt, settings `{"description":"","save":"Salvar lançamento","show_errors":false,"title":"Novo lançamento","width":440.0}`
- **With description**: 584 × 812 pt, settings `{"description":"Despesas entram no mês da data.","save":"Salvar lançamento","show_errors":false,"title":"Novo lançamento","width":440.0}`
- **Wide**: 664 × 794 pt, settings `{"description":"","save":"Salvar lançamento","show_errors":false,"title":"Novo lançamento","width":600.0}`
- **Rules not met**: 584 × 818 pt, settings `{"description":"","save":"Salvar lançamento","show_errors":true,"title":"Novo lançamento","width":440.0}`
- **Every kind of field**: 584 × 892 pt, settings `{"description":"","save":"Salvar","show_errors":false,"title":"Campos","width":440.0}`

## Sizing

Hugs its content: 440 × 520 pt in a parent of 760 pt; in a parent of 320 pt it narrows to 288 pt; its height follows the width, as its content wraps.

## Interaction

The pointer over the first control, the focus brought to it with Tab and the button held down on it, measured in Raven Dark on the story New record:

- **Hover** on "Fechar": fill transparent → ink@7; border {color:control-border,width:1.0} → {color:accent,width:1.5}.
- **Focus** on "Fechar": border {color:control-border,width:1.0} → {color:accent,width:1.5}; adds a rect (accent); adds a rect (border 2.0 focus-ring); takes a shape away.
- **Pressed** on "Fechar": adds a rect (ink@12); adds a icon (null); takes a shape away; takes a shape away.

## Behavior

- **Keys** its first control handles once focused: `ArrowDown`, `ArrowUp`, `Space`, `Escape`.
- **Accessibility**: role `Dialog` "Novo lançamento".
- **Focus order** (Tab): Button "Fechar" → RadioButton "Despesa" → RadioButton "Receita" → TextInput "Descrição" → TextInput "Valor" → ComboBox "Data" → ComboBox "Conta" → ComboBox "Categoria" → ComboBox "De quem" → Button "Cancelar" → Button "Salvar lançamento".

## Anatomy (Raven Dark, every story)

Measured from what RavenRS paints, in points. Colors are tokens; `token@N` is the token at N% opacity. Parts that measure the same are listed once, with the story they come from.

| Story | Part | Size | Radius | Fill | Border | Text | Icon | Left padding |
|---|---|---|---|---|---|---|---|---|
| New record | Dialog "Novo lançamento" | 440.0 × 520.0 | radius-xl | bg-200 | 1.0 border | Inter 600 22.0px ink | x 14.0px ink | space-6 |
| New record | Heading "Novo lançamento" | 348.0 × 28.0 | — | — | — | Inter 600 22.0px ink | — | 0.0 |
| New record | Button "Fechar" | 32.0 × 32.0 | radius-md | transparent | — | — | x 14.0px ink | 9.0 |
| New record | Label "Tipo" | 29.0 × 18.0 | — | — | — | Inter 600 13.0px ink | — | 0.0 |
| New record | RadioGroup "Tipo" | 166.0 × 36.0 | radius-md | bg-300 | — | Inter 600 13.0px ink | — | 17.0 |
| New record | RadioButton "Despesa" | 83.0 × 30.0 | 7.0 | bg-200 | 1.0 border | Inter 600 13.0px ink | — | 14.0 |
| New record | RadioButton "Receita" | 75.0 × 30.0 | — | — | — | Inter 600 13.0px ink-muted | — | 14.0 |
| New record | Label "*" | 8.0 × 18.0 | — | — | — | Inter 600 13.0px danger | — | 0.0 |
| New record | TextInput "Descrição" | 392.0 × 40.0 | radius-md | bg-300 | 1.0 control-border | Inter 400 15.0px ink-muted | — | space-3 |
| New record | ComboBox "Data" | 188.0 × 40.0 | radius-md | bg-300 | 1.0 control-border | Inter 400 15.0px ink-muted | calendar 16.0px ink-muted | space-3 |
| New record | ComboBox "Conta" | 188.0 × 40.0 | radius-md | bg-300 | 1.0 control-border | Inter 400 15.0px ink | chevron-down 16.0px ink-muted | space-3 |
| New record | Button "Cancelar" | 101.0 × 40.0 | radius-md | bg-300 | 1.0 border | Inter 600 15.0px ink | — | 18.0 |
| New record | Button "Salvar lançamento" | 195.0 × 40.0 | radius-md | accent | — | Inter 600 15.0px on-accent | check 16.0px on-accent | 18.0 |
| With description | Dialog "Novo lançamento" | 440.0 × 556.0 | radius-xl | bg-200 | 1.0 border | Inter 600 22.0px ink | x 14.0px ink | space-6 |
| With description | Label "Despesas entram no mês da data." | 392.0 × 22.0 | — | — | — | Inter 400 15.0px ink-muted | — | 0.0 |
| Rules not met | Dialog "Novo lançamento" | 440.0 × 568.0 | radius-xl | bg-200 | 1.0 border | Inter 600 22.0px ink | x 14.0px ink | space-6 |
| Rules not met | Label "Descrição é obrigatório" | 145.0 × 18.0 | — | — | — | Inter 400 13.0px danger | — | 0.0 |
| Every kind of field | Dialog "Campos" | 440.0 × 716.0 | radius-xl | bg-200 | 1.0 border | Inter 600 22.0px ink | x 14.0px ink | space-6 |
| Every kind of field | TextInput "Integer" | 188.0 × 40.0 | radius-md | bg-300 | 1.0 control-border | — | — | — |
| Every kind of field | Button "Escolher horário" | 40.0 × 40.0 | radius-md | transparent | — | — | clock 16.0px ink | space-3 |
| Every kind of field | ComboBox "Color" | 188.0 × 40.0 | radius-md | bg-300 | 1.0 border | Inter 400 15.0px ink-muted | chevron-down 16.0px ink-muted | 40.0 |
| Every kind of field | CheckBox "Boolean" | 86.0 × 22.0 | — | — | — | Inter 400 15.0px ink | check 14.0px on-accent | 2.0 |
| Every kind of field | TextInput "Long text" | 392.0 × 100.0 | radius-md | bg-300 | 1.0 control-border | — | — | — |

Every number above is in `EntityForm.json` beside this card.
