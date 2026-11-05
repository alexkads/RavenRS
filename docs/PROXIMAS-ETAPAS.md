# Automação comercial: próximas etapas

Ponto de partida para continuar este projeto. Situação em 2026-11-05. O branch é `worktree-nfe-beta-mini-pdv`, ainda sem push e sem merge no `master`.

O plano aprovado no início está em `~/.claude/plans/eu-quero-criar-um-jolly-whisper.md`, com os pedidos que chegaram depois. A finalidade é uma POC que reúna tudo para lançar um produto vendável de automação comercial para pequenas e médias empresas. Tudo fica nos crates do RavenRS, reaproveitável; `examples/mini-pdv` só compõe e traz dados de exemplo.

## Como retomar

```bash
cargo run -p mini-pdv
cargo test -p mini-pdv
RAVEN_PDV_TELAS=/tmp/telas cargo test -p mini-pdv --test telas -- --ignored
```

Leia antes: a skill `ravenrs-fiscal`, `crates/ravenrs-nfe/REGRAS.md`, `docs/AUTOMACAO-COMERCIAL.md`, `docs/PAGAMENTOS.md`, `docs/TEF-COMPARATIVO.md`, `docs/NFSE.md` e `docs/PERIFERICOS.md`.

O desenho das telas está em https://claude.ai/artifact/MpucDz6n1eyKo1DzcFseme.

## Etapas, nesta ordem

1. **Fechar a verificação.** Rodar `scripts/check.sh` inteiro, com `cargo deny`, e corrigir o que apontar. Medir com `ravenrs-desempenho` as telas e os componentes novos. Levar o branch ao `master` e publicar quando o Alex pedir.
2. **Entrada de mercadoria completa.** Botão "Buscar na SEFAZ" na tela de entrada, com distribuição de DF-e, ciência e manifestação. Fluxo de recompra sugerindo etiqueta com testes de ponta a ponta. Acrescentar entrada, preços e periféricos ao desenho.
3. **Pagamentos no caixa.** Ligar `ravenrs-pagamento` à tela de pagamento: máquina de estados, diário, pendência ao religar e confirmação ou desfazimento em volta da emissão. Levar `DadosFiscaisPagamento` ao grupo `card` da NFC-e. Primeiro com o simulador, depois com o provedor recomendado em `docs/TEF-COMPARATIVO.md`.
4. **Periféricos na tela.** Tela de periféricos sobre `ravenrs-perifericos`: dispositivo, modelo exato, versão do driver, situação de teste, atualizar e voltar. Substituir os campos de texto de impressora da configuração do caixa.
5. **Laboratório fiscal na tela.** Mostrar no app o diagnóstico do sandbox: linha do tempo, autópsia da assinatura, reprodução de incidente, relatório exportado e gerador de certificados.
6. **Crate fiscal comum e NFS-e.** Extrair `Transport`, `Signer`, C14N e o mecanismo de handlers do sandbox para um crate comum, sem cópia, com GET e HEAD e SHA-256. Criar `ravenrs-nfse` no menor escopo de `docs/NFSE.md`, com DANFSe A4 no editor de relatórios.
7. **Homologação real.** Com certificado A1 e contribuinte: status, autorização, consulta e cancelamento; registrar em `REGRAS.md` o que mudou. Conferir no MOC os textos de `cStat`, o arredondamento e o QR Code.
8. **Teste em aparelho.** Compra sugerida: Gertec PPC930, uma etiquetadora Elgin L42, uma térmica Elgin i9 ou Epson e um leitor. Cada teste muda a `situacao` do driver para testado no aparelho.
9. **Windows e Linux.** O Alex testa, conforme a regra da casa. A maioria dos TEFs aponta para Windows nos caixas.
10. **O que falta para vender.** Lista em `docs/AUTOMACAO-COMERCIAL.md`: contingência, CRT 3 e IBS/CBS, fechamento de caixa, permissões do operador, balança, atualização, backup, instalação e suporte.

## O que depende do Alex

- Qual TEF contratar, com base em `docs/TEF-COMPARATIVO.md`.
- Certificado A1 e CNPJ para homologar.
- Aparelhos para teste.
- Contador para a troca de NFC-e por NF-e, a venda mista de produto e serviço e o documento impresso da NFS-e.

## O que nunca foi provado

Nenhuma nota falou com a SEFAZ real. Nada foi impresso em impressora física nem passou por pinpad real. Nada rodou em Windows ou Linux. A situação de cada item está na seção de pendências abaixo.

## Pendências desta entrega

Atualizado ao fechar o branch.
