# ravenrs-perifericos

Periféricos de automação comercial como dados: impressoras térmicas, etiquetadoras, pinpads, leitores de código, balanças e gavetas. Cada driver é um manifesto JSON versionado que diz exatamente para qual modelo serve.

> **Aviso:** nenhum driver daqui foi testado em aparelho. O campo `situacao` diz isso em cada um, e a tela deve mostrar. Nenhuma atualização é publicada por este repositório: não há chave nem endereço reais.

```rust
use ravenrs_perifericos::{Registro, Version, verificar_atualizacoes};

let mut registro = Registro::novo(Version::new(27, 1, 13));
let driver = registro.ativo("bematech.mp-4200-th.cupom");

// A busca é de quem chama: endereço e chave são configuração.
let catalogo = registro.abrir_catalogo(&bytes, &chave, "2026-11-05")?;
for oferta in verificar_atualizacoes(&catalogo, &registro) {
    // Mostre `oferta`. Só depois que a pessoa aceitar:
    registro.aceitar(&catalogo, &oferta)?;
}
registro.voltar("bematech.mp-4200-th.cupom")?;
```

- **O driver é dado.** Um manifesto não carrega código, caminho nem endereço. Os comandos passam por uma lista fechada por tipo de aparelho.
- **Modelo exato é obrigatório.** "TSC" como modelo é recusado.
- **`situacao` honesta:** `testado_no_aparelho`, `so_pela_documentacao` ou `presumido`.
- **Atualização nunca é automática:** o catálogo oferece, a pessoa aceita e pode voltar.
- **Catálogo assinado** com ed25519. Assinatura errada, validade vencida, número menor que o último visto e manifesto inválido têm erro próprio.
- **Núcleo compila em wasm32.** `ArmazemArquivo` e `TransporteHttp` (feature `http`) são só nativos.

Como escrever, assinar e publicar um driver: [`docs/PERIFERICOS.md`](../../docs/PERIFERICOS.md). De onde veio cada dado: [`FONTES.md`](FONTES.md).

Licensed under MIT or Apache-2.0.
