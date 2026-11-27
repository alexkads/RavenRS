# FlowCanvas

Nodes joined by connections on an endless surface: pan, zoom, ports that check what connects, a map and keys for everything.

**Family:** Data · **Rust:** `ravenrs::prelude::FlowCanvas`

## Settings

| Setting | Control | Default | What it does |
|---|---|---|---|
| `cables` | on/off | `false` |  |
| `controls` | on/off | `true` |  |
| `editable_titles` | on/off | `true` |  |
| `grid` | number 0–96 by 1 | `24` |  |
| `grow` | number 0–4 by 0.1 | `1` |  |
| `width` | number 0–2000 by 1 | `880` | 0 takes the width of the parent. |
| `height` | number 80–1200 by 1 | `368` |  |
| `label` | text | `` | Empty leaves it as the component has it. |
| `hint` | text | `` | What a click does, after the zoom; empty shows none. |
| `locked` | on/off | `false` |  |
| `minimap` | on/off | `true` |  |
| `ports_on_hover` | on/off | `false` |  |
| `quick_create` | on/off | `true` |  |
| `read_only` | on/off | `false` |  |
| `resizable` | on/off | `true` |  |
| `rotatable` | on/off | `true` |  |
| `snap` | on/off | `true` |  |
| `status_icons` | on/off | `false` |  |
| `run` | on/off | `false` | A run told from outside, with execution. |
| `json` | on/off | `false` | How the example starts. |

## Stories

- **Example**: 879 × 420 pt, settings `{"cables":false,"controls":true,"editable_titles":true,"grid":24.0,"grow":1.0,"height":368.0,"hint":"","json":false,"label":"","locked":false,"minimap":true,"ports_on_hover":false,"quick_create":true,"read_only":false,"resizable":true,"rotatable":true,"run":false,"snap":true,"status_icons":false,"width":880.0}`
- **Read only**: 879 × 368 pt, settings `{"cables":false,"controls":true,"editable_titles":true,"grid":24.0,"grow":1.0,"height":368.0,"hint":"","json":false,"label":"","locked":false,"minimap":true,"ports_on_hover":false,"quick_create":true,"read_only":true,"resizable":true,"rotatable":true,"run":false,"snap":true,"status_icons":false,"width":880.0}`
- **Stage strip**: 439 × 198 pt, settings `{"cables":false,"controls":true,"editable_titles":true,"grid":24.0,"grow":1.0,"height":198.0,"hint":"Click a stage to see only its log","json":false,"label":"","locked":false,"minimap":false,"ports_on_hover":false,"quick_create":true,"read_only":true,"resizable":true,"rotatable":true,"run":true,"snap":true,"status_icons":true,"width":0.0}`
- **Run with status icons**: 879 × 368 pt, settings `{"cables":false,"controls":true,"editable_titles":true,"grid":24.0,"grow":1.0,"height":368.0,"hint":"","json":false,"label":"","locked":false,"minimap":false,"ports_on_hover":false,"quick_create":true,"read_only":true,"resizable":true,"rotatable":true,"run":true,"snap":true,"status_icons":true,"width":880.0}`
- **Json**: 879 × 632 pt, settings `{"cables":false,"controls":true,"editable_titles":true,"grid":24.0,"grow":1.0,"height":368.0,"hint":"","json":true,"label":"","locked":false,"minimap":true,"ports_on_hover":false,"quick_create":true,"read_only":false,"resizable":true,"rotatable":true,"run":false,"snap":true,"status_icons":false,"width":880.0}`
- **Cables**: 879 × 420 pt, settings `{"cables":true,"controls":true,"editable_titles":true,"grid":24.0,"grow":1.0,"height":368.0,"hint":"","json":false,"label":"","locked":false,"minimap":true,"ports_on_hover":false,"quick_create":true,"read_only":false,"resizable":true,"rotatable":true,"run":false,"snap":true,"status_icons":false,"width":880.0}`
- **Cards and a selection**: 864 × 420 pt, settings `{"cables":false,"controls":true,"editable_titles":true,"grid":24.0,"grow":1.0,"height":368.0,"hint":"","json":false,"label":"","locked":false,"minimap":true,"ports_on_hover":false,"quick_create":true,"read_only":false,"resizable":true,"rotatable":true,"run":false,"snap":true,"status_icons":false,"width":880.0}`
- **Stage strip in a row**: 1146 × 198 pt, settings `{"cables":false,"controls":true,"editable_titles":true,"grid":24.0,"grow":1.0,"height":368.0,"hint":"","json":false,"label":"","locked":false,"minimap":true,"ports_on_hover":false,"quick_create":true,"read_only":false,"resizable":true,"rotatable":true,"run":false,"snap":true,"status_icons":false,"width":880.0}`

## Sizing

Hugs its content: 879 × 420 pt in a parent of 760 pt.

## Interaction

The pointer over the first control, the focus brought to it with Tab and the button held down on it, measured in Raven Dark on the story Example:

- **Hover** on "Show JSON": fill transparent → ink@7.
- **Focus** on "Show JSON": adds a rect (border 2.0 focus-ring).
- **Pressed** on "Show JSON": fill transparent → ink@12, rect [212.0,0.0,146.0,40.0] → [214.19,0.6,141.62,38.8].

## Behavior

- **Events**: `on_change`, `on_export`.
- **Keys** its first control handles once focused: `Enter`, `Space`.
- **Accessibility**: role `Button` "Undo".
- **Focus order** (Tab): Button "Show JSON" → Group "Flow diagram" → Button "New sale" → Button "out" → Button "Check stock" → Button "in" → Button "out" → Button "Enough in stock?" → Button "in" → Button "yes" → Button "no" → Button "Issue receipt" → Button "in" → Button "Tell the manager" → Button "in" → Button "Zoom in" → Button "Zoom out" → Button "Fit" → Button "Arrange" → Button "Lock editing".

## Anatomy (Raven Dark, every story)

Measured from what RavenRS paints, in points. Colors are tokens; `token@N` is the token at N% opacity. Parts that measure the same are listed once, with the story they come from.

| Story | Part | Size | Radius | Fill | Border | Text | Icon | Left padding |
|---|---|---|---|---|---|---|---|---|
| Example | Button "Undo" | 99.0 × 40.0 | radius-md | bg-300@45 | 1.0 border | Inter 600 15.0px ink@45 | undo-2 16.0px ink@45 | 18.0 |
| Example | Button "Redo" | 97.0 × 40.0 | radius-md | bg-300@45 | 1.0 border | Inter 600 15.0px ink@45 | redo-2 16.0px ink@45 | 18.0 |
| Example | Button "Show JSON" | 146.0 × 40.0 | radius-md | transparent | — | Inter 600 15.0px ink | code 16.0px ink | 18.0 |
| Example | Button "New sale" | 134.0 × 56.0 | 6.67 | bg-200 | 1.0 border | Inter 600 8.0px ink-muted | zap 12.0px warning | 14.0 |
| Example | Button "out" | 24.0 × 24.0 | — | — | — | — | — | — |
| Example | Button "Check stock" | 134.0 × 56.0 | 6.67 | bg-200 | 1.0 border | Inter 600 8.0px ink-muted | package 12.0px accent | 14.0 |
| Example | Button "Enough in stock?" | 134.0 × 56.0 | 6.67 | bg-200 | 1.0 border | Inter 600 8.0px ink-muted | git-branch 12.0px success | 14.0 |
| Example | Button "Issue receipt" | 134.0 × 56.0 | 6.67 | bg-200 | 1.0 border | Inter 600 8.0px ink-muted | package 12.0px accent | 14.0 |
| Example | Button "Tell the manager" | 134.0 × 56.0 | 6.67 | bg-200 | 2.0 warning | Inter 600 8.0px ink-muted | package 12.0px accent | 14.0 |
| Example | Label "Yes" | 23.0 × 16.0 | — | — | — | Inter 600 12.0px ink | — | 0.0 |
| Example | Toolbar "Diagram view" | 223.0 × 40.0 | radius-md | bg-100 | 1.0 border | Inter 400 13.0px ink-muted | plus 14.0px ink | 13.0 |
| Example | Button "Zoom in" | 32.0 × 32.0 | radius-md | transparent | — | — | plus 14.0px ink | 9.0 |
| Example | Button "Zoom out" | 32.0 × 32.0 | radius-md | transparent | — | — | minus 14.0px ink | 9.0 |
| Example | Button "Fit" | 32.0 × 32.0 | radius-md | transparent | — | — | maximize-2 14.0px ink | 9.0 |
| Example | Button "Arrange" | 32.0 × 32.0 | radius-md | transparent | — | — | layout-dashboard 14.0px ink | 9.0 |
| Example | Button "Lock editing" | 32.0 × 32.0 | radius-md | transparent | — | — | lock-open 14.0px ink | 9.0 |
| Example | Label "67%" | 29.0 × 18.0 | — | — | — | Inter 400 13.0px ink-muted | — | 0.0 |
| Example | Slider "Navigator" | 200.0 × 132.0 | radius-md | bg-100 | 1.0 border | — | — | — |
| Read only | Button "Arrange" | 32.0 × 32.0 | radius-md | transparent | — | — | layout-dashboard 14.0px ink@45 | 9.0 |
| Stage strip | Button "New sale" | 56.0 × 24.0 | 2.8 | bg-200 | 2.0 success | — | — | — |
| Stage strip | Button "Check stock" | 56.0 × 24.0 | 2.8 | bg-200 | 2.5 accent | — | — | — |
| Stage strip | Button "Enough in stock?" | 56.0 × 24.0 | 2.8 | bg-200 | 2.0 warning | — | — | — |
| Stage strip | Button "Issue receipt" | 56.0 × 24.0 | 2.8 | bg-200 | 1.0 border | — | — | — |
| Run with status icons | Button "New sale" | 134.0 × 56.0 | 6.67 | bg-200 | 2.0 success | Inter 600 8.0px ink-muted | circle-check 12.0px success | 14.0 |
| Run with status icons | Button "Check stock" | 134.0 × 56.0 | 6.67 | bg-200 | 2.5 accent | Inter 600 8.0px ink-muted | loader-circle 12.0px accent | 14.0 |
| Run with status icons | Button "Enough in stock?" | 134.0 × 56.0 | 6.67 | bg-200 | 2.0 warning | Inter 600 8.0px ink-muted | git-branch 12.0px success | 14.0 |
| Json | Button "Show JSON" | 146.0 × 40.0 | radius-md | accent@18 | 1.0 accent@55 | Inter 600 15.0px mix(ink, accent, 30) | code 16.0px mix(ink, accent, 30) | 18.0 |
| Json | Label "{ "edges": [ { "animated": true, "from": { "node": "sale", "port": "out" }, "id": "e1", "shape": "Curve", "to": { "node": "stock", "port": "in" } }, { "animated": false, "from": { "node": "stock", "port": "out" }, "id": "e2", "shape": "Curve", "to": { "node": "enough", "port": "in" } }, { "animated": false, "from": { "node": "enough", "port": "yes" }, "id": "e3", "label": "Yes", "shape": "Curve", "to": { "node": "invoice", "port": "in" } }, { "animated": false, "from": { "node": "enough", "port": "no" }, "id": "e4", "label": "No", "shape": "Step", "to": { "node": "warn", "port": "in" } } ], "format": 2, "nodes": [ { "height": 84.0, "id": "sale", "kind": "trigger", "ports": [ { "direction": "Output", "id": "out", "kind": "sale", "side": "Right" } ], "status": "Normal", "subtitle": "Registers 1 and 2", "title": "New sale", "width": 200.0, "x": 0.0, "y": 96.0 }, { "height": 84.0, "id": "stock", "kind": "action", "ports": [ { "direction": "Input", "id": "in", "kind": "sale", "side": "Left" }, { "direction": "Output", "id": "out", "kind": "sale", "side": "Right" } ], "status": "Normal", "subtitle": "For each item", "title": "Check stock", "width": 200.0, "x": 264.0, "y": 96.0 }, { "height": 84.0, "id": "enough", "kind": "condition", "ports": [ { "direction": "Input", "id": "in", "kind": "sale", "side": "Left" }, { "direction": "Output", "id": "yes", "kind": "sale", "side": "Bottom" }, { "direction": "Output", "id": "no", "kind": "shortage", "side": "Bottom" } ], "status": "Normal", "title": "Enough in stock?", "width": 200.0, "x": 528.0, "y": 96.0 }, { "height": 84.0, "id": "invoice", "kind": "action", "ports": [ { "direction": "Input", "id": "in", "kind": "sale", "side": "Top" } ], "status": "Normal", "title": "Issue receipt", "width": 200.0, "x": 360.0, "y": 312.0 }, { "height": 84.0, "id": "warn", "kind": "action", "ports": [ { "direction": "Input", "id": "in", "kind": "shortage", "side": "Top" } ], "status": { "Warning": "No chat chosen" }, "title": "Tell the manager", "width": 200.0, "x": 648.0, "y": 312.0 } ] }" | 791.0 × 2768.0 | — | — | — | Fira 500 12.0px ink | zap 12.0px warning | 0.0 |
| Json | Button "Copy" | 65.0 × 22.0 | — | — | — | — | — | — |
| Cards and a selection | Button "Planner" | 195.0 × 101.0 | 7.88 | bg-200 | 2.5 accent | Inter 600 11.81px ink | loader-circle 9.45px mix(ink, warning, 30) | 9.0 |
| Cards and a selection | Button "Builder" | 196.0 × 101.0 | 7.88 | bg-200 | 2.0 success | Inter 600 11.81px ink | check 9.45px mix(success, ink, 70) | 9.0 |
| Cards and a selection | Button "in" | 24.0 × 24.0 | — | — | — | Inter 400 10.24px ink-muted | — | 21.0 |
| Cards and a selection | Button "Reviewer" | 196.0 × 101.0 | 7.88 | bg-200 | 2.0 accent | Inter 600 11.81px ink | clock 9.45px mix(ink, ink-muted, 30) | 9.0 |
| Stage strip in a row | Heading "Build 12" | 64.0 × 24.0 | — | — | — | Inter 600 17.0px ink | — | 0.0 |
| Stage strip in a row | Button "Checkout" | 200.0 × 84.0 | radius-md | bg-200 | 2.0 success | Inter 600 15.0px ink | — | 60.0 |
| Stage strip in a row | Button "Package" | 200.0 × 84.0 | radius-md | bg-200 | 1.0 border | Inter 600 15.0px ink | — | 60.0 |
| Stage strip in a row | Button "Publish" | 200.0 × 84.0 | — | — | — | — | — | — |

## API

Builder methods: `arrivals`, `cables`, `checks`, `connection`, `content`, `controls`, `editable_titles`, `execution`, `grid`, `grow`, `height`, `hint`, `history`, `kind`, `label`, `layout`, `locked`, `minimap`, `new`, `next_node`, `notation`, `on_change`, `on_export`, `open_end`, `pan_with_wheel`, `ports_on_hover`, `quick_create`, `range`, `read_only`, `resizable`, `rotatable`, `runs`, `selection`, `size`, `snap`, `status_icons`, `validate`, `view`.

## Example

```rust
fn flow_canvas() -> impl View {
    let graph = State::new(
        FlowGraph::new()
            .node(
                FlowNode::new("sale", "trigger", "New sale", 0.0, 96.0)
                    .subtitle("Registers 1 and 2")
                    .port(FlowPort::output("out").kind("sale")),
            )
            .node(
                FlowNode::new("stock", "action", "Check stock", 264.0, 96.0)
                    .subtitle("For each item")
                    .port(FlowPort::input("in").kind("sale"))
                    .port(FlowPort::output("out").kind("sale")),
            )
            .node(
                FlowNode::new("enough", "condition", "Enough in stock?", 528.0, 96.0)
                    .port(FlowPort::input("in").kind("sale"))
                    .port(FlowPort::output("yes").side(PortSide::Bottom).kind("sale"))
                    .port(FlowPort::output("no").side(PortSide::Bottom).kind("shortage")),
            )
            .node(
                FlowNode::new("invoice", "action", "Issue receipt", 360.0, 312.0)
                    .port(FlowPort::input("in").side(PortSide::Top).kind("sale")),
            )
            .node(
                FlowNode::new("warn", "action", "Tell the manager", 648.0, 312.0)
                    .status(FlowNodeStatus::Warning("No chat chosen".into()))
                    .port(FlowPort::input("in").side(PortSide::Top).kind("shortage")),
            )
            .edge(
                FlowEdge::new("e1", FlowEndpoint::new("sale", "out"), FlowEndpoint::new("stock", "in"))
                    .animated(true),
            )
            .edge(FlowEdge::new("e2", FlowEndpoint::new("stock", "out"), FlowEndpoint::new("enough", "in")))
            .edge(
                FlowEdge::new("e3", FlowEndpoint::new("enough", "yes"), FlowEndpoint::new("invoice", "in"))
                    .label("Yes"),
            )
            .edge(
                FlowEdge::new("e4", FlowEndpoint::new("enough", "no"), FlowEndpoint::new("warn", "in"))
                    .shape(EdgeShape::Step)
                    .label("No"),
            ),
    );
    let history = State::new(FlowHistory::new());
    let json = State::new(false);
    let (undo, redo) = (history.clone(), history.clone());
    let (shown, written) = (json.clone(), graph.clone());
    let canvas = FlowCanvas::new(graph.binding())
        .history(history.binding())
        .kind("trigger", FlowKind::new(IconName::Zap, "Trigger").tone(Semantic::Warning))
        .kind("action", FlowKind::new(IconName::Package, "Action"))
        .kind("condition", FlowKind::new(IconName::GitBranch, "Condition").tone(Semantic::Success))
        .validate(|graph, from, to| {
            let carries = |end: &FlowEndpoint| graph.port(end).and_then(|port| port.kind.clone());
            if carries(from) == carries(to) {
                Ok(())
            } else {
                Err(String::from("This input takes something else"))
            }
        })
        .size(880.0, 368.0);
    VStack::new((
        view_fn(move || {
            // Undo and redo here go through the same history as Cmd+Z in the canvas.
            let (back, again) = (undo.clone(), redo.clone());
            let (g1, g2) = (written.clone(), written.clone());
            let toggle = shown.clone();
            HStack::new((
                Button::new("Undo")
                    .style(ButtonStyle::Secondary)
                    .icon(IconName::Undo2)
                    .disabled(!undo.get().can_undo())
                    .on_click(move || {
                        let mut h = back.get();
                        if let Some(before) = h.undo(g1.get()) {
                            back.set(h);
                            g1.set(before);
                        }
                    }),
                Button::new("Redo")
                    .style(ButtonStyle::Secondary)
                    .icon(IconName::Redo2)
                    .disabled(!redo.get().can_redo())
                    .on_click(move || {
                        let mut h = again.get();
                        if let Some(after) = h.redo(g2.get()) {
                            again.set(h);
                            g2.set(after);
                        }
                    }),
                Button::new("Show JSON")
                    .style(ButtonStyle::Ghost)
                    .icon(IconName::Code)
                    .selected(shown.get())
                    .on_click(move || toggle.update(|on| *on = !*on)),
            ))
            .spacing(8.0)
        }),
        // The diagram as it is now, as the app would save it; `FlowGraph::from_json` reads it back.
        view_fn(move || {
            json.get().then(|| ScrollView::new((CodeBlock::new(graph.get().to_json()),)).height(200.0))
        }),
        canvas,
    ))
    .spacing(12.0)
    .align(Align::Stretch)
}
```

Every number above is in `FlowCanvas.json` beside this card.
