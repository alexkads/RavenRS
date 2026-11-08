// Keeps the "A to Z" list where it was: every component is its own page, so
// without this the list jumps back to the top on each click.
(() => {
  const side = document.querySelector(".side");
  if (!side) return;
  const key = "ravenrs-components-side";
  let saved = null;
  try {
    saved = sessionStorage.getItem(key);
  } catch {}
  if (saved !== null) {
    side.scrollTop = Number(saved) || 0;
  } else {
    // First visit straight to a component: bring its row into view.
    const current = side.querySelector("a[aria-current]");
    if (current) {
      const top = current.offsetTop - side.clientHeight / 2 + current.offsetHeight / 2;
      side.scrollTop = Math.max(0, top);
    }
  }
  const save = () => {
    try {
      sessionStorage.setItem(key, String(side.scrollTop));
    } catch {}
  };
  side.addEventListener("click", save);
  addEventListener("pagehide", save);
})();
