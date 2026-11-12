# Two applications, one package

Two small applications of different sectors install the same `ravenrs-app-requests` package, each with its own entities, steps, roles and rules, and neither imports the other. Scenario C19 of [docs/APP-CONTRACTS.md](../../docs/APP-CONTRACTS.md).

| | Associação Exemplo | Centro de Cursos Exemplo |
|---|---|---|
| What is requested | a purchase | a late enrollment |
| Steps | council check, then board approval | coordination only |
| Rule of its own | whoever asked does not decide | a request with no campus is not decided |
| Entity of its own | `Fornecedor` | `Curso` |

Each folder is a workspace of its own, outside RavenRS's, as an application of someone else would be:

```bash
cd examples/contracts/associacao && cargo run && cargo test
```

```bash
cd examples/contracts/cursos && cargo run && cargo test
```

They depend on the crates by path while the package is not published; then the paths become versions.
