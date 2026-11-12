# Fontes

Nenhum driver foi testado em aparelho. A situação de cada um segue o que foi lido, e não o que seria conveniente.

As impressoras e etiquetadoras vêm do levantamento em [`crates/ravenrs-report-label/FONTES.md`](../ravenrs-report-label/FONTES.md), onde estão as fontes de cada comando.

| Driver | Situação | Por quê |
|---|---|---|
| `epson.tm-t20.cupom` | presumido | só os 203 dpi vieram do manual do modelo |
| `epson.tm-t20ii.cupom` | documentação | referência rápida ESC/POS |
| `elgin.i9.cupom` | documentação | folheto: ESC/POS, 203 dpi, guilhotina, buzzer, QRCode. Sem o comando de raster nem o do buzzer |
| `bematech.mp-4200-th.cupom` | documentação | manual de programação |
| `elgin.l42-pro.etiqueta` | presumido | só anúncios de loja |
| `elgin.l42-dt.etiqueta` | presumido | anúncios que discordam |
| `zebra.zd220.etiqueta` | documentação | folha da Zebra |
| `tsc.ttp-244me.etiqueta` | documentação | lista de modelos do manual TSPL; conexões e resolução não vieram dele |
| `argox.x-2300.etiqueta` | documentação | manual da série X lista PPLA e PPLB; o formato de imagem foi lido no manual da série Mercury |
| `generico.hid-teclado.leitor` | presumido | descreve um modo de uso, não um aparelho conferido |
| `gertec.ppc930.pinpad` | documentação | ver abaixo |
| `ingenico.lane-3000.pinpad` | presumido | ver abaixo |

`TSC` e `Argox`, que eram predefinições só com a marca, viraram os modelos TSC TTP-244ME e Argox X-2300. Foram escolhidos porque aparecem nominalmente nos manuais; o catálogo não cobre as demais máquinas dessas marcas.

## MP-4200 TH

Tudo do [manual de programação, revisão 1.0, 2010](https://www.bztech.com.br/arquivos/manual-programacao-bematech-mp-4200-th.pdf), capítulo 4 (modo ESC/POS):

- `comando_modo = 1D F9 20 01`: `GS F9h SP n`, troca de modo **sem gravar**. O manual lista esse comando nos dois modos. A forma que grava (`GS F9h 5 n`) é proibida pela validação.
- `bipe = 1B 28 41 05 00 61 64 01 02 01`: `ESC ( A ENQ NUL a d c t1 t2`, um ciclo, 200 ms ligado e 100 ms desligado. Os tempos são escolha nossa.
- `corte_total = 1D 56 42 18`: o manual descreve `GS V m n` só com `m = 66`, como "avança e corta", sem dizer se é total ou parcial. `GS V 0` corta sem avançar, e assim cortaria antes do fim da impressão.

## Pinpads

`docs/PAGAMENTOS.md`, citado no pedido e pelo módulo `abecs` de `ravenrs-pagamento`, **não existe neste worktree**. Usei o que o próprio módulo diz e busquei os aparelhos.

| Item | Situação | Fonte |
|---|---|---|
| PPC930 lista ABECS nas certificações | só em trecho de busca | [folha da Gertec](https://www.gertec.com.br/wp-content/uploads/2025/08/PPC930-1.pdf). O PDF não abriu depois (404 no redirecionamento) |
| PPC930 USB e serial, ref. 70500035, V5 | loja | [KaBuM](https://www.kabum.com.br/produto/546960/pin-pad-gertec-ppc930-v5-usb-serial-70500035) |
| Pix por QR no PPC930 | só indício, **não declarado** | documentação da TOTVS, em trecho de busca; a página recusou leitura |
| Lane 3000 é da Ingenico, com USB, serial e tela | confirmado | [folha da Ingenico](https://cdn.shopify.com/s/files/1/0593/8426/6940/files/LANE-3000_datasheet_EN_OCT22.pdf) |
| ABECS no Lane 3000 | **presumido** | a folha não cita. Um manual da Vero lista o modelo como pinpad, mas não abriu para leitura |
| Versão ABECS 2.20 nos dois | **presumido** | é a versão que `ravenrs-pagamento` implementa. A [Cielo](https://docs.cielo.com.br/conecta/docs/conheca-a-biblioteca-do-pin-pad) fala em versões de 1.08 a 2.12 em campo, então o aparelho pode estar em outra |

Nenhum dos dois declara capacidade de transação: o manifesto nem tem esse campo. A transação com cartão fica com o software de TEF.

Balança e gaveta existem como tipo, sem driver no catálogo: não houve pesquisa de modelo.

## Dependências

`ed25519-dalek` (BSD-3-Clause) e `semver` (MIT ou Apache-2.0), aceitas pelo `deny.toml`.
