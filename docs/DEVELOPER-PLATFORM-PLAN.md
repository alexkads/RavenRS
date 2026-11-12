# Plataforma de desenvolvimento RavenRS — inventário e proposta

Data da revisão: 12/11/2026. Base local: `5fa5019c4`, workspace `27.1.14`, incluindo os arquivos modificados e exemplos ainda não versionados presentes na revisão.

**Status: proposta de arquitetura e sequência de trabalho. Nenhum comando, crate ou suporte mobile proposto neste documento foi implementado.**

**Atualização após consulta ao GitHub:** o `master` remoto já está em 27.1.27 e inclui `cargo-ravenrs`, `create-ravenrs`, `ravenrs-live`, `ravenrs-live-derive`, `ravenrs-app-i18n` e `ravenrs-app-a11y`, ausentes neste checkout. As issues #205, #208, #209 e #218 registram entregas e pendências das ferramentas. Portanto, o inventário abaixo é histórico do checkout local: a CLI deve ser ampliada, e o preview deve considerar o intérprete existente. Antes de implementar, reconciliar este plano com o `master` atual e preservar `.ravenrs-project.toml`.

## 1. Conclusão

**Direção vigente, após refinamento do escopo:** Android em Kotlin e iOS em Swift, com interfaces nativas, são a estratégia do produto. O monorepo gerado pelo `npx create-ravenrs` integra API Rust, web e desktop RavenRS e esses aplicativos nativos por contratos e serviços. O compartilhamento principal está no domínio do servidor, dados, regras, permissões, auditoria e recursos do ecossistema. As telas mobile são específicas de cada plataforma. Bindings Rust locais só entram por necessidade demonstrada. Criar runtime universal, renderer mobile ou camada própria de UI multiplataforma está fora da [issue #245](https://github.com/alexkads/RavenRS/issues/245). As propostas anteriores de prova do renderer Android/iOS, preservadas abaixo como histórico, não são etapas obrigatórias nem entregas desta iniciativa.

**Escopo atualizado pelo autor:** gerar um monorepo com API, web, desktop e mobile pelo NPX existente. Android pode começar em Kotlin e iOS em Swift, com UI nativa. Esses clientes aproveitam a API, os contratos e os serviços RavenRS; bindings para lógica Rust no aparelho são opcionais e dependem de prova técnica. Web/desktop reutilizam a UI RavenRS. A disponibilidade do renderer RavenRS no mobile não bloqueia a entrega inicial. Esta definição substitui a sequência anterior centrada na prova do renderer Android; ver [issue #245](https://github.com/alexkads/RavenRS/issues/245) para o escopo consolidado.

**Decisão confirmada pelo autor:** reaproveitar o CLI npm existente, com `npx create-ravenrs` (ou `npm create ravenrs@latest`) como porta de entrada, e ampliar o núcleo Rust `cargo-ravenrs` que ele já utiliza. Preservar prompts, preparação do ambiente, progresso, logs, cancelamento, templates, configuração de IA/MCP e compatibilidade de versões. Não criar pacote de CLI ou instalador concorrente. Essa decisão prevalece sobre as sugestões históricas de nomes e comandos abaixo.

A experiência inspirada no Expo deve ser uma evolução das ferramentas do RavenRS, integrada ao F15 (CLI e templates) e, depois, ao F16 (ambiente de projetos). O nome RavenKit da conversa é apenas uma sugestão; não estabelece um segundo framework ou uma nova linha de versões.

A recomendação atual é ampliar o gerador existente com a opção de monorepo e demonstrar um fluxo API + web + desktop + clientes Kotlin/Swift. O backend deve reutilizar o serviço de aplicação existente, após conferir seu estado atual, incluindo regras, permissões e persistência. Contratos de intercâmbio devem evitar cópias manuais entre linguagens. Renderização RavenRS no mobile fica como experimento posterior, com critérios próprios de entrada, ciclo de vida e acessibilidade.

O código existente continua definindo componentes, estado, layout, desenho, modelo de aplicação e dados. Não há justificativa identificada para substituir essas camadas.

## 2. Alcance e limites da revisão

- Inventário de manifests, features, dependências e tipos de targets de **todos os 44 pacotes locais: 33 crates e 11 exemplos**, usando `cargo metadata --no-deps --format-version 1`.
- Leitura da arquitetura, guias de qualidade e versões, roadmap, paridades de componentes/aplicação/relatórios, documentação de ferramentas e entradas recentes do changelog.
- Inspeção dos pontos de integração no código: fachada; eventos e runtime de UI; plataformas desktop/web; GPU; serviços de arquivos, clipboard e áudio; shell da aplicação; persistência de modelo; impressão/exportação; diagnóstico, captura de erros; ferramentas MCP; instalação e atualização do exemplo de finanças.
- Inspeção dos scripts locais de build, verificação e publicação, configuração de execução e presença dos exemplos novos de tutorial.
- As alterações locais preexistentes em widgets, finanças, testes e lockfile foram identificadas e preservadas. Este trabalho acrescenta somente este documento.

Esta é uma revisão arquitetural, não uma leitura linha a linha dos 449 arquivos Rust encontrados nem uma certificação de todos os comportamentos. A existência de código e testes não equivale à execução deles nesta revisão. Não foram executados builds, testes do workspace ou testes em dispositivos. A compatibilidade mobile continua não demonstrada.

## 3. Inventário completo dos crates e destino no plano

| Crates existentes | Papel atual | Encaminhamento |
|---|---|---|
| `ravenrs` | Fachada, prelude, seleção dos backends por target | Preservar imports existentes; revisar seleção de plataforma antes de mobile |
| `ravenrs-core` | Geometria, eventos, display list, semântica e diagnóstico | Contratos comuns; estender eventos apenas onde a prova mobile exigir |
| `ravenrs-reactive` | `State`, `Binding`, `Derived`, observadores | Manter estado na thread de UI e modelo atual de invalidação |
| `ravenrs-animation` | Springs, transições e interpolação | Reutilizar; medir em dispositivos |
| `ravenrs-layout` | Layout sobre Taffy | Reutilizar; integrar viewport útil, teclado e áreas seguras |
| `ravenrs-text` | Shaping, layout e fontes embarcadas | Reutilizar; completar integração de edição/IME nas plataformas |
| `ravenrs-style` | Tokens e temas | Fonte única de estilo; não criar outro design system |
| `ravenrs-render` | Renderização GPU sobre wgpu | Reutilizar; validar superfície, limites e consumo mobile |
| `ravenrs-widgets` | Componentes, `View`, `Component`, `BuildCx`, `Ui`, providers | Fonte única dos componentes e serviços já abstraídos |
| `ravenrs-graphics` | Vetores, gráficos e códigos de barras | Reutilizar onde necessário; não impor dependências de UI à CLI |
| `ravenrs-lottie` | Leitura e execução de animações | Reutilizar; sem promessa automática de desempenho mobile |
| `ravenrs-platform-native` | Janela, eventos e integrações desktop | Preservar comportamento desktop; decidir extração de partes comuns após prova |
| `ravenrs-platform-web` | Canvas, WebGPU, entrada web e ARIA | Base do fluxo web e previews no navegador |
| `ravenrs-wayland-gestures` | Ponte de gestos específica de Wayland | Manter isolada; não é uma camada de gestos mobile |
| `ravenrs-app` | Entidades, regras, ações, controllers, frames, segurança e modelo | Reutilizar como opção para apps de negócio; não exigir em todo projeto |
| `ravenrs-app-derive` | Derives de entidades e enums | Reutilizar nos templates de negócio |
| `ravenrs-app-ui` | Shell, listas e formulários gerados | Evoluir templates/layout existentes para mobile |
| `ravenrs-app-sqlite` | Persistência SQLite e migrações | Validar compilação e diretórios mobile; não criar outro ORM |
| `ravenrs-app-library` | Entidades prontas, inclusive tarefas | Reutilizar quando adequado; não recriar domínio em templates |
| `ravenrs-app-report` | Relatórios e painéis sobre entidades | Preservar integração com permissões e modelo |
| `ravenrs-criteria` | Expressões, validação, avaliação e SQL | Fonte única para regras e filtros existentes |
| `ravenrs-grid-export` | Exportação do grid | Reutilizar; adaptar entrega do arquivo ao host |
| `ravenrs-report` | Engine de relatórios e documentos | Reutilizar sem acoplar a ferramentas de desenvolvimento |
| `ravenrs-report-export` | Entrada comum de exportação | Reutilizar em todas as plataformas compatíveis |
| `ravenrs-report-pdf` | Exportação PDF | Reutilizar; exportar PDF não significa ler arquivos PDF |
| `ravenrs-report-xlsx` | Exportação XLSX/CSV | Reutilizar; validar targets e custo de memória |
| `ravenrs-report-viewer` | Exibição de documentos e impressão | Remover pressuposto desktop dos caminhos de impressão antes de mobile |
| `ravenrs-report-designer` | Designer e preview | Revisar abertura/exportação e dependência nativa antes de mobile |
| `ravenrs-report-server` | HTTP e entrega agendada de relatórios | Manter serviço específico; não usar como servidor de desenvolvimento genérico |
| `ravenrs-movie` | Gravação, reprodução, treinamento, flight recorder e bug reports | Base de captura/reprodução no ambiente de desenvolvimento |
| `ravenrs-ai` | Modelos locais, assistente e voz | Capacidade desktop opcional; excluir do template mínimo mobile/web |
| `ravenrs-mcp` | Consulta offline de API, tokens, exemplos e paridade | Preservar ferramenta somente leitura; revisar cobertura ao introduzir CLI |
| `ravenrs-perf` | Medições e gate de desempenho | Preservar cenários atuais; acrescentar medições mobile em equipamento declarado |

Os 11 exemplos inventariados são `hello-world`, `calculator`, `component-gallery`, `grid-poc`, `quick-app`, `quick-plan`, `feature-center`, `report-designer`, `movie-editor`, `family-finance` e `tutorial-tasks`.

`hello-world` é a referência de UI compartilhada; `quick-app` e `tutorial-tasks`, de templates pequenos; `feature-center`, de integração com o framework de aplicações; a galeria, de componentes. `family-finance` exercita instalação, SQLite, relatórios, atualização e recursos desktop, mas suas dependências tornam inadequado usá-lo como primeiro app mobile.

## 4. Sobreposições encontradas e decisões propostas

| Ideia inicial | Evidência existente | Decisão proposta |
|---|---|---|
| Nova CLI independente | F15 em `APP-PARITY.md` e CLI no roadmap do README | Implementar a experiência como entrega de F15 |
| Central gráfica de projetos | F16 já prevê instalador, Zed, extensão/skills e visualizador de projetos | Planejar como interface sobre os mesmos comandos; não exigir editor específico para a CLI |
| SDK que reúna UI, estado, dados e serviços | Fachada `ravenrs`, crates especializados e providers existentes | Adicionar apenas capacidades ausentes e adaptações de host |
| Outro sistema de navegação de negócio | `Frame`, `Session`, `AppShell`, `SystemModule` e `FrameTemplate` | Adaptar apresentação e ação de voltar; evitar nova máquina de navegação concorrente |
| Devtools e captura próprios | `Diagnostics`, `DiagnosticsHost`, `FrameSample`, `RecentErrors`, `FlightRecorder`, `BugReporter` | Acrescentar transporte e controles de desenvolvimento aos mecanismos atuais |
| Build web do zero | `scripts/build-web.sh`, profile `wasm-release` e exemplos com entradas web | Preservar resultado e semântica; generalizar para projetos externos ao monorepo |
| Novo atualizador genérico desde o começo | `family-finance/install.cmd`, `src/update.rs` e `src/paths.rs` | Aproveitar requisitos e experiência; manter atualização atual até existir substituição testada |
| Novo backend/API de dados | F12 já prevê API com permissões e cliente remoto | Tratar como frente própria; mobile local não depende de terminar F12 |
| Publicação automática em CI hospedada | `CONTRIBUTING.md`, `scripts/check.sh` e arquitetura adotam checks locais | Primeiro builds locais reproduzíveis; CI hospedada/nuvem exige decisão posterior |

F15 e F16 são itens planejados na documentação, não ferramentas já encontradas no workspace. Não foi encontrado pacote de CLI de projetos, cliente mobile, projeto Gradle/Xcode ou runtime de carregamento de apps.

## 5. Bloqueios técnicos confirmados no código

### Seleção de plataforma

`crates/ravenrs/Cargo.toml` e `src/lib.rs` tratam `not(target_arch = "wasm32")` como backend desktop. Android e iOS também passam nessa condição. As features `tray` e `notifications` encaminham recursos ao crate desktop.

Há o mesmo acoplamento nos manifests de `ravenrs-report-viewer` e `ravenrs-report-designer`. `print.rs` do viewer escreve HTML temporário e chama `open_with_system`; `preview.rs` do designer abre exportações pelo mesmo caminho. Só criar uma fachada mobile deixaria essas dependências transitivas no caminho.

Antes do suporte mobile, introduzir seleção explícita de hosts e adaptar esses serviços. A decisão entre ampliar `ravenrs-platform-native` e criar adaptadores Android/iOS fica condicionada à prova técnica; os nomes dos novos crates ainda não estão definidos.

### Entrada, teclado e ciclo de vida

`ravenrs-core/src/event.rs` já oferece pointer, scroll, teclado e pinch, mas não descreve identidade de múltiplos contatos, área ocupada pelo teclado ou composição IME como contratos próprios. O backend desktop não possui handlers de `WindowEvent::Touch`, `WindowEvent::Ime`, `suspended` ou `memory_warning` no código inspecionado.

O backend web já implementa toque com distinção entre toque e rolagem, cancelamento, captura de pointer e textarea para texto composto. Isso deve orientar a evolução comum; não é prova de suporte mobile nativo ou multitouch completo.

A prova Android deve receber a `AndroidApp` na construção do event loop e demonstrar destruição/recriação da superfície na suspensão/retomada. winit documenta que superfícies Android precisam ser descartadas antes do término do callback de suspensão. A integração também deve evitar versões conflitantes de `android-activity`, preferindo a API reexportada por winit.

Referências oficiais conferidas: [Android em winit](https://docs.rs/winit/0.30.13/winit/platform/android/), [ciclo de vida em ApplicationHandler](https://docs.rs/winit/0.30.13/winit/application/trait.ApplicationHandler.html), [iOS em winit](https://docs.rs/winit/0.30.13/winit/platform/ios/). O suporte da dependência não certifica o RavenRS.

### Serviços e dados

Já existem `ClipboardProvider`, `SoundProvider`, `FilePickerProvider` e pontos de instalação em `Ui`. `FilePickerProvider::pick` admite retorno posterior na thread de UI. Não criar interfaces concorrentes para essas mesmas funções.

Notificações são diferentes: `SystemNotifier` é um serviço desktop, sem um provider genérico equivalente identificado. Permissões, armazenamento seguro, câmera, deep links e compartilhamento precisam de contratos novos ou extensão justificada dos atuais. A CLI configura permissões de build; o host trata solicitações e recusas em execução.

`AppShell::on_file` e `AppShell::on_url` já são pontos de integração. Impressão, compartilhamento e abertura de documentos precisam respeitar o host. Diretórios do exemplo de finanças usam `HOME`/`USERPROFILE` e locais de instalação desktop; isso não é política de armazenamento mobile.

`SqliteDatabase`, `ModelStore`, `SqliteModelStore`, `AttachmentStore` e camadas de diferenças já resolvem problemas distintos de persistência. Reutilizá-los onde compatíveis. `raven.toml`, se adotado, configura desenvolvimento/empacotamento; não substitui os metadados de negócio nem vira um segundo schema de banco.

### Apresentação e acessibilidade

Já há `responsive`, viewport, layouts e `FrameTemplate`. O shell colapsa a sidebar abaixo de 720 pixels, mas isso não demonstra navegação, toque, teclado virtual e áreas seguras adequados ao celular. Templates mobile devem aproveitar esses contratos e preservar os templates existentes.

A árvore semântica `AccessTree` é reaproveitável. As pontes atuais foram construídas para desktop e web. TalkBack e VoiceOver em iOS precisam de integração e validação próprias; não assumir que a presença de AccessKit resolve isso automaticamente.

### Desenvolvimento e desempenho

Estado reativo usa `Rc`/`RefCell` na thread de UI. Servidor de desenvolvimento, compilação e transporte devem entregar resultados nessa thread pelos mecanismos de wake existentes, sem mover `State` entre threads.

Os limites desktop e o perfil de memória de relatórios, imagens, atlas e modelos locais não certificam desempenho em celular. Medir dispositivos reais, suspensão, pressão de memória e consumo ocioso. Manter a renderização sob demanda.

## 6. Correções necessárias nas premissas do plano inicial

- A CLI já tem lugar no roadmap: F15. Não é uma iniciativa sem antecedentes.
- Devtools não começam do zero. O código web já instala `DiagnosticsHost`, mede frames e habilita GPU timing quando solicitado; a seção de plataformas da arquitetura ainda diz que isso é apenas desktop.
- O roadmap de paridade contém linhas antigas: F9/F10 aparecem pendentes ao mesmo tempo que linhas anteriores e código registram editores e relatórios armazenados implementados. Tratar cada capacidade individualmente.
- O plugin de assistência ainda menciona exportação PDF/XLSX como planejada, apesar dos exporters existentes. Atualizar essas instruções quando consolidar documentação, preservando os doctests incluídos pela fachada.
- `VERSIONING.md` registra uma exceção que permite quebras até 28.1.0. Não presumir estabilidade apenas pela versão major 27; templates precisam de versões compatíveis e lockfile, e upgrades precisam ser explícitos.
- `ravenrs-mcp` embute fontes, mas sua consulta de paridade contempla componentes e reporting. Adicionar este plano não o torna automaticamente consultável pela ferramenta de paridade.

Essas divergências foram registradas, não corrigidas nesta etapa de planejamento, para não transformar a revisão em uma atualização ampla de documentação.

## 7. Arquitetura proposta para as ferramentas

O pacote npm `create-ravenrs` continua sendo a porta de entrada via NPX e delega ao núcleo Rust existente `cargo-ravenrs`. Evoluir essas implementações para coordenar criação, diagnóstico de ambiente, builds e execução. A interface gráfica futura chama esse mesmo núcleo. Aplicativos finais não dependem do compilador, servidor de desenvolvimento ou gerenciador de projetos.

Os três primeiros comandos abaixo já pertencem às ferramentas existentes. Os demais ilustram extensões propostas, com flags e disponibilidade ainda a especificar:

```text
npx create-ravenrs meu-app
cargo ravenrs new meu-app --template app
cargo ravenrs doctor
cargo ravenrs dev --platform desktop
cargo ravenrs dev --platform web
cargo ravenrs run --platform android
cargo ravenrs build --platform android --release
```

Manter projetos Cargo comuns, abertos em VS Code, Zed ou outro editor. Usar Cargo metadata e formatos estruturados de diagnóstico em vez de inferir nomes de binários ou depender do diretório `examples/`.

O script web atual calcula os caminhos a partir do monorepo, transforma o nome do pacote no nome da biblioteca e depende de `wasm-bindgen` exatamente compatível com o workspace. A CLI precisa localizar targets reais e versões; não basta copiar esse script para um projeto gerado. Fontes e demais recursos embarcados por `include_bytes!` continuam exigindo recompilação, salvo adoção explícita de recursos externos recarregáveis.

O arquivo de configuração proposto guarda apenas opções que Cargo não representa bem: identidade, recursos de empacotamento, opções de execução e permissões. Cargo continua sendo a fonte de dependências e targets. Versões do framework, ferramentas e protocolo de desenvolvimento precisam de verificação de compatibilidade.

Um servidor acessível por dispositivo deve ter pareamento e limite de acesso aos comandos de desenvolvimento. O código de transporte/servidor fica fora do build de produção. QR code, se houver, identifica uma sessão ou endereço; não elimina a compilação/instalação inicial.

## 8. Atualização durante desenvolvimento

| Mudança | Comportamento inicial proposto |
|---|---|
| Rust | Build incremental; substituir/reinstalar artefato bem-sucedido; reiniciar |
| Build com erro | Mostrar diagnóstico; preservar a versão funcional enquanto possível |
| Recurso embarcado | Recompilar como hoje |
| Tema/metadado externo com mecanismo de aplicação validado | Recarregar somente o recurso suportado |
| Dependência/manifest/permissão nativa | Regerar/recompilar o aplicativo e instalar novamente |
| Estado durante reinício | Restaurar somente estado declarado e persistido; memória reativa não sobrevive por padrão |

Um cliente de desenvolvimento próprio facilita logs e ferramentas, mas não carrega Rust nativo arbitrário sem nova compilação. O pipeline atual de `wasm-bindgen` depende de APIs do navegador e não é um contrato portátil para um runtime WASM embarcado. Um Raven Go universal exige experimento separado, interface host/guest e análise de distribuição; não é requisito para entregar F15 ou mobile nativo.

## 9. Sequência de entregas e critérios de saída

| Etapa | Escopo | Evidência necessária |
|---|---|---|
| A — evolução de F15 | Reaproveitar `npx create-ravenrs` e `cargo-ravenrs`; ampliar desenvolvimento/execução/build | Fluxo NPX preservado; projeto gerado fora do monorepo compila; app existente executa; web mantém saída equivalente ao script |
| B — prova Android, independente de A | UI mínima com renderer e widgets existentes, entrypoint, superfície e entrada | Executar em aparelho; editar texto composto; rolar; rotacionar; suspender/retomar repetidamente; demonstrar caminho de acessibilidade |
| C — base mobile | Seleção de plataforma, providers, permissões básicas, áreas seguras, teclado e navegação | Sem dependências desktop indevidas; dados sobrevivem a reinício; leitores de tela operam o fluxo essencial |
| D — ciclo de desenvolvimento Android | Rebuild, instalação, logs, diagnóstico e reinício pelo mesmo núcleo da CLI | Erro não destrói a última versão funcional; cliente desconecta/reconecta; build de produção sem servidor de desenvolvimento |
| E — aplicação de negócio mobile | Adaptar templates do `ravenrs-app-ui`, SQLite e entrega de exportações | Cadastro e edição no dispositivo sem duplicar regras, modelo ou persistência |
| F — iOS | Entrypoint e host próprios, recursos, assinatura e execução em aparelho | Mesmos fluxos essenciais e ciclo de vida validados, incluindo VoiceOver e teclado |
| G — F16 e distribuição ampliada | Interface gráfica sobre a CLI; empacotamento e documentação | CLI segue independente do editor; reproduzir build em ambiente limpo; assinatura documentada |
| H — experimentos posteriores | Preview universal, atualização de lógica, builds em nuvem | Protótipos e decisões próprias antes de prometer suporte |

Não estimar prazo fechado antes da prova Android: entrada/IME, acessibilidade, dependências nativas e ciclo de vida podem alterar o esforço de forma relevante. A definição de pronto para mobile é a conjunção dos critérios de C e D, não apenas desenhar uma tela.

## 10. Preservação e validação na implementação futura

- Não renomear APIs, formatos `.ravenmovie`/`.ravenbug`/relatórios, tokens ou modelo de aplicação para viabilizar a CLI.
- Separar templates mínimo, negócio e recursos opcionais. IA, relatórios e SQLite não devem entrar por obrigação em todo projeto.
- Se uma fronteira compartilhada exigir `unsafe`/FFI, justificar e isolar a exceção: o workspace usa `unsafe_code = "forbid"` e hoje só a ponte Wayland registra uma exceção. Não relaxar a regra global por conveniência.
- Mudanças de core, eventos, providers ou resolução de targets precisam preservar builds desktop e wasm e respeitar `docs/QUALITY.md`, `docs/VERSIONING.md` e `scripts/check.sh`.
- Medir mobile separadamente, com dispositivo e condições declarados; manter os budgets e cenários existentes como referência de regressão.
- Acrescentar testes de integração para templates externos, resolução de targets, erro de compilação e recuperação. Testar em aparelho o que testes headless não cobrem.
- Consolidar README, arquitetura, paridades, documentação do plugin e guias de instalação ao promover a proposta a implementação. Evitar publicar capacidade como pronta a partir deste documento.

Próxima unidade de trabalho recomendada: especificar o contrato mínimo de F15 e executar a prova técnica Android. A prova deve produzir uma decisão sobre divisão dos hosts e uma lista confirmada de dependências incompatíveis antes de criar uma família de novos crates.
