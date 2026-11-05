# Fontes

O que cada formato usa e de onde veio. Três níveis: **confirmado** (lido no manual do fabricante ou da família da linguagem), **indício** (loja, fórum, cópia de terceiro ou comportamento do ACBr) e **lembrança** (conhecimento sem fonte aberta nesta pesquisa). Nada aqui foi testado em impressora física.

O ACBrETQ (LGPL, espelho `github.com/joaoduarte19/ACBr`) serviu só para conferir comportamento. Nenhum código foi copiado.

## Formatos

### ZPL II

| Item | Situação | Fonte |
|---|---|---|
| `^GFa,b,c,d,data`, `A` = hexadecimal ASCII, `b` igual a `c` em ASCII, `d` = bytes por linha, limite 99999 | confirmado | [Zebra, ^GF](https://docs.zebra.com/us/en/printers/software/zpl-pg/c-zpl-zpl-commands/r-zpl-gf.html) |
| `^PWa`, largura em pontos | confirmado | [Zebra, ^PW](https://docs.zebra.com/us/en/printers/software/zpl-pg/c-zpl-zpl-commands/r-zpl-pw.html) |
| `^PQq`, de 1 a 99.999.999 | confirmado | [Zebra, ^PQ](https://docs.zebra.com/us/en/printers/software/zpl-pg/c-zpl-zpl-commands/r-zpl-pq.html) |
| `~SD##`, de 00 a 30 | confirmado | [Zebra, ~SD](https://docs.zebra.com/us/en/printers/software/zpl-pg/c-zpl-zpl-commands/r-zpl-sd.html) |
| `^FO`, no exemplo do `^GF` | confirmado | mesma página do `^GF` |
| `^XA`, `^XZ` | indício | ACBrETQZplII |
| `^LL`, `^LH`, `^FS` | lembrança | |
| Preto é 1 | indício | a página do `^GF` não diz. O ACBr inverte o raster, que nele tem 1 = branco, antes de mandar `~DG` |

Etiqueta acima de 99999 bytes sai em várias faixas `^FO`/`^GF`, por causa do limite do manual.

### EPL2

| Item | Situação | Fonte |
|---|---|---|
| `GWp1,p2,p3,p4,DATA`, largura em bytes, altura em pontos, dados binários crus de `p3 x p4` bytes | confirmado | [EPL Programming Guide 14245L-001, p. 108, cópia da Zebra em servopack.de](https://www.servopack.de/support/zebra/EPL2_Manual.pdf) e [guia na Zebra](https://www.zebra.com/content/dam/support-dam/en/documentation/unrestricted/guide/software/epl2-pm-en.pdf) (este passou do limite de leitura) |
| `N` limpa a imagem, com LF antes; configuração vem antes do `N` | confirmado | mesmo guia |
| `Pp1` (1 a 65535), `qp1`, `Qp1,p2` (vão de 16 a 240 pontos a 203 dpi, 18 a 240 a 300 dpi), `Dp1` (0 a 15) | confirmado | mesmo guia |
| `R0,0` | indício | ACBrETQEpl2; o comando existe no guia, o valor é escolha nossa |
| **Branco é 1, preto é 0** | indício | o guia oficial **não diz**. [Dica no CodeProject](https://www.codeproject.com/Tips/667062/Print-Image-to-Zebra-Printer-using-EPL-Language) afirma 0 = queimado, 1 = branco, igual à orientação recebida. Um post antigo de fórum sugere o contrário, sem detalhe. **Primeira coisa a conferir no papel.** |

### TSPL

| Item | Situação | Fonte |
|---|---|---|
| `BITMAP X,Y,width,height,mode,data`, largura em bytes, altura em pontos, modo 0 = OVERWRITE | confirmado | [TSPL/TSPL2 Programming, TSC, 2014, p. 43-44, cópia em servopack.de](https://www.servopack.de/support/tsc/TSPL_TSPL2_Programming.pdf); o endereço da TSC que eu tinha respondeu 404 |
| **0 é preto**, primeira linha em cima, dados crus seguidos de CRLF | confirmado | a figura de resultado do exemplo da p. 44 só fecha assim (a seta preta sai dos bits 0 e as linhas 1 a 3 são o topo). Também [tutorial na Hackernoon](https://www.hackernoon.com/how-to-print-images-with-tspl-and-javascript) |
| `SIZE m mm,n mm`, `GAP m mm,n mm` (0 a 127 mm), `DENSITY n` (0 a 15), `PRINT m[,n]`, `REFERENCE x,y`, `CLS` | confirmado | mesmo manual |
| `DIRECTION` | não enviado | fica na memória da impressora; o sentido de saída depende da instalação do rolo |

### PPLA

| Item | Situação | Fonte |
|---|---|---|
| `<STX>Imbfnn…n`: `A` = RAM, `A` = 7 bits, `F` = HEX, nome até 16 caracteres | confirmado | [PPLA Programmer's Manual, Mercury Series, V1.00, 2016, p. 19, cópia em a-one.uz](https://a-one.uz/pdf/PPLA_Programmer_manual_MercurySeries.pdf) |
| Registros HEX: `80xx` + dados, `FFFF` no fim, repetição `0000FFxx` (não usada aqui) | confirmado | mesmo manual, Apêndice AA, p. 124 |
| `1Y11000yyyyxxxxn..n`, origem no canto inferior esquerdo | confirmado | mesmo manual, p. 73 |
| `<STX>L`, `D11` (menor ponto; o padrão de 203 dpi é `D22`), `Hxx` (00 a 20), `Qxxxx`, `E`, `<STX>xAGnome` | confirmado | mesmo manual, p. 26, 38, 46, 49, 52 |
| **1 é preto e o primeiro registro é a linha de baixo** | confirmado pela figura | na Fig. A6-6 (p. 74) os últimos registros, `FFFFFF00…00FFFFFF`, saem em cima e pretos; os primeiros saem embaixo |
| Discordância | a conferir | o ACBr (`BMP2HEX`) envia de cima para baixo, com 1 = preto no padrão. Seguimos o manual. Se a etiqueta sair de ponta-cabeça no papel, é aqui |

Uma linha PPLA conta seus bytes em dois dígitos, então acima de 255 bytes por linha (2040 pontos) o resultado é erro. O manual é da série Mercury; as outras Argox e a Elgin podem variar. O [fórum do ACBr](https://www.projetoacbr.com.br/forum/topic/55108-elgin-l42pro-n%C3%A3o-carrega-imagem-impress%C3%A3o-acbretq/) registra uma L42 Pro que não carregava imagem BMP/PCX em PPLA, PPLB e EPL2 por lá, e o caminho aqui é outro (HEX e `GW`), mas é um sinal para testar cedo.

### ESC/POS

A referência da Epson (`download4.epson.biz`, `support.epson.net`) recusa leitura automática com 403. O que vem dela foi lido só em trechos de busca, e o manual completo lido foi o da Bematech, no modo ESC/POS.

| Item | Situação | Fonte |
|---|---|---|
| `GS v 0 m xL xH yL yH d…` (`1D 76 30`), `m = 0` normal, largura em bytes, altura em pontos | confirmado | [MP-4200 TH Programmer's Manual, rev. 1.0, 2010, capítulos 3 e 4](https://www.bztech.com.br/arquivos/manual-programacao-bematech-mp-4200-th.pdf) e trecho da [referência Epson](https://download4.epson.biz/sec_pubs/pos/reference_en/escpos/gs_lv_0.html) |
| **1 imprime, 0 não imprime** | confirmado no manual Bematech | "Setting a bit to 1 prints a dot". Na Epson, só por fórum |
| Epson marca `GS v 0` como obsoleto e recomenda `GS ( L` | confirmado por trecho | mesma página da Epson. Mantive `GS v 0`: é o único dos dois nas listas do TM-T20II e da MP-4200 TH que li |
| Largura de 1 a 255 bytes e altura de 1 a 4607 pontos | trecho de busca, sem o modelo | mesma página da Epson. A tabela por modelo não abriu; o limite do TM-T20 fica sem confirmar |
| **Bloco de 1024 linhas** | escolha nossa | bem abaixo do limite, porque ele muda por modelo e o bloco precisa caber no buffer. Que os blocos saem sem emenda visível é lembrança: conferir no papel |
| `ESC a 1` centraliza o raster | confirmado no manual Bematech | "ESC a is affective on raster bit images", no capítulo ESC/Bema. Na Epson, lembrança |
| `ESC @` inicializa | confirmado | manual Bematech, nos dois modos |
| `GS V 66 n`: avança até o corte mais `n` e corta | confirmado | manual Bematech, cap. 4; [Epson](https://download4.epson.biz/sec_pubs/pos/reference_en/escpos/cutting_paper.html) por trecho, corte parcial. A unidade de `n` depende de `GS P`, então `n = 24` é só uma margem |
| Corte total `GS V 65 n` | confirmado na Epson, **fora da lista da MP-4200 TH** | a [TM-T20II](https://www.datona.cz/sites/default/files/support/TM-T20II_escape_sekvence.pdf) aceita `A` e `B` e a Epson dá 65 = total por trecho. O manual Bematech só lista `GS V 66 n` com avanço, e `GS V 0/48` e `1/49` sem avanço. Na MP-4200 TH, o corte total do layout precisa ser conferido no papel; o corte parcial e o do fim do cupom (66) estão nas duas |
| Gaveta `ESC p m t1 t2` | confirmado | manual Bematech e TM-T20II: `m` = pino (0 = pino 2), tempos em unidades de 2 ms. Os valores 25 e 250 (50 ms, 500 ms) são escolha nossa |
| Avanço `ESC d n` | confirmado | manual Bematech e TM-T20II |
| **Bipe: não há comando comum** | variante escolhida | **Usei a da Epson**, `ESC ( A 3 0 97 n c` (`1B 28 41 03 00 61 01 01`), da TM-T20II, que é para campainha externa opcional. A MP-4200 TH em ESC/POS tem outra forma, `ESC ( A 5 0 97 100 c t1 t2`. Como o comando diz o próprio tamanho, espero que ela o ignore, sem confirmação. `ESC B`, de impressoras compatíveis, é lembrança e não foi usado. Elgin i9: sem fonte. **O bipe pode ficar mudo fora da Epson com campainha** |
| 180 dpi | lembrança | modelos antigos, como a TM-T88IV. Um [revendedor](https://umd.com.au/itd/var/epson_tmt20.html) dá 180 para a TM-T20, contra os 203 do manual da Epson. Aceito para não recusar essas impressoras |
| Cópias | escolha nossa | não há comando: o cupom é reenviado. O limite de 99 só evita engano de digitação |
| `darkness` e `gap_mm` | ignorados | sem comando portátil para escuridão; bobina não tem vão |

### Largura da bobina

| Bobina | Pontos a 203 dpi | Largura da página | Situação |
|---|---|---|---|
| 80 mm | 576 (72 mm) | 204,3 pt (72,07 mm) | confirmado para a TM-T20II pelo [guia técnico](https://manualowl.com/m/Epson/TM-T20II/Manual/624190?page=22), por trecho. Manual Bematech: largura de impressão de 72 mm em papel de 80 mm no modo ESC/Bema, e 73,5 mm fixos em ESC/POS, onde 576 cabe |
| 58 mm | 384 (48 mm) | 136,2 pt (48,03 mm) | **parcial.** O manual Bematech dá 48 mm em papel de 58 mm, mas só no modo ESC/Bema: em ESC/POS ele diz que o papel fica fixo em 80 mm. A TM-T20II usa 52,5 mm (420 pontos) ou 47,3 mm (378 pontos) em 58 mm, e nesse segundo ajuste 384 passa 6 pontos, que a impressora descarta |

72 mm exatos dão 575 pontos, e não 576. O crate completa até o múltiplo de 8, centralizando, então funciona, com meio ponto de deslocamento no máximo. Para não haver ajuste, use as larguras da tabela. A folha da [Elgin i9](https://www.bztech.com.br/arquivos/folheto-impressora-nao-fiscal-elgin-i9.pdf) dá papel de 57,5 e 80 mm e 203 dpi, sem pontos por linha.

## Mínimos de código de barras

- **EAN/UPC, 0,264 mm: confirmado.** 80% do nominal de 0,33 mm, em [GS1 GSCN 16-354](https://www.gs1.org/docs/barcodes/GSCN_16-354_Remove_Ambiguity.pdf), [GS1 Suécia](https://gs1.se/en/?p=2524) e [GS1 Canadá](https://www.gs1ca.org/gs1ca-components/documents/EAN-UPC-Symbol-Reference.pdf). Li resumos e notas da GS1, não a tabela das General Specifications. A 203 dpi isso dá 3 pontos (0,375 mm), porque 2 pontos são 0,250 mm; a 300 dpi dá 4 pontos (0,339 mm).
- **Demais formatos, incluindo QR, 0,19 mm (7,5 mil): lembrança.** É o piso usual de leitores, não regra dos padrões. A 203 dpi dá 2 pontos.
- **ITF-14: lembrança, não aplicada.** O mínimo GS1 para caixas é bem maior (0,495 mm de memória). Com ele o ITF-14 não caberia nem em 60 x 40 mm. O código aplica só o piso de 0,19 mm: lê bem, mas não é uma etiqueta logística conforme.
- **Zona quieta:** usa `QUIET_LINEAR`, 10 módulos de cada lado. A GS1 Suécia pede 11 e 7 no EAN-13 e 7 no EAN-8; o de 11 à esquerda fica 1 módulo abaixo no EAN-13. Vale para toda a família de códigos do RavenRS, e por isso não mudei aqui.

## O que saiu no teste de leitura

`shelf_label` a 203 dpi, lido com `rxing` a partir do bitmap (`tests/reading.rs`):

| Código | 40 x 25 mm | 60 x 40 mm |
|---|---|---|
| EAN-13 | erro: tem 36,7 mm, precisa de 43,2 mm | lido |
| EAN-8 | lido | lido |
| UPC-A | erro: tem 36,7 mm, precisa de 43,2 mm | lido |
| Code 128 | lido | lido |
| ITF-14 | erro: tem 36,7 mm, precisa de 38,8 mm | lido |
| QR Code (conteúdo curto, `RAVENRS`) | lido | lido |

A 203 dpi, EAN-13 e UPC-A com 2 pontos dariam 0,250 mm, ou 75,8% do nominal, abaixo dos 80% da GS1. Por isso o erro em 40 x 25 mm.

## Predefinições

| Impressora | Lista | Situação |
|---|---|---|
| Elgin L42 Pro | ZPL, EPL/PPLB, PPLA | indício: [KaBuM](https://www.kabum.com.br/produto/404780/impressora-termica-de-etiquetas-elgin-l42-pro-full-usb-ethernet-e-serial-preta) e outras lojas. Não achei manual da Elgin. Um [PDF de atualização](https://m.media-amazon.com/images/G/32/FBAOnsite/Manual_de_atualizacao_da_Impressora_Elgin_L42_Pro.pdf) sugere que ZPL depende de firmware (não aberto) |
| Elgin L42 DT | ZPL, EPL, TSPL | indício: [KaBuM](https://www.kabum.com.br/produto/481286/impressora-termica-direta-de-etiquetas-elgin-l42dt-203dpi-usb-serial-nao-usa-ribon) lista os três, mas [outro anúncio](https://www.kabum.com.br/produto/302595/impressora-termica-elgin-46l42dtussap-usb-serial-203-dpi-preto-e-bivolt) só EPL e ZPL. TSPL pode depender do modelo |
| Zebra ZD220 | ZPL, EPL | confirmado: [folha da Zebra](https://www.zebra.com/content/dam/zebra_dam/en/spec-sheets/zd220-spec-sheet-en-us.pdf), "ZPL II; EPL 2; XML", 203 dpi. O pedido listava só ZPL; EPL entra porque a folha confirma, com ZPL na frente |
| TSC | TSPL | confirmado: manual TSPL da TSC |
| Argox | PPLA, PPLB | confirmado: [manuais Argox](https://www.manualsdir.com/manuals/754567/argox-x-series.html?page=116) listam PPLA e PPLB; as Argox também listam PPLZ, não incluída |
| Bematech LB-1000 | fora | sem fonte: o [guia rápido](https://www.bztech.com.br/arquivos/guia-rapido-bematech-lb-1000.pdf) não cita linguagem, e só o [fórum do ACBr](https://www.projetoacbr.com.br/forum/topic/11507-acbretq-bematech-lb-1000/) fala em firmware ZPL, EPL e DPL |

| Epson TM-T20 | ESC/POS | confirmado em parte: o manual do usuário dá 203 dpi, em [trecho de busca](https://www.manualslib.com/manual/555355/Epson-Tm-T20.html?page=18). ESC/POS e `GS v 0` neste modelo são lembrança; a confirmação é da T20II |
| Epson TM-T20II | ESC/POS | confirmado: [referência rápida ESC/POS](https://www.datona.cz/sites/default/files/support/TM-T20II_escape_sekvence.pdf), com `GS v 0` e `GS V m n` (cópia em site de terceiro) |
| Elgin i9 | ESC/POS | confirmado: [folheto da Elgin](https://www.bztech.com.br/arquivos/folheto-impressora-nao-fiscal-elgin-i9.pdf), "ESC/POS", 203 dpi. O folheto não lista `GS v 0`, então o raster é lembrança |
| Bematech MP-4200 TH | ESC/POS | confirmado: [manual de programação](https://www.bztech.com.br/arquivos/manual-programacao-bematech-mp-4200-th.pdf). Veja abaixo |

**MP-4200 TH:** tem modo próprio, e é preciso configurar. Só um entre ESC/Bema e ESC/POS fica ativo. `GS F9h 5 n` (`1D F9 35 n`) escolhe e grava, com `n = 1` para ESC/POS; `GS F9h SP n` (`1D F9 20 n`) troca sem gravar. O `GS v 0` existe nos dois, mas o `GS V` só está na lista do ESC/POS: no ESC/Bema o corte é `ESC i`, `ESC w` ou `ESC m`. O crate não envia a troca, porque em outra marca o comando viraria lixo; a predefinição traz o aviso em `setup`. A versão MP-4200 HS fica de fora: os anúncios discordam sobre ESC/Bema.

**Elgin e Bematech:** a Elgin comprou a unidade de hardware da Bematech da Totvs, em [maio de 2019](https://www.baguete.com.br/noticias/totvs-vende-bematech-para-elgin), sem o negócio de software. A [base de CNPJ](https://cnpj-api.com/en/cnpj/27101611000182) lista a Elgin S.A. como sócia da Bematech Hardware Ltda. e o folheto chama a i9 também de "Bematech i9 Full". As duas marcas são do mesmo grupo no hardware, mas o CNPJ da Bematech Hardware continua separado.

**PPLB = EPL: indício, não confirmado pela Argox.** O ACBrETQ usa a mesma implementação para os dois, com o comentário "EPL2 = PPLB". Não encontrei a referência de comandos PPLB da Argox, então `GW` em PPLB fica de lembrança.
