import fs from "fs";
import path from "path";
import zlib from "zlib";

const root = path.resolve(import.meta.dirname, "..");
const dir = path.join(root, "web", "static");

function walk(d) {
  const out = [];
  for (const e of fs.readdirSync(d, { withFileTypes: true })) {
    const p = path.join(d, e.name);
    if (e.isDirectory()) out.push(...walk(p));
    else out.push(p);
  }
  return out;
}

const files = walk(dir).filter(
  (f) =>
    /\.(js|css|html|svg)$/.test(f) && !f.endsWith(".br") && !f.endsWith(".gz")
);

for (const f of files) {
  const data = fs.readFileSync(f);
  fs.writeFileSync(f + ".br", zlib.brotliCompressSync(data, { params: { [zlib.constants.BROTLI_PARAM_QUALITY]: 11 } }));
  fs.writeFileSync(f + ".gz", zlib.gzipSync(data, { level: 9 }));
  console.log("compressed:", path.relative(root, f));
}
