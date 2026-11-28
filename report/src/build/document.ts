import { createHash } from "node:crypto";
import { checkCSS, checkMarkup } from "./offline";
import css from "../browser/report.css";
import enhance from "../browser/enhance.js.txt";
import type { Theme } from "../types";

const htmlText = (text: string) => text.replace(/[&<>"]/g, c => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;" })[c]!);

export function page(title: string, markup: string, customCSS: string, theme: Theme, interactive: boolean) {
  checkMarkup(markup);
  if (customCSS) checkCSS(customCSS, "bundled CSS");
  if (/<\/style/i.test(customCSS)) throw new Error("bundled CSS cannot contain a style closing tag");
  const hash = createHash("sha256").update(enhance).digest("base64");
  const script = interactive ? `<script>${enhance}</script>` : "";
  const policy = `default-src 'none'; img-src data:; style-src 'unsafe-inline'; script-src ${interactive ? "'sha256-" + hash + "'" : "'none'"}; base-uri 'none'; form-action 'none'`;
  return `<!doctype html>
<html lang="en" data-theme="${theme}">
<head><meta charset="utf-8">
<meta http-equiv="Content-Security-Policy" content="${policy}">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>${htmlText(title)}</title>
<style>${css}\n${customCSS}</style>
</head>
<body><main>${markup}</main>${script}</body>
</html>
`;
}
