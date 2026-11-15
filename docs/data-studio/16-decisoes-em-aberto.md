# Raven Data Studio: decisões em aberto

Cada decisão tem uma recomendação, para não travar a implementação. Fica em aberto até alguém confirmar.

| Id | Decisão | Opções | Recomendação | Impacto se mudar |
|---|---|---|---|---|
| D-01 | Primeiro banco relacional do MVP | PostgreSQL; SQLite; os dois | **PostgreSQL** como destino e origem; SQLite logo depois, só como origem | Só o adaptador muda; domínio e telas não |
| D-02 | Nome e divisão dos crates | `ravenrs-datastudio` (+ `-connectors`, `-engine`, `-agent`, `-app`); um crate com módulos | **Crates separados**, como em [08-arquitetura.md](08-arquitetura.md), pela regra de camadas | Reorganização barata antes do primeiro código; cara depois |
| D-03 | A IA local consegue gerar `PlanPatch` válido? | Medir antes; confiar e ajustar depois | **Medir antes**: protótipo descartável com o pedido de referência em 2 ou 3 modelos do catálogo, contando patches válidos em 20 rodadas | Se não der, o MVP sai com heurísticas e edição manual, e a IA vira Evolução ou passa a ser remota por escolha explícita |
| D-04 | Canvas no MVP? | Não; canvas só leitura; canvas completo | **Não no MVP**; grade primeiro. Canvas na Evolução, depois de E-1 e N-1 | Antecipar o canvas antecipa a #238 inteira |
| D-05 | Decimal exato no SQLite | `TEXT` exato; `REAL` | **`TEXT`** com conversão no adaptador | `REAL` perde centavos acima de 15 dígitos |
| D-06 | Retenção de amostras e rejeitados | 7, 30 ou 90 dias; até apagar | **30 dias** depois de concluir, configurável por projeto | Política de privacidade |
| D-07 | Web | Só desktop; web com servidor | **Desktop no MVP**; web na Evolução, com motor e conectores num servidor | Exige o motor sem interface estável (já previsto) |
| D-08 | Tabelas de controle no destino (`_raven_import_map`, `_raven_import_checkpoint`) | Sempre; nunca; opcional | **Opcional, ligada por padrão**; desligada vira "idempotência por conferência", mais lenta | Sem elas, a retomada é mais frágil |
| D-09 | Desenho de alta fidelidade antes de implementar | Claude Design com o design system; direto em código | **Claude Design primeiro**, com uma página de interações por gesto, e aprovação antes do código. **Resolvida:** desenho feito e aprovado em 15/11/2026, no [Raven Data Studio](https://claude.ai/artifact/QT6Fc1Mp5CSJrXRK4VLM9g) | Desenho direto em código costuma fugir do design system |
| D-10 | Onde ficam `SchemaDiagram`, `TextDiff` e `IssueList` | Famílias propostas em [04-lacunas-e-novos-componentes.md](04-lacunas-e-novos-componentes.md); outras famílias | **Como proposto** (charts, chat, kit); conferir com o check de camadas antes | Família errada cria dependência entre famílias |
| D-11 | Separação de funções (quem aprova não executa) | Ligada; desligada; por projeto | **Por projeto, desligada por padrão** | Equipes maiores ligam |
| D-12 | Provedor de IA remoto | Nunca; por escolha explícita | **Por escolha explícita**, na Evolução, com a política "só estrutura e perfil" | Só o agente muda |
| D-13 | Destino gerido por um app RavenRS com chave `MAX + 1` | Proibir; avisar; coordenar | **Avisar** na Revisão final que outro processo não pode criar registros durante a execução | Coordenar exige a #241 ou chaves por sequência |
| D-14 | Mescla de duplicados: o sobrevivente | Mais antigo; mais completo; manual | **Mais antigo**, com troca por grupo (pendência) | Muda só a regra `SurvivorRule` |
| D-15 | Issues no GitHub para o roadmap | Criar agora; criar quando a implementação for pedida | **Quando pedida**, uma por entrega do MVP, com o padrão completo de descrição | — |

## Hipóteses que dependem de confirmação

As hipóteses H-01 a H-05 estão em [01-visao-e-requisitos.md](01-visao-e-requisitos.md#hipóteses-registradas). A H-02 (IA local dá conta) é a mais arriscada e está ligada à D-03.
