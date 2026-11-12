const base = document.body.dataset.docsBase;
const dialog = document.querySelector("#search-dialog");
const input = document.querySelector("#docs-search");
const results = document.querySelector("#search-results");
const status = document.querySelector("#copy-status");
let pages;
let selected = -1;
const normalize = (value) =>
  value
    .normalize("NFD")
    .replace(/[\u0300-\u036f]/g, "")
    .toLowerCase();
const announce = (value) => {
  status.textContent = value;
};

async function copy(text, button, normal) {
  try {
    await navigator.clipboard.writeText(text);
    button.textContent = "Copied ✓";
    announce("Copied to clipboard");
    setTimeout(() => {
      button.textContent = normal;
    }, 1800);
  } catch {
    button.textContent = "Select text to copy";
    announce("Clipboard unavailable. Select and copy the text.");
    setTimeout(() => {
      button.textContent = normal;
    }, 2500);
  }
}
for (const button of document.querySelectorAll("[data-copy-code]")) {
  button.addEventListener("click", () =>
    copy(
      button.closest(".codeblock").querySelector("code").textContent,
      button,
      "Copy ⧉",
    ),
  );
}
document
  .querySelector("#copy-page")
  .addEventListener("click", async (event) => {
    const button = event.currentTarget;
    try {
      const response = await fetch("index.md");
      if (!response.ok) throw new Error("Page unavailable");
      await copy(await response.text(), button, "Copy page ⧉");
    } catch {
      announce("Unable to load Markdown. Use View Markdown below the page.");
    }
  });
const themeButton = document.querySelector("#theme-toggle");
themeButton.addEventListener("click", () => {
  const dark =
    document.documentElement.dataset.theme === "dark" ||
    (!document.documentElement.dataset.theme &&
      matchMedia("(prefers-color-scheme: dark)").matches);
  const theme = dark ? "light" : "dark";
  document.documentElement.dataset.theme = theme;
  themeButton.setAttribute(
    "aria-label",
    `Switch to ${dark ? "dark" : "light"} theme`,
  );
  try {
    localStorage.setItem("ravenrs-docs-theme", theme);
  } catch {}
});
function hint(message) {
  results.replaceChildren();
  const p = document.createElement("p");
  p.className = "search-hint";
  p.textContent = message;
  results.append(p);
}
async function loadSearch() {
  if (pages) return pages;
  const response = await fetch(`${base}search.json`);
  if (!response.ok) throw new Error("Search index unavailable");
  pages = await response.json();
  return pages;
}
function search() {
  selected = -1;
  if (!pages) return;
  const query = normalize(input.value.trim());
  if (!query) {
    hint("Search installation, components, forms, reports...");
    return;
  }
  const terms = query.split(/\s+/);
  const matches = pages
    .map((page) => {
      const title = normalize(page.title);
      const headings = page.headings.map((h) => normalize(h.title)).join(" ");
      const text = normalize(page.text);
      const score = terms.every((term) =>
        `${title} ${headings} ${text}`.includes(term),
      )
        ? terms.reduce(
            (n, term) =>
              n +
              (title.includes(term) ? 12 : 0) +
              (headings.includes(term) ? 5 : 0) +
              (text.includes(term) ? 1 : 0),
            0,
          )
        : 0;
      return { page, score };
    })
    .filter((item) => item.score)
    .sort((a, b) => b.score - a.score)
    .slice(0, 10);
  if (!matches.length) {
    hint(`No results for “${input.value.trim()}”. Try another term.`);
    return;
  }
  results.replaceChildren();
  for (const { page } of matches) {
    const anchor = document.createElement("a");
    anchor.className = "search-result";
    const heading = page.headings.find((h) =>
      terms.some((term) => normalize(h.title).includes(term)),
    );
    anchor.href = `${base}${page.path}${heading ? `#${heading.id}` : ""}`;
    const group = document.createElement("small");
    group.textContent = page.group;
    const title = document.createElement("strong");
    title.textContent = heading
      ? `${page.title} › ${heading.title}`
      : page.title;
    const excerpt = document.createElement("p");
    const at = normalize(page.text).indexOf(terms[0]);
    const start = Math.max(0, at - 35);
    excerpt.textContent = `${start ? "…" : ""}${page.text.slice(start, start + 155)}…`;
    anchor.append(group, title, excerpt);
    anchor.addEventListener("click", () => dialog.close());
    results.append(anchor);
  }
}
async function openSearch() {
  if (!dialog.open) dialog.showModal();
  input.focus();
  hint("Loading documentation...");
  try {
    await loadSearch();
    search();
  } catch {
    hint("Search is unavailable. Use the navigation to find a page.");
  }
}
for (const button of document.querySelectorAll("[data-open-search]"))
  button.addEventListener("click", openSearch);
document
  .querySelector("[data-close-search]")
  .addEventListener("click", () => dialog.close());
dialog.addEventListener("click", (event) => {
  if (event.target === dialog) {
    const r = dialog.getBoundingClientRect();
    if (
      event.clientX < r.left ||
      event.clientX > r.right ||
      event.clientY < r.top ||
      event.clientY > r.bottom
    )
      dialog.close();
  }
});
input.addEventListener("input", search);
input.addEventListener("keydown", (event) => {
  const links = [...results.querySelectorAll("a")];
  if ((event.key === "ArrowDown" || event.key === "ArrowUp") && links.length) {
    event.preventDefault();
    selected =
      (selected + (event.key === "ArrowDown" ? 1 : -1) + links.length) %
      links.length;
    links.forEach((link, i) =>
      link.classList.toggle("selected", i === selected),
    );
    links[selected].scrollIntoView({ block: "nearest" });
  }
  if (event.key === "Enter" && links.length) {
    event.preventDefault();
    links[Math.max(selected, 0)].click();
  }
});
document.addEventListener("keydown", (event) => {
  if ((event.metaKey || event.ctrlKey) && event.key.toLowerCase() === "k") {
    event.preventDefault();
    openSearch();
  }
});
const mobile = document.querySelector("#mobile-nav");
document
  .querySelector("#open-nav")
  .addEventListener("click", () => mobile.showModal());
mobile
  .querySelector(".close-nav")
  .addEventListener("click", () => mobile.close());
if ("IntersectionObserver" in window) {
  const links = [...document.querySelectorAll(".toc nav a")];
  const observer = new IntersectionObserver(
    (entries) => {
      const entry = entries
        .filter((e) => e.isIntersecting)
        .sort((a, b) => a.boundingClientRect.top - b.boundingClientRect.top)[0];
      if (!entry) return;
      for (const link of links) {
        if (link.hash === `#${entry.target.id}`)
          link.setAttribute("aria-current", "location");
        else link.removeAttribute("aria-current");
      }
    },
    { rootMargin: "-90px 0px -65% 0px" },
  );
  for (const heading of document.querySelectorAll(".prose h2,.prose h3"))
    observer.observe(heading);
}
