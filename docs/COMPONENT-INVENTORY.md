# Inventário e projeção de componentes

Data: 15/11/2026. Base: `6b50ff874`.

## Critério reproduzível

A fonte da contagem atual é `site/public/components/index.html`, a mesma usada por `scripts/check-component-list.py`. Contar nomes únicos em links `data-api=...struct.Nome.html` resulta em **181 entradas**. O catálogo inclui primitivas, controles, composições e alguns tipos auxiliares; não equivale a 181 famílias independentes.

Não somar um tipo de gráfico adicional como novo widget quando a mesma API `Chart` passa a desenhá-lo. Não contar enums, eventos, providers, motores, formatos de arquivo ou issues como componentes visuais. Melhorias de virtualização, edição, exportação e validação preservam a identidade do componente.

## Projeção nominal das issues

| Base/entrega | Quantidade |
|---|---:|
| Entradas no catálogo atual | 181 |
| Novas experiências/componentes visuais candidatos identificados no backlog abaixo | 25 |
| Projeção conservadora usando o mesmo catálogo como base | **206** |

**206 é uma projeção, não uma contagem de APIs já disponíveis nem um total exaustivo do workspace.** Algumas experiências podem se tornar composição ou evolução de um tipo existente; outras serão desdobradas durante implementação. Nomes finais e entrada no catálogo dependem dos contratos públicos entregues. Por exemplo, o editor/visualizador de dashboards deve reutilizar DashboardView quando possível, em vez de criar tipos apenas para aumentar a contagem.

### Componentes/experiências candidatos

| Nome/capacidade | Issue |
|---|---|
| TreeGrid | [#401](https://github.com/alexkads/RavenRS/issues/401) |
| PropertyGrid | [#402](https://github.com/alexkads/RavenRS/issues/402) |
| PivotGrid | [#403](https://github.com/alexkads/RavenRS/issues/403) |
| PdfViewer | [#404](https://github.com/alexkads/RavenRS/issues/404) |
| DocumentEditor | [#405](https://github.com/alexkads/RavenRS/issues/405) |
| Gantt | [#406](https://github.com/alexkads/RavenRS/issues/406) |
| HeatMap | [#407](https://github.com/alexkads/RavenRS/issues/407) |
| ImageEditor | [#412](https://github.com/alexkads/RavenRS/issues/412) |
| Kanban | [#415](https://github.com/alexkads/RavenRS/issues/415) |
| Signature | [#416](https://github.com/alexkads/RavenRS/issues/416) |
| RangeSlider | [#417](https://github.com/alexkads/RavenRS/issues/417) |
| StockChart | [#426](https://github.com/alexkads/RavenRS/issues/426) |
| ListBox | [#448](https://github.com/alexkads/RavenRS/issues/448) |
| FloatingActionButton | [#449](https://github.com/alexkads/RavenRS/issues/449) |
| SpeedDial | [#450](https://github.com/alexkads/RavenRS/issues/450) |
| Treemap | [#69](https://github.com/alexkads/RavenRS/issues/69) |
| Map | [#170](https://github.com/alexkads/RavenRS/issues/170) |
| AppPlayground | [#153](https://github.com/alexkads/RavenRS/issues/153) |
| Editor de dashboards | [#203](https://github.com/alexkads/RavenRS/issues/203) |
| TileBoard | [#203](https://github.com/alexkads/RavenRS/issues/203) |
| Designer visual de apps | [#437](https://github.com/alexkads/RavenRS/issues/437) |
| Player de mídia | [#265](https://github.com/alexkads/RavenRS/issues/265) |
| WorkCalendarEditor | [#461](https://github.com/alexkads/RavenRS/issues/461) |
| TimePhasedGrid | [#469](https://github.com/alexkads/RavenRS/issues/469) |
| ActivityNetwork | [#470](https://github.com/alexkads/RavenRS/issues/470) |

A projeção inclui as três novas composições de planejamento: WorkCalendarEditor, TimePhasedGrid e ActivityNetwork. Biblioteca de calendários, CPM, nivelamento, baseline, custos/EVM, módulos de aplicação e importadores permanecem fora da contagem visual.

## Recursos atuais fora do catálogo

O workspace já contém componentes em outros crates que a lista de ravenrs-widgets não contabiliza: Spreadsheet, AppShell/ListView/DetailView e outras peças do App Framework, Designer/DocumentViewer de relatórios e MoviePlayer/MovieEditor, entre outros. Não somar automaticamente todos os structs com impl View, porque alguns são detalhes de implementação ou partes de outro componente.

Por isso, a resposta comparável é **181 catalogados hoje e aproximadamente 206 após as entregas nomeadas**. O total global pode ser maior, mas exige padronizar o que é componente independente entre os crates antes de publicar um número exato.

## Reprodução da base atual

```python
from pathlib import Path
import re
page = Path("site/public/components/index.html").read_text()
names = set(re.findall(r'data-api="(?:\w+/)*struct\.(\w+)\.html"', page))
print(len(names))  # 181 no commit auditado
```

## Atualização

Recontar a partir do catálogo a cada entrega. Transferir candidato para disponível somente após API, documentação e exemplo existirem; excluir duplicação quando uma issue for resolvida por composição já catalogada. A referência de planejamento é [#457](https://github.com/alexkads/RavenRS/issues/457) e seu [roadmap](PROJECT-PLANNING-ROADMAP.md).
