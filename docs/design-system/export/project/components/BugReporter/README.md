# BugReporter

The dialog that turns the last minutes on screen into a bug report: its kind, what happened, a box over the problem, the film to watch and cover, and all that goes along.

**Family:** Application · **Rust:** `ravenrs::prelude::BugReporter`

## Settings

| Setting | Control | Default | What it does |
|---|---|---|---|
| `language` | pt-BR · en | `pt-BR` | The words, from `BugLabels`. |
| `kind` | Something is wrong · It is confusing · A suggestion | `Something is wrong` | What kind of report it is. |
| `open` | on/off | `true` | Open, or only the status bar action that opens it. |
| `include_film` | on/off | `true` | The film of the last minutes goes along. |
| `marked` | on/off | `false` | A box already drawn around the problem. |
| `errors` | on/off | `false` | The app's last errors, from `RecentErrors`. |

## Stories

- **Describing**: 716 × 1050 pt, settings `{"errors":false,"include_film":true,"kind":"Something is wrong","language":"pt-BR","marked":false,"open":true}`
- **It is confusing**: 716 × 1050 pt, settings `{"errors":false,"include_film":true,"kind":"It is confusing","language":"pt-BR","marked":false,"open":true}`
- **A suggestion**: 716 × 1050 pt, settings `{"errors":false,"include_film":true,"kind":"A suggestion","language":"pt-BR","marked":false,"open":true}`
- **Box marked**: 716 × 1050 pt, settings `{"errors":false,"include_film":true,"kind":"Something is wrong","language":"pt-BR","marked":true,"open":true}`
- **Without the film**: 716 × 1050 pt, settings `{"errors":false,"include_film":false,"kind":"Something is wrong","language":"pt-BR","marked":false,"open":true}`
- **With errors**: 716 × 1050 pt, settings `{"errors":true,"include_film":true,"kind":"Something is wrong","language":"pt-BR","marked":false,"open":true}`
- **English**: 716 × 1050 pt, settings `{"errors":false,"include_film":true,"kind":"Something is wrong","language":"en","marked":false,"open":true}`
- **Closed**: 185 × 29 pt, settings `{"errors":false,"include_film":true,"kind":"Something is wrong","language":"pt-BR","marked":false,"open":false}`

## Sizing

Hugs its content: 704 × 1032 pt in a parent of 760 pt; in a parent of 320 pt it narrows to 288 pt; its height follows the width, as its content wraps.

- **Closed**: Hugs its content: 185 × 29 pt in a parent of 760 pt; in a parent that stretches its children, as a form column, it takes the whole width.

## Interaction

The pointer over the first control, the focus brought to it with Tab and the button held down on it, measured in Raven Dark on the story Describing:

- **Hover** on "Close": fill transparent → ink@7.
- **Focus** on "Close": adds a rect (border 2.0 focus-ring); color ink → on-accent.
- **Pressed** on "Close": fill transparent → ink@12, rect [660.0,42.0,32.0,32.0] → [660.48,42.48,31.04,31.04].

## Behavior

- **Keys** its first control handles once focused: `ArrowDown`, `ArrowUp`, `ArrowLeft`, `ArrowRight`, `Escape`, `End`.
- **Accessibility**: role `Dialog` "Relatar um problema".
- **Focus order** (Tab): Button "Close" → RadioButton "Algo está errado" → RadioButton "Está confuso" → RadioButton "Uma sugestão" → TextInput → Button "Marcar o problema na tela" → Switch "Incluir um filme dos últimos minutos" → Button "Reproduzir" → Button "Reproduzir" → Button "Do início" → Slider "Posição" → Button "1×" → Switch "Parar em cada passo" → Switch "Som" → Switch "Narração" → Button "Cobrir ou cortar partes do filme" → Button "Cancelar".

## Anatomy (Raven Dark, every story)

Measured from what RavenRS paints, in points. Colors are tokens; `token@N` is the token at N% opacity. Parts that measure the same are listed once, with the story they come from.

| Story | Part | Size | Radius | Fill | Border | Text | Icon | Left padding |
|---|---|---|---|---|---|---|---|---|
| Describing | Dialog "Relatar um problema" | 704.0 × 1032.0 | radius-xl | bg-200 | 1.0 border | Inter 600 12.0px ink | circle-alert 12.0px ink | 36.0 |
| Describing | Heading "Relatar um problema" | 612.0 × 28.0 | — | — | — | Inter 600 22.0px ink | — | 0.0 |
| Describing | Button "Close" | 32.0 × 32.0 | radius-md | transparent | — | — | x 14.0px ink | 9.0 |
| Describing | Label "Conte à equipe o que aconteceu. Você vê tudo antes de sair daqui." | 656.0 × 22.0 | — | — | — | Inter 400 15.0px ink-muted | — | 0.0 |
| Describing | Label "O que é?" | 656.0 × 18.0 | — | — | — | Inter 400 13.0px ink | — | 0.0 |
| Describing | RadioButton "Algo está errado" | 132.0 × 30.0 | 7.0 | bg-200 | 1.0 border | Inter 600 13.0px ink | — | 14.0 |
| Describing | RadioButton "Está confuso" | 110.0 × 30.0 | — | — | — | Inter 600 13.0px ink-muted | — | 14.0 |
| Describing | TextInput | 656.0 × 40.0 | radius-md | bg-300 | 1.0 control-border | — | — | — |
| Describing | Button "Marcar o problema na tela" | 249.0 × 40.0 | radius-md | bg-300 | 1.0 border | Inter 600 15.0px ink | crosshair 16.0px ink | 18.0 |
| Describing | Switch "Incluir um filme dos últimos minutos" | 305.0 × 24.0 | — | — | — | Inter 400 15.0px ink | — | 50.0 |
| Describing | Label "versão: 0.1.32" | 656.0 × 16.0 | — | — | — | Inter 600 12.0px ink-muted | — | 0.0 |
| Describing | Label "O filme mostra o que estava na sua tela. Assista e cubra o que não deve ir." | 458.0 × 18.0 | — | — | — | Inter 400 13.0px ink-muted | — | 0.0 |
| Describing | Image "Filme" | 638.0 × 399.0 | 0.0 | on-accent | — | — | play 16.0px ink | 315.0 |
| Describing | Button "Reproduzir" | 40.0 × 40.0 | radius-md | bg-300 | 1.0 border | — | play 16.0px ink | space-3 |
| Describing | Button "Reproduzir" | 115.0 × 32.0 | radius-md | accent | — | Inter 600 13.0px on-accent | play 14.0px on-accent | space-3 |
| Describing | Button "Do início" | 101.0 × 32.0 | radius-md | transparent | — | Inter 600 13.0px ink | rotate-ccw 14.0px ink | space-3 |
| Describing | Slider "Posição" | 281.0 × 24.0 | — | — | — | — | — | — |
| Describing | Button "1×" | 39.0 × 32.0 | radius-md | transparent | — | Inter 600 13.0px ink | — | space-3 |
| Describing | Button "Cobrir ou cortar partes do filme" | 288.0 × 40.0 | radius-md | bg-300 | 1.0 border | Inter 600 15.0px ink | eye-off 16.0px ink | 18.0 |
| Describing | Button "Cancelar" | 101.0 × 40.0 | radius-md | bg-300 | 1.0 border | Inter 600 15.0px ink | — | 18.0 |
| Describing | Button "Enviar" | 82.0 × 40.0 | radius-md | accent@45 | — | Inter 600 15.0px on-accent@45 | — | 18.0 |
| Closed | Button "Relatar problema" | 137.0 × 22.0 | radius-md | transparent | — | Inter 600 12.0px ink | circle-alert 12.0px ink | space-2 |

Every number above is in `BugReporter.json` beside this card.
