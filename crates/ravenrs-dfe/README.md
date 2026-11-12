# ravenrs-dfe

> **BETA, prova de conceito.** Peças comuns aos documentos fiscais eletrônicos do RavenRS. Não foi homologado por nenhum órgão.

O que a NF-e (`ravenrs-nfe`) e a NFS-e (`ravenrs-nfse`) compartilham fica aqui, para que nenhum motor copie código do outro. Nenhum módulo conhece leiaute: cada motor diz qual elemento assina, com qual hash, e o que uma rejeição significa.

| Módulo | O que faz |
|---|---|
| `xml` | Árvore pequena sobre `quick-xml`. Recusa DTD, então nenhuma entidade é definida e nenhum recurso externo é lido. |
| `c14n` | C14N inclusiva (`REC-xml-c14n-20010315`), conferida com `xmllint --c14n`. |
| `assinatura` | XMLDSig envelopada. `Algoritmo::NFE` (RSA-SHA1) para a NF-e; `Algoritmo::SHA256` para quem pedir. `verificar_envelopado` diz qual hash foi usado e só aceita os listados. Trait `Signer` para chave em memória, A1 ou token. |
| `transporte` | `Transport`, a única saída para a rede. `HttpRequest` com `Metodo` (POST, GET, HEAD) e `caminho()`. |
| `interceptacao` | A cadeia de handlers dos sandboxes, no estilo do MSW (`use_once`, `reset_handlers`, `OnUnhandled`), e `Rota` para APIs REST: `GET /nfse/{chave}`, comparada pelo fim do caminho. |
| `compactacao` | GZip + Base64, com limite de 20 MiB na leitura contra bomba de compressão. |
| `certificado_x509` | Leitura de certificado ICP-Brasil: CNPJ no `otherName`, validade, EKU, tamanho da chave. |
| `certificados` | Laboratório: AC de teste, e-CNPJ A1 (também em PFX), token A3 simulado com PIN e cada falha de certificado que a produção pode trazer. Tudo marcado `RAVENRS SANDBOX - SEM VALOR`. |
| `tempo` | Instantes em segundos, sem ler relógio, para testes determinísticos. |
| `http` (feature `http`, só nativo) | mTLS com A1, TLS sempre verificado, sem seguir redirecionamento. Recusa certificado do laboratório. |

As chaves em `fixtures/laboratorio/` são públicas de propósito e não valem nada. `scripts/gerar-chaves-laboratorio.sh` as regenera.

Compila em `wasm32` sem a feature `http`.
