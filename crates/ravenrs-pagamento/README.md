# ravenrs-pagamento

> **BETA, PROOF OF CONCEPT. Never tested against a real provider, pinpad or card. No homologation or certificate from anyone. Do not take real payments with it.**

Electronic payments for a Brazilian point of sale, with no GUI. It does not know `ravenrs-app` or `ravenrs-nfe`, and the core compiles for `wasm32`.

- `Pagamentos`: the one door. A state machine, a journal written before each effect, and `recuperar()` for what a power loss left open.
- `ProvedorPagamento`: one flow for all providers, with questions for the operator as `Passo`.
- `simulador`: a provider in the style of MSW, deterministic, with ready scenarios.
- `abecs`: the ABECS pinpad protocol as a sans-IO codec, a simulated pinpad, and a real port behind the `serial` feature. No PIN and no card capture, on purpose.
- `intpos`: PayGo Windows and the standard manager through files, with no `unsafe`.
- `mercado_pago`: Point through the Orders API, behind a transport trait (`http` feature for `ureq`).
- `auxiliar` and `sitef`: the JSON-lines contract for a helper process that will hold the native libraries outside this workspace. The helper with FFI does not exist yet.
- `stone`: fiscal data of Connect 2.0, and AutoTEF, which is not confirmed with Stone.

```rust
use ravenrs_pagamento::simulador::SimuladorProvedor;
use ravenrs_pagamento::{
    DiarioMemoria, Estado, IdPagamento, Impressao, Pagamentos, Passo, PedidoPagamento, TipoPagamento,
};

# fn main() -> Result<(), Box<dyn std::error::Error>> {
let mut pagamentos = Pagamentos::new(SimuladorProvedor::new(), DiarioMemoria::new())?;
pagamentos.recuperar()?;

let id = IdPagamento::new("venda-1")?;
let pedido = PedidoPagamento::new(id.clone(), 1990, TipoPagamento::Debito)?;
if let Passo::Aprovado(aprovacao) = pagamentos.iniciar(pedido)? {
    // Issue the fiscal document with aprovacao.dados_fiscais and print aprovacao.vias, then:
    assert_eq!(pagamentos.concluir(&id, Impressao::Concluida)?, Estado::Confirmado);
}
# Ok(())
# }
```

An absent `cAut` stays absent and comes with an `AlertaFiscal`: nothing here invents a fiscal value.

Sources, what each provider requires, and what this proves and does not prove: `docs/PAGAMENTOS.md` in the repository, in Portuguese.
