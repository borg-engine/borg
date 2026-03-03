#!/usr/bin/env bun
/**
 * Template fill service — reads a .docx template, replaces {placeholders}
 * with provided data, writes the filled document.
 *
 * Usage: echo '{"templatePath":...,"outputPath":...,"data":{...}}' | bun run fill.ts
 *
 * Data fields (all optional, absent fields render as empty string):
 *   title, subtitle, client_name, case_number, jurisdiction, matter_type,
 *   date, prepared_by, privilege_header, body, body_html
 *
 * The template should contain {tags} matching the data keys, e.g.:
 *   {title}, {client_name}, {body}, etc.
 *
 * Angular expressions like {#items}{name}{/items} are supported via
 * docxtemplater's built-in loop syntax.
 */

import Docxtemplater from "docxtemplater";
import PizZip from "pizzip";
import { readFileSync, writeFileSync } from "fs";

interface FillRequest {
  templatePath: string;
  outputPath: string;
  data: Record<string, unknown>;
}

async function main() {
  const chunks: Buffer[] = [];
  for await (const chunk of Bun.stdin.stream()) {
    chunks.push(Buffer.from(chunk));
  }
  const input = Buffer.concat(chunks).toString("utf-8").trim();
  if (!input) {
    console.error("No input provided on stdin");
    process.exit(1);
  }

  let req: FillRequest;
  try {
    req = JSON.parse(input);
  } catch {
    console.error("Invalid JSON input");
    process.exit(1);
  }

  if (!req.templatePath || !req.outputPath || !req.data) {
    console.error("Missing required fields: templatePath, outputPath, data");
    process.exit(1);
  }

  try {
    const templateBuf = readFileSync(req.templatePath);
    const zip = new PizZip(templateBuf);
    const doc = new Docxtemplater(zip, {
      paragraphLoop: true,
      linebreaks: true,
      // Silently replace missing tags with empty string
      nullGetter() { return ""; },
    });

    doc.render(req.data);

    const buf = doc.getZip().generate({
      type: "nodebuffer",
      compression: "DEFLATE",
    });
    writeFileSync(req.outputPath, buf);

    console.log(JSON.stringify({ ok: true, size: buf.length }));
  } catch (e: unknown) {
    const msg = e instanceof Error ? e.message : String(e);
    console.error(`Template fill failed: ${msg}`);
    process.exit(1);
  }
}

main();
