import { describe, expect, it } from "vitest";
import { checkCSS, checkMarkup } from "./offline";

describe("offline resource verification", () => {
  it("accepts inline data images, fragment references and ordinary links", () => {
    expect(() => checkMarkup('<a href="https://example.invalid">Source</a><img src="data:image/png;base64,AAAA"><svg><use href="#symbol"></use></svg>')).not.toThrow();
    expect(() => checkCSS('svg { fill: url("#gradient") }', "chart")).not.toThrow();
  });
  it.each([
    ['<img src="https://example.invalid/image.png">', "img"],
    ['<link rel="preload" as="image" href="https://example.invalid/external.png">', "external.png"],
    ['<svg><image xlink:href="other.svg#icon"></image></svg>', "href"],
    ['<style>@import "remote.css";</style>', "@import"],
    ['<template><iframe src="external.html"></iframe></template>', "iframe"],
    ['<a href="javascript:alert(1)">Click</a>', "href"],
    ['<div style="background:image-set(\'outside.png\' 1x)"></div>', "image-set"],
  ])("rejects resources in %s", (markup, hint) => {
    expect(() => checkMarkup(markup)).toThrow(hint);
  });
});
