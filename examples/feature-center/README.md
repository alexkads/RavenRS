# Feature center

Every feature of the application framework, one small demonstration each, in the navigation by what it shows. Each list says what it is there to show and what to try. It is how to see what the framework does, how to review its screens, and how to know it all still works.

```bash
cargo run -p feature-center
```

```bash
./scripts/build-web.sh feature-center && python3 -m http.server -d examples/feature-center/web 8099
```

On the desktop it keeps its records in SQLite, in `RAVEN_CENTRO` or a file in the temporary folder, so delete that file to start over. In the browser it keeps them in memory and starts over when the page reloads.

## Signing in

These accounts exist only here. The password of all three is `raven-demo`.

| Name | Role | What to notice |
|---|---|---|
| `ana` | Administrators | Everything, with Administration: users, roles, the audit trail and the model editor |
| `bia` | Vendas | Reads every document, sees no cost, and changes only her own |
| `caio` | Leitura | Only reads, does not see ana's documents or the Audit group, and has one action taken away |

## What is here

| Group | Demonstrations |
|---|---|
| Validação | Rules of every kind, warnings and notes, checking on leaving a field |
| Aparência condicional | Colors, enabled and visible, on fields and actions |
| Actions | Simple, with a question, with a value, with a choice, with a window, an address, by criteria, inside a form, messages |
| Editores de campo | Every kind of field, and fields that come from the model |
| Listas | Chart, month, timeline and tree; variants and filters; editing on the list; many rows |
| Relações | Lists that belong to a record, link and unlink, a filtered reference, cloning |
| Temas | The token editor, changing the whole application at once |
| Sem armazenamento | A form with no storage behind it |
| Estados e lembretes | A state machine, and a reminder that comes up 30 seconds after opening |
| Segurança | The same documents as each of the three people |
| Auditoria | Each mode side by side, a field left out, changes in collections |
| Acessibilidade | Names and descriptions for screen readers from the model, the focus order of a form, an action that says its keys and announces what it did |
| Registros prontos | `ravenrs-app-library` |
| Relatórios | Reports people design and keep, and panels |
| Administração | Users, roles, the audit trail and the model editor, for `ana` |

## It is also the test

`tests/tour.rs` goes through all of this, as each of the three people, in memory and in SQLite. A new feature of the framework gets a demonstration here and a place in that tour.
