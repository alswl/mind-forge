import { parseFragment, type DefaultTreeAdapterMap } from "parse5";

// The CommonJS exports inline the CSS parser's data when esbuild bundles them.
const { parse: cssParse, walk } = require("css-tree") as typeof import("css-tree");

const embeddedImage = /^data:image\/(?:png|gif|jpeg|webp|avif|bmp|x-icon);base64,[a-z0-9+/]+=*$/i;
const localFragment = /^#[^\s]*$/;

export function checkCSS(css: string, context: string, declaration = false) {
  const invalid = (value: string): never => { throw new Error(`${context}: external or unsupported CSS resource ${value}`); };
  const tree = cssParse(css, {
    context: declaration ? "declarationList" : "stylesheet",
    onParseError(error) { throw new Error(`${context}: ${error.message}`); },
  });
  walk(tree, node => {
    if (node.type === "Url" && !localFragment.test(node.value) && !embeddedImage.test(node.value)) invalid(node.value);
    if (node.type === "Atrule" && node.name.toLowerCase() === "import") invalid("@import");
    if (node.type === "Function" && /^(?:-webkit-)?image-set$/i.test(node.name)) invalid(node.name);
    if (node.type === "Raw" && /url|image-set|@import|\\/i.test(node.value)) invalid(node.value);
  });
}

const activeTags = new Set(["script", "iframe", "object", "embed", "base", "link", "meta", "video", "audio", "source", "track", "portal", "frame", "fencedframe"]);
const resourceAttrs = new Set(["src", "srcset", "imagesrcset", "imagesizes", "poster", "background", "manifest", "ping", "attributionsrc", "data", "srcdoc", "action", "formaction", "codebase", "archive", "dynsrc", "lowsrc"]);
const svgPaint = new Set(["fill", "stroke", "filter", "mask", "clip-path", "marker-start", "marker-mid", "marker-end", "cursor"]);

export function checkMarkup(fragment: string) {
  function visit(node: DefaultTreeAdapterMap["childNode"] | DefaultTreeAdapterMap["documentFragment"]) {
    if ("tagName" in node) {
      const tag = node.tagName.toLowerCase();
      if (activeTags.has(tag) || (node.namespaceURI.endsWith("/svg") && /^(animate|set)/.test(tag))) {
        const resource = node.attrs.find(attr => attr.name === "href" || attr.name === "src");
        if (resource) {
          throw new Error(`offline markup: unsupported resource <${tag}> ${resource.name}=${JSON.stringify(resource.value)}`);
        }
        throw new Error(`offline markup: unsupported <${tag}> element`);
      }
      if (tag === "style") {
        checkCSS(node.childNodes.map(child => "value" in child ? child.value : "").join(""), "<style>");
      }
      for (const attr of node.attrs) {
        const { name, value } = attr;
        const where = `<${tag}> ${name}`;
        if (name === "style") checkCSS(value, where, true);
        if (svgPaint.has(name) && /url|\\/i.test(value)) checkCSS(`${name}:${value}`, where, true);
        if (name.startsWith("on") || name === "is") throw new Error(`offline markup: unsupported ${where}`);
        if (resourceAttrs.has(name) && !(name === "src" && tag === "img" && embeddedImage.test(value))) {
          throw new Error(`offline markup: unsupported resource ${where}=${JSON.stringify(value)}`);
        }
        if (name === "href" && !localFragment.test(value) &&
          !(tag === "a" && /^(https?:\/\/|mailto:|tel:)/i.test(value)) &&
          !(tag === "image" && embeddedImage.test(value))) {
          throw new Error(`offline markup: unsupported resource ${where}=${JSON.stringify(value)}`);
        }
      }
    }
    if ("content" in node) visit((node as DefaultTreeAdapterMap["template"]).content);
    if ("childNodes" in node) node.childNodes.forEach(visit);
  }
  visit(parseFragment(fragment));
}
