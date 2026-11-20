// Legacy query links stay functional; static example pages have their own crawlable metadata.
const links = JSON.parse(
  document.getElementById("raven-example-routes").textContent,
);
const chosen = new URL(location.href).searchParams.get("example");
const current = document.querySelector('meta[name="ravenrs-example"]')?.content;
if (chosen && Object.hasOwn(links, chosen) && !current) {
  location.replace(new URL(links[chosen], location.href).href);
} else if (current) {
  const url = new URL(location.href);
  url.searchParams.set("example", current);
  url.searchParams.set("playground", "1");
  globalThis.__ravenPlaygroundURL = document.querySelector(
    'meta[name="ravenrs-example-url"]',
  ).content;
  history.replaceState(null, "", url);
}
