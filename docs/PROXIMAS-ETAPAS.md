# Automação comercial: próximas etapas

Ponto de partida para continuar este projeto. Situação em 2026-11-06. O branch é `worktree-nfe-beta-mini-pdv`, ainda sem push e sem merge no `master`.

O plano aprovado no início está em `~/.claude/plans/eu-quero-criar-um-jolly-whisper.md`, com os pedidos que chegaram depois. A finalidade é uma POC que reúna tudo para lançar um produto vendável de automação comercial para pequenas e médias empresas. Tudo fica nos crates do RavenRS, reaproveitável; `examples/mini-pdv` só compõe e traz dados de exemplo.

## Como retomar

```bash
cargo run -p mini-pdv
cargo test -p mini-pdv
RAVEN_PDV_TELAS=/tmp/telas cargo test -p mini-pdv --test telas -- --ignored
```

Leia antes: a skill `ravenrs-fiscal`, `crates/ravenrs-nfe/REGRAS.md`, `crates/ravenrs-dfe/README.md`, `crates/ravenrs-nfse/README.md`, `docs/AUTOMACAO-COMERCIAL.md`, `docs/PAGAMENTOS.md`, `docs/TEF-COMPARATIVO.md`, `docs/NFSE.md` e `docs/PERIFERICOS.md`.

O desenho das telas está em https://claude.ai/artifact/MpucDz6n1eyKo1DzcFseme.

## Feito em 2026-11-06

- Cartão e Pix no caixa pelo `ravenrs-pagamento`, com o simulador: confirmação só depois da nota autorizada, desfazimento quando ela é recusada, grupo `card` na NFC-e.
- Tela de periféricos e drivers na impressão: o cupom sai pelo `LabelJob::do_driver` do modelo configurado.
- "Buscar na SEFAZ" na entrada de mercadoria, com ciência nos resumos e o limitador do 656 guardado no emitente.
- `ravenrs-dfe`, o crate fiscal comum: XML, C14N, assinatura SHA-1 ou SHA-256, `Transport` com GET e HEAD, cadeia de handlers e rotas do sandbox, GZip + Base64, leitura e geração de certificados ICP-Brasil. A NF-e passou a usá-lo sem mudar a API.
- `ravenrs-nfse`, a NFS-e do Padrão Nacional no menor escopo de `docs/NFSE.md`, com recuperação pelo Id da DPS e o `SefinSandbox`. DANFSe A4 como relatório do editor padrão, em `ravenrs-app-nfe`.

## Etapas, nesta ordem

1. **Fechar a verificação.** Rodar `scripts/check.sh` inteiro, com `cargo deny`, e corrigir o que apontar. Medir com `ravenrs-desempenho` as telas novas. Levar o branch ao `master` e publicar quando o Alex pedir.
2. **NFS-e no caixa.** Cadastro de serviço (código de tributação nacional), venda de serviço emitindo pelo `ravenrs-nfse` em segundo plano, guardando a DPS antes de enviar para a recuperação depois de reabrir o programa, e DANFSe A4 impressa. Venda mista de produto e serviço só com a regra do contador.
3. **Entrada de mercadoria completa.** Confirmação da operação (210200) ou desconhecimento (210220) depois de conferir a mercadoria, com prazo à vista; fluxo de recompra sugerindo etiqueta com teste de ponta a ponta; acrescentar entrada, preços e periféricos ao desenho.
4. **Laboratório fiscal na tela.** Mostrar no app o diagnóstico do sandbox: linha do tempo, autópsia da assinatura, reprodução de incidente, relatório exportado e gerador de certificados. Vale para os dois sandboxes.
5. **Homologação real.** Com certificado A1 e contribuinte: NF-e (status, autorização, consulta, cancelamento) e NFS-e na produção restrita, que decide o algoritmo de assinatura, o prefixo da URL e os campos JSON. Registrar em `REGRAS.md` e `docs/NFSE.md` o que mudou.
6. **Teste em aparelho.** Compra sugerida: Gertec PPC930, uma etiquetadora Elgin L42, uma térmica Elgin i9 ou Epson e um leitor. Cada teste muda a `situacao` do driver para testado no aparelho.
7. **Windows e Linux.** O Alex testa, conforme a regra da casa. A maioria dos TEFs aponta para Windows nos caixas.
8. **O que falta para vender.** Lista em `docs/AUTOMACAO-COMERCIAL.md`: contingência, CRT 3 e IBS/CBS, fechamento de caixa, permissões do operador, balança, atualização, backup, instalação e suporte.

## O que depende do Alex

- Qual TEF contratar, com base em `docs/TEF-COMPARATIVO.md`.
- Certificado A1 e CNPJ para homologar.
- Aparelhos para teste.
- Contador para a troca de NFC-e por NF-e, a venda mista de produto e serviço e o documento impresso da NFS-e.

## O que nunca foi provado

Nenhuma nota falou com a SEFAZ real. Nada foi impresso em impressora física nem passou por pinpad real. Nada rodou em Windows ou Linux. A situação de cada item está na seção de pendências abaixo.

## Pendências desta entrega

Conhecidas ao fechar o branch:

- **TEF.** A recomendação em `docs/TEF-COMPARATIVO.md` é a PayGo por troca de arquivos no Windows, pedindo proposta do TEF Express na mesma rodada. A pesquisa foi encerrada antes do fim: faltam preços atuais, a recertificação por versão do PDV e a resposta da PayGo sobre CNPJ da credenciadora e bandeira. A seção 10 do documento lista o resto.
- **Exigência por UF.** RS multa quem vende sem integração, e SC pede homologação do PAF-NFC-e, conforme fonte secundária. Conferir antes de vender nessas UFs.
- **Pagamentos.** Ligados ao caixa, só com o simulador. O auxiliar real de SiTef e TEF Stone não existe, só o contrato. Falta o servidor de webhook e token de Mercado Pago e Connect. O diário do caixa é em memória: trocar pelo `DiarioArquivo` antes de vender, para a pendência sobreviver a uma queda.
- **Drivers.** O cupom usa o driver configurado; o editor e a tela de etiquetas aplicam os ajustes do modelo escolhido. Não existem chave nem URL do catálogo: `chave_embutida()` devolve `None`, e "Procurar atualizações" só avisa isso. Balança e gaveta são só tipos.
- **Entrada de mercadoria.** Importa XML pela tela ou busca na SEFAZ (`NfeService::buscar_compras`): distribuição por NSU, ciência (210210) nos resumos e download da nota completa, em segundo plano. O `ultNSU` e a hora de espera pedida pelo ambiente nacional ficam no emitente, então nem um emissor novo nem o programa reaberto repetem a consulta antes da hora (rejeição 656). Notas já em estoque, listadas ou abertas não reaparecem. Só testado no sandbox. A tela de entrada e a de preços têm testes de regra e fluxo, mas ainda não foram conferidas em imagem contra um desenho, que não as inclui.
- **Cupom em bobina.** Testado em bytes e leitura de código, sem papel. Conferir bipe e corte na MP-4200 TH.
- **Nota grande.** O DANFE A4 é um primeiro layout e não foi conferido contra o modelo do MOC. A troca de NFC-e por NF-e precisa da validação do contador.
- **Sandbox da SEFAZ.** O catálogo traz a fonte de cada regra; uma ainda é de memória e sete são inferidas.
- **NFS-e.** Motor, sandbox e DANFSe existem; o caixa ainda não emite. Os códigos E do `SefinSandbox` vêm de catálogo de terceiros, os nomes dos campos JSON do ACBr e do nfse-php, e a DPS não foi validada com `xmllint` contra o XSD oficial. O algoritmo de assinatura, o prefixo da URL, o DV da chave, o Id do pedido de evento e o endereço do QR Code não estão confirmados. O `SefinSandbox` ainda não tem o diagnóstico, a autópsia e a gravação de incidentes do sandbox da SEFAZ, que ficaram no `ravenrs-nfe` e podem subir para o `ravenrs-dfe`.
- **Verificação.** Em 2026-11-06, `scripts/check.sh` passou inteiro: formatação, clippy nativo e wasm, testes, rede local, features opcionais, MSRV, `cargo deny`, benchmarks, gate de desempenho (117 interações iguais ao baseline), documentação da API e builds web. A documentação só passou depois de corrigir dois links quebrados em `ravenrs-pagamento`.
