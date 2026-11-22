---
name: ravenrs-site
description: Como montar, testar e publicar o site do RavenRS (home em canvas WebGL2, páginas de componentes, docs, playground, API e SEO) - build local numa pasta do scratchpad, percurso da navegação com Playwright no desktop e em 375 px antes de publicar, SEO, GitHub Pages e Vercel sem Actions, e o que não mexer na home. Use sempre que o pedido citar o site, a home, a página de um componente, o playground no site, SEO, sitemap, GitHub Pages, Vercel, site/, examples/site-home, scripts/build-site.sh ou deploy, mesmo que não cite esta skill.
---

# O site do RavenRS

O endereço canônico é **https://ravenrs.vercel.app**. O GitHub Pages (https://alexkads.github.io/RavenRS/) é um espelho, e cada página dele aponta a da Vercel como canônica. Não há GitHub Actions (memória `sem-github-actions`): tudo é montado e publicado nesta máquina.

## De onde vem cada parte

| Parte | Fonte |
|---|---|
| Home: um app RavenRS desenhado num canvas WebGL2 | `examples/site-home` (`web/` vai para a raiz do site) |
| Páginas de cada componente | `site/components/`, geradas por `scripts/gen-component-pages.py`; a lista é conferida por `scripts/check-component-list.py` |
| Docs | `site/docs/`, com o Markdown do repositório renderizado no build |
| Playground | `site/playground/`, sobre o build da galeria |
| SEO: metadados, sitemap, robots | `site/seo/`, `scripts/check-site-seo.mjs`, `scripts/test-site-seo.mjs`, `docs/SITE_SEO.md` |
| Demonstrações do Centro de Recursos | o modo `vitrine` e o `componentes` do `tools/raven-feature-center` (#587) |

## Montar e servir localmente

Monte sempre numa pasta do scratchpad, nunca no repositório:

```bash
./scripts/build-site.sh <scratchpad>/site
node scripts/check-site-seo.mjs <scratchpad>/site
node scripts/test-site-seo.mjs <scratchpad>/site
python3 -m http.server -d <scratchpad>/site 8098
```

`RAVENRS_SITE_URL` diz a origem do deploy (com `/RavenRS` no Pages). Para uma mudança só de metadados, `RAVENRS_SITE_ASSETS_FROM` reaproveita um build já publicado da mesma versão (veja `docs/SITE_SEO.md`).

## Antes de publicar: Playwright

Pedido do Alex em 12/11 (#284): **"Antes de publicar precisa testar a navegação do site com playwrite".** A home é um canvas e as outras páginas são HTML gerado; link, âncora, menu e cabeçalho quebram sem que teste de unidade perceba.

Percorra o site servido localmente, em **desktop e em 375 px**:

1. a home carrega o canvas sem erro no console, e o cabeçalho leva a cada seção;
2. as âncoras da home e das docs param no lugar certo;
3. no celular, o menu abre, leva às páginas e fecha;
4. a lista de componentes abre a página de alguns deles, e cada uma abre a amostra e o link da API;
5. as docs abrem e os links entre elas funcionam;
6. o playground abre um exemplo e roda.

Use o MCP do Playwright; se ele não conectar, use o pacote `playwright` pelo Node, com o roteiro no scratchpad. **Registre no PR** o que foi percorrido, nas duas larguras, e o que falhou. Sem esse registro, o PR que mexe no site não está pronto.

## Publicar

Publicar só quando o Alex pedir.

```bash
./scripts/deploy-pages.sh        # monta e publica na branch gh-pages
./scripts/deploy-vercel.sh --prod
```

Depois do deploy, confira o HTML inicial, o status das imagens e os PNG, como diz o `docs/SITE_SEO.md`. Mudança só de metadados não pede versão nova no npm nem no crates.io.

## O que não mexer

- **A home WebGL2 fica.** O redesenho (#284, rascunho #341) melhora o comportamento dela no canvas: não a troque por HTML nem tire peças boas, como o leque de relatórios (memória `site-home-webgl-fica`).
- Redesenho de tela passa antes pelo Claude Design (memória `design-pelo-claude-design`).
- Nada público cita uma suíte comercial como modelo (memória `nao-expor-referencia`).

## Limpar

O build do site, o servidor local e o roteiro do Playwright são desta sessão: pare o servidor e apague a pasta ao terminar.
