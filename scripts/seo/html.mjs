import { createRequire } from "node:module";
const require = createRequire(
  new URL("../../site/docs/package.json", import.meta.url),
);
const { parse, serialize, parseFragment } = await import(
  require.resolve("parse5")
);
export { parse, serialize, parseFragment };
export function walk(node, predicate, found = []) {
  if (predicate(node)) found.push(node);
  for (const child of node.childNodes || []) walk(child, predicate, found);
  return found;
}
export const attribute = (node, name) =>
  node?.attrs?.find((a) => a.name === name)?.value;
export function attr(node, name, value) {
  node.attrs ||= [];
  const current = node.attrs.find((a) => a.name === name);
  if (current) current.value = String(value);
  else node.attrs.push({ name, value: String(value) });
}
export const text = (node) =>
  node.nodeName === "#text"
    ? node.value
    : (node.childNodes || []).map(text).join("");
export function element(tagName, attrs = {}, content = "") {
  const node = {
    nodeName: tagName,
    tagName,
    namespaceURI: "http://www.w3.org/1999/xhtml",
    attrs: Object.entries(attrs).map(([name, value]) => ({
      name,
      value: String(value),
    })),
    childNodes: [],
  };
  if (content)
    node.childNodes.push({
      nodeName: "#text",
      value: content,
      parentNode: node,
    });
  return node;
}
export function append(parent, child) {
  child.parentNode = parent;
  parent.childNodes.push(child);
  return child;
}
export function remove(node) {
  const p = node.parentNode;
  if (p) p.childNodes = p.childNodes.filter((child) => child !== node);
}
export const meta = (head, key) =>
  walk(
    head,
    (n) =>
      n.tagName === "meta" &&
      (attribute(n, "name") === key || attribute(n, "property") === key),
  )[0];
export function setMeta(head, key, value, property = false) {
  const nodes = walk(
    head,
    (n) =>
      n.tagName === "meta" &&
      (attribute(n, "name") === key || attribute(n, "property") === key),
  );
  nodes.forEach(remove);
  append(
    head,
    element("meta", { [property ? "property" : "name"]: key, content: value }),
  );
}
export const plain = (value) => String(value).replace(/\s+/g, " ").trim();
export function pageRoute(file) {
  const path = file.replaceAll("\\", "/").replace(/^reference\//, "api/");
  return path === "index.html"
    ? ""
    : path.endsWith("/index.html")
      ? path.slice(0, -10)
      : path;
}
