// Installs every audiocn registry item into a fresh shadcn app and type-checks
// it. Run `pnpm registry:build` first.
import { execFileSync, spawnSync } from "node:child_process";
import {
  createReadStream,
  existsSync,
  mkdtempSync,
  readFileSync,
  writeFileSync,
} from "node:fs";
import { createServer } from "node:http";
import { tmpdir } from "node:os";
import path from "node:path";

const root = process.cwd();
const registryDir = path.join(root, "public/r");
const port = 4999;

if (!existsSync(path.join(registryDir, "registry.json"))) {
  console.error("Run `pnpm registry:build` first.");
  process.exit(1);
}

const { items } = JSON.parse(
  readFileSync(path.join(root, "registry.json"), "utf-8")
);
const names = items.map((item) => item.name);

const server = createServer((request, response) => {
  const file = path.join(registryDir, path.basename(request.url ?? ""));
  if (!existsSync(file)) {
    response.writeHead(404).end();
    return;
  }
  response.writeHead(200, { "Content-Type": "application/json" });
  createReadStream(file).pipe(response);
});

await new Promise((resolve) => {
  server.listen(port, resolve);
});

const run = (command, args, cwd) => {
  console.log(`$ ${command} ${args.join(" ")}`);
  const result = spawnSync(command, args, { cwd, stdio: "inherit" });
  if (result.status !== 0) {
    server.close();
    throw new Error(
      `${command} ${args.join(" ")} failed with ${result.status}`
    );
  }
};

const workspace = mkdtempSync(
  path.join(process.env.AUDIOCN_FIXTURE_DIR ?? tmpdir(), "audiocn-install-")
);
const app = path.join(workspace, "fixture");

try {
  run(
    "pnpm",
    [
      "dlx",
      "shadcn@latest",
      "init",
      "--name",
      "fixture",
      "--template",
      "next",
      "--preset",
      "base-nova",
      "--yes",
    ],
    workspace
  );

  const configPath = path.join(app, "components.json");
  const config = JSON.parse(readFileSync(configPath, "utf-8"));
  config.registries = {
    ...config.registries,
    "@audiocn": `http://localhost:${port}/{name}.json`,
  };
  writeFileSync(configPath, `${JSON.stringify(config, null, 2)}\n`);

  run(
    "pnpm",
    [
      "dlx",
      "shadcn@latest",
      "add",
      ...names.map((name) => `@audiocn/${name}`),
      "--yes",
      "--overwrite",
    ],
    app
  );
  run("pnpm", ["exec", "tsc", "--noEmit"], app);
  console.log(`\nInstalled and type-checked ${names.length} items in ${app}`);
  execFileSync("ls", ["components/ui", "hooks", "lib/audio"], {
    cwd: app,
    stdio: "inherit",
  });
} finally {
  server.close();
}
