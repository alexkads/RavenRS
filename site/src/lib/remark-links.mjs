import { posix } from "node:path";
import { visit } from "unist-util-visit";
import { pages } from "./content.mjs";
import { publicPath } from "./site.mjs";

const routes = new Map(pages.map(({ source, slug }) => [source, `${slug.replace(/\/$/, "")}/`]));
const text = (node) => node.value ?? node.children?.map(text).join("") ?? "";
const slugify = (value) => value.normalize("NFD").replace(/[\u0300-\u036f]/g, "").toLowerCase().replace(/<[^>]+>/g, "").replace(/&[^;]+;/g, "").replace(/[^a-z0-9]+/g, "-").replace(/^-|-$/g, "");

// Resolve links against the original Markdown location, including reference-style links.
export default function remarkLinks() {
  return (tree, file) => {
    const source = file.data.astro?.frontmatter?.source;
    if (!source) return;
    const used = new Map();
    visit(tree, (node) => {
      if (node.type === "heading") {
        // Keep the anchors shared by the previous documentation renderer.
        const id = slugify(text(node));
        const count = used.get(id) || 0;
        used.set(id, count + 1);
        node.data ??= {};
        node.data.hProperties = { ...node.data.hProperties, id: `${id}${count ? `-${count}` : ""}` };
      }
      // Rustdoc accepts flags such as rust,ignore; syntax highlighters expect rust.
      if (node.type === "code" && node.lang?.startsWith("rust,")) node.lang = "rust";
      if (!["link", "image", "definition"].includes(node.type) || !node.url || /^(?:[a-z][a-z\d+.-]*:|\/|#)/i.test(node.url)) return;
      const [, path, suffix = ""] = node.url.match(/^([^?#]*)(.*)$/);
      const target = posix.normalize(posix.join(posix.dirname(source), decodeURI(path)));
      if (routes.has(target)) node.url = publicPath(routes.get(target)) + suffix;
      else if (target.startsWith("docs/media/")) node.url = publicPath(`docs/_media/${target.slice(11)}`) + suffix;
      else node.url = `https://github.com/alexkads/RavenRS/blob/master/${target}${suffix}`;
    });
  };
}
