import { defineConfig } from "astro/config";
import starlight from "@astrojs/starlight";
import { unified } from "@astrojs/markdown-remark";
import { resolve } from "node:path";
import { root, pages, prepareContent } from "./src/lib/content.mjs";
import { base, siteUrl, publicPath } from "./src/lib/site.mjs";
import remarkLinks from "./src/lib/remark-links.mjs";

await prepareContent();

export default defineConfig({
  site: siteUrl,
  publicDir: ".generated/public",
  base: base || "/",
  trailingSlash: "always",
  markdown: { processor: unified({ remarkPlugins: [remarkLinks] }) },
  integrations: [
    {
      name: "ravenrs-markdown-sources",
      hooks: {
        "astro:server:setup": ({ server }) => {
          const sources = new Set(pages.map(({ source }) => resolve(root, source)));
          server.watcher.add([...sources]);
          let pending = Promise.resolve();
          server.watcher.on("change", (path) => {
            if (sources.has(path)) {
              pending = pending.then(() => prepareContent({ clean: false })).catch(console.error);
            }
          });
        },
      },
    },
    starlight({
      title: "RavenRS",
      description: "One declarative Rust UI for native desktop and WebAssembly applications.",
      social: [{ icon: "github", label: "GitHub", href: "https://github.com/alexkads/RavenRS" }],
      customCss: ["./src/styles/custom.css"],
      components: { PageTitle: "./src/components/PageTitle.astro" },
      disable404Route: true,
      sidebar: [
        { label: "Home", link: publicPath() },
        ...[...new Set(pages.map((p) => p.group))].map((label) => ({
          label,
          collapsed: !["Start here", "Build applications"].includes(label),
          items: pages.filter((p) => p.group === label).map(({ slug, title }) => ({ slug: slug.replace(/\/$/, ""), label: title })),
        })),
        { label: "Explore", items: [
          { label: "Components", link: publicPath("components/") },
          { label: "Playground", link: publicPath("playground/") },
          { label: "Rust API", link: publicPath("api/ravenrs/") },
        ] },
      ],
    }),
  ],
});
