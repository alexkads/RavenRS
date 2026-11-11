import { createRequire } from "node:module";
import { writeFile, mkdir } from "node:fs/promises";
import { resolve } from "node:path";
const require = createRequire(
  new URL("../../site/docs/package.json", import.meta.url),
);
const { Resvg } = require("@resvg/resvg-js");
const root = resolve(new URL("../..", import.meta.url).pathname);
const escape = (s) =>
  String(s)
    .replaceAll("&", "&amp;")
    .replaceAll("<", "&lt;")
    .replaceAll(">", "&gt;")
    .replaceAll('"', "&quot;");
function lines(value, width, max = 3) {
  const words = value.split(/\s+/);
  const result = [];
  let line = "";
  for (const word of words) {
    if (line && (line + " " + word).length > width) {
      result.push(line);
      line = word;
    } else line += (line ? " " : "") + word;
  }
  if (line) result.push(line);
  return result
    .slice(0, max)
    .map((line, i) =>
      i === max - 1 && result.length > max
        ? line.slice(0, width - 1) + "…"
        : line,
    );
}
export async function socialImage(out, id, title, description, section) {
  const folder = resolve(out, "assets/social");
  await mkdir(folder, { recursive: true });
  const heading = lines(title, 31, 3);
  const subtitle = lines(description, 76, 2);
  const svg = `<svg xmlns="http://www.w3.org/2000/svg" width="1200" height="630" viewBox="0 0 1200 630">
 <defs><linearGradient id="g"><stop stop-color="#a98bff"/><stop offset="1" stop-color="#63d9ef"/></linearGradient><radialGradient id="glow"><stop stop-color="#41479a" stop-opacity=".7"/><stop offset="1" stop-color="#0b0d16" stop-opacity="0"/></radialGradient><pattern id="grid" width="48" height="48" patternUnits="userSpaceOnUse"><path d="M48 0H0V48" fill="none" stroke="#22283b" stroke-width="1"/></pattern></defs>
 <rect width="1200" height="630" fill="#0b0d16"/><rect width="1200" height="630" fill="url(#grid)"/>
 <ellipse cx="1090" cy="110" rx="590" ry="520" fill="url(#glow)"/>
 <rect x="58" y="52" width="42" height="42" rx="12" fill="url(#g)"/><path d="M72 65h10a7 7 0 0 1 0 14H72zm0 14 14 9" fill="none" stroke="#101322" stroke-width="4"/>
 <text x="116" y="84" font-family="Inter" font-size="31" font-weight="600" fill="#edf1ff">RavenRS</text>
 <text x="60" y="152" font-family="Inter" font-size="19" font-weight="600" letter-spacing="3" fill="#9dacf1">${escape(section.toUpperCase())}</text>
 ${heading.map((line, i) => `<text x="58" y="${224 + i * 70}" font-family="Inter" font-size="62" font-weight="600" fill="#f2f4ff">${escape(line)}</text>`).join("")}
 ${subtitle.map((line, i) => `<text x="60" y="${443 + i * 31}" font-family="Inter" font-size="23" fill="#b0bbd2">${escape(line)}</text>`).join("")}
 <path d="M60 535h1080" stroke="#30394f"/><text x="60" y="582" font-family="Inter" font-size="20" fill="#a2b0cf">Rust at heart. Native + Web.</text><text x="1140" y="582" text-anchor="end" font-family="Inter" font-size="20" fill="#81d8eb">ravenrs.vercel.app</text></svg>`;
  const renderer = new Resvg(svg, {
    font: {
      fontFiles: [
        resolve(root, "crates/ravenrs-text/fonts/Inter-Regular.ttf"),
        resolve(root, "crates/ravenrs-text/fonts/Inter-SemiBold.ttf"),
      ],
      loadSystemFonts: false,
    },
  });
  await writeFile(resolve(folder, id + ".png"), renderer.render().asPng());
  return "assets/social/" + id + ".png";
}
