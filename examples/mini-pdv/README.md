# Mini-PDV

> **BETA, PROVA DE CONCEITO. NÃO USE EM LOJA DE VERDADE.**
> Roda contra o sandbox da SEFAZ que fica dentro do próprio programa: nenhuma nota sai do computador e nenhuma tem valor fiscal. Nada aqui foi homologado com a SEFAZ nem impresso em impressora física. Pagamento com cartão ainda não está ligado a este caixa.

Um caixa de supermercado feito só com crates do RavenRS. Este exemplo não tem regra de negócio: ele junta `ravenrs-app-pos` e `ravenrs-app-nfe`, traz alguns produtos e guarda tudo em SQLite. O que falta para virar produto está em [docs/AUTOMACAO-COMERCIAL.md](../../docs/AUTOMACAO-COMERCIAL.md).

## Como rodar

```bash
cargo run -p mini-pdv
```

Os dados ficam em `~/mini-pdv-beta.sqlite`, ou no arquivo indicado em `RAVEN_PDV`. Apague o arquivo para começar de novo. O exemplo só compila no desktop, porque depende de SQLite. Os testes rodam no macOS; Windows e Linux ainda não foram tentados.

## O que testar, por tecla

Tudo funciona sem mouse. As teclas ficam sempre na barra de baixo, e F1 lista as da tela em que você está.

### Em qualquer tela

| Tecla | O que faz |
|---|---|
| F1 | Consulta de atalhos |
| Esc | Volta |
| F12 | Bloqueia a tela. O PIN fica em Configuração do caixa |

### Na tela inicial

| Tecla | O que faz |
|---|---|
| F2 | Lançar vendas |
| F3 | Produtos |
| F4 | Etiquetas |
| F5 | Outras funções: notas e NFS-e emitidas, serviços, emitente, modelos de cupom e etiqueta, clientes e configuração |
| F6 | Entrada de mercadoria |
| F7 | Formação de preço |
| F8 | Vender serviço, com NFS-e do Padrão Nacional no sandbox da Sefin. Uma DPS sem resposta certa fica pendente e vai de novo igual, com o mesmo número |

### No caixa

O campo de leitura fica sempre com o foco. Um leitor de código de barras funciona como um teclado que digita rápido e aperta Enter.

| Tecla | O que faz |
|---|---|
| Digitar ou bipar e Enter | Adiciona o produto. Experimente `7891000100103` |
| `3*7891000100103` e Enter | Adiciona 3 unidades |
| F6 | Cancela o último item |
| F8 | Liga a nota grande: NF-e modelo 55 para um cliente cadastrado |
| F9 | Vai para o pagamento e, nele, confirma |
| F10 | Guarda a venda como pendente ou retoma uma guardada |
| F11 | Cancela a venda |
| Alt+C | Informa o CPF ou CNPJ do cliente |
| Ctrl+O | Observação do documento, que vai para a nota |

### Roteiro curto

1. **Venda simples:** F2, bipe dois produtos, F9, confirme. Deve sair uma NFC-e autorizada pelo sandbox, com o aviso de sem valor fiscal no cupom.
2. **Quantidade:** `3*7891000100103` deve somar 3 itens de uma vez, com o total certo em centavos.
3. **Código errado:** digite `123` e Enter. Deve dizer que não encontrou, sem adicionar nada.
4. **Venda pendente:** bipe um item, F10 para guardar, volte depois e F10 para retomar.
5. **Nota grande:** com um cliente cadastrado, F8, Alt+C com o CNPJ `11444777000161`, e F9. Deve sair um modelo 55.
6. **Rejeição e serviço fora do ar:** use o painel de cenários do caixa para forçar rejeição, duplicidade e paralisação. O caixa deve dizer o motivo e deixar tentar de novo, sem perder a venda.
7. **Bloqueio:** F12, digite um PIN errado e depois o certo. Nada do que estava na tela pode se perder.
8. **Etiquetas:** F4, escolha produtos e quantidades e veja a prévia. Sem impressora, grave em arquivo `.prn` e abra o PNG da prévia.
9. **Cupom no editor:** F5, modelos de cupom e etiqueta, abra um modelo em Desenhar e use "Impressora direta". Se tirar a chave ou o QR Code do layout, o caixa recusa imprimir com ele.

## O que foi verificado

- `cargo test -p mini-pdv`: venda do leitor até a nota, rejeição, duplicidade, paralisação, bloqueio e nota grande, sem janela.
- `RAVEN_PDV_TELAS=pasta cargo test -p mini-pdv --test telas -- --ignored` desenha as telas em PNG, para olhar sem abrir a janela.

Falta testar com leitor físico, impressora física, certificado A1 e servidor real da SEFAZ.

Licença: MIT ou Apache-2.0.
