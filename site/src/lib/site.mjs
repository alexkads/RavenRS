export const siteUrl = (process.env.RAVENRS_SITE_URL || "https://ravenrs.vercel.app").replace(/\/$/, "");
export const base = new URL(siteUrl).pathname.replace(/\/$/, "");
export const publicPath = (path = "") => `${base}/${path}`;
