# ravenrs-app-pos

> **BETA, proof of concept.** It issues through `ravenrs-app-nfe`, which is not homologated with any SEFAZ and cannot issue in production: only the local sandbox and homologation, with no fiscal value. Card and Pix payments go through a simulated provider unless you give it one of your own.

A point of sale as a module for `ravenrs-app`. You add it to an application and get the records, the screens and the look of a shop counter: products with their barcodes, a checkout used from the keyboard and a scanner, sales kept to pick up later, goods arriving from suppliers, services sold with an NFS-e, shelf labels and receipts. The rules of a sale (totals, discounts, change) are in `ravenrs-pos`; the documents are issued by `ravenrs-app-nfe`.

## Using it

```rust,ignore
use ravenrs_app::App;
use ravenrs_app_nfe::NfeService;
use ravenrs_app_nfe::engine::BetaAck;
use ravenrs_app_pos::Pos;

let service = NfeService::sandbox(BetaAck::i_understand_this_is_experimental())?;
let pos = Pos::new(service);
let app = pos.instalar(App::new("Mini-PDV"));
```

`Pos::new` charges through a simulated payment provider, and its screens offer the simulator's scenarios. `Pos::com_tef` takes your own provider instead. `instalar` adds everything to the `App`: the entities of `entidades()`, which a database such as `ravenrs-app-sqlite` needs to know; the screens, with the window opening on the checkout's first screen; Brazilian Portuguese; and the light theme.

The `examples/mini-pdv` application in the repository is this module with demonstration data, stored in memory or in a SQLite file.

## The checkout

The checkout is made to be used without a mouse: the scanner or the keyboard adds an item, function keys move between the steps, and a sale left halfway is kept so the next customer can be served and the first one picked up later. A code that is not registered can be linked to a product on the spot. `Pos::caixa` reaches it from code, for tests and automation.

## Labels and receipts anyone can change

Shelf labels (60 × 40 mm), the receipts for 80 mm and 58 mm rolls, the DANFE and the DANFSe are stored reports, made in the standard report designer and changed there by whoever runs the shop, with no programming. They print to label and ESC/POS printers. A fiscal receipt still has to carry its key, QR Code, number and protocol, which `ravenrs-app-nfe` checks when printing.

## Limits

It is a proof of concept: the list of what is missing for a product that a shop could install and keep is in `docs/AUTOMACAO-COMERCIAL.md`, in the repository. The fiscal limits are those of `ravenrs-app-nfe`.

Licensed under MIT or Apache-2.0.
