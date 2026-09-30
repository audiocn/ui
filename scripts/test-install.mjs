// Installs every audiocn registry item into a fresh shadcn app and type-checks
// it. Run `pnpm registry:build` first.
import { execFileSync, spawn } from "node:child_process";
import { once } from "node:events";
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

server.listen(port);
await once(server, "listening");

// Async on purpose: the registry server shares this process, so a blocking
// spawn would stop it from answering the CLI.
const run = async (command, args, cwd) => {
  console.log(`$ ${command} ${args.join(" ")}`);
  const child = spawn(command, args, { cwd, stdio: "inherit" });
  const [code] = await once(child, "close");
  if (code !== 0) {
    throw new Error(`${command} ${args.join(" ")} failed with ${code}`);
  }
};

const workspace = mkdtempSync(
  path.join(process.env.AUDIOCN_FIXTURE_DIR ?? tmpdir(), "audiocn-install-")
);
const app = path.join(workspace, "fixture");

try {
  await run(
    "pnpm",
    [
      "dlx",
      "shadcn@rc",
      "init",
      "--name",
      "fixture",
      "--template",
      "next",
      "--base",
      "base",
      "--preset",
      "nova",
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

  await run(
    "pnpm",
    [
      "dlx",
      "shadcn@rc",
      "add",
      ...names.map((name) => `@audiocn/${name}`),
      "--yes",
      "--overwrite",
    ],
    app
  );
  await run("pnpm", ["exec", "tsc", "--noEmit"], app);
  console.log(`\nInstalled and type-checked ${names.length} items in ${app}`);
  execFileSync("ls", ["components/ui", "hooks", "lib/audio"], {
    cwd: app,
    stdio: "inherit",
  });
} finally {
  server.close();
}
