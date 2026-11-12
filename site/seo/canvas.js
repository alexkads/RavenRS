// Progressive enhancement: the same useful HTML is available to people and crawlers.
const intro = document.querySelector("[data-raven-intro]");
function ready() {
  const canvas = document.getElementById("raven-canvas");
  if (canvas && (canvas.width !== 300 || canvas.height !== 150)) {
    requestAnimationFrame(() =>
      requestAnimationFrame(() => {
        intro.hidden = true;
        if (globalThis.__ravenPlaygroundURL)
          history.replaceState(null, "", globalThis.__ravenPlaygroundURL);
        document.documentElement.classList.remove("raven-fallback");
      }),
    );
    observer.disconnect();
  }
}
const observer = new MutationObserver(ready);
if (intro) {
  observer.observe(document.body, {
    attributes: true,
    childList: true,
    subtree: true,
    attributeFilter: ["width", "height"],
  });
  ready();
}
