// Installs audiocn registry items into fresh shadcn apps and type-checks them.
// Run `pnpm registry:build` first.
//
// Two fixtures:
// - base: a Base UI app, every item.
// - radix: a Radix app compiled against ES2022, every item that does not
//   compose a Base-only shadcn API (the blocks and audio-device-select do).
//   This is how a Radix project such as Videorc installs audiocn, and the
//   ES2022 lib catches newer APIs (`toSorted`) the default config hides.
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
const allNames = items.map((item) => item.name);
// Built on a Base-only shadcn API: the Select `items` prop, render-function
// `SelectValue`, `render` composition.
const baseOnly = new Set(["audio-device-select"]);
const radixNames = items
  .filter((item) => item.type !== "registry:block" && !baseOnly.has(item.name))
  .map((item) => item.name);

const fixtures = [
  { base: "base", lib: null, name: "base", names: allNames },
  {
    base: "radix",
    lib: ["ES2022", "DOM", "DOM.Iterable"],
    name: "radix",
    names: radixNames,
  },
];

const only = process.env.AUDIOCN_FIXTURE;

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

const editJson = (file, edit) => {
  const json = JSON.parse(readFileSync(file, "utf-8"));
  edit(json);
  writeFileSync(file, `${JSON.stringify(json, null, 2)}\n`);
};

const installFixture = async (fixture) => {
  const workspace = mkdtempSync(
    path.join(
      process.env.AUDIOCN_FIXTURE_DIR ?? tmpdir(),
      `audiocn-install-${fixture.name}-`
    )
  );
  const app = path.join(workspace, "fixture");

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
      fixture.base,
      "--preset",
      "nova",
      "--yes",
    ],
    workspace
  );

  editJson(path.join(app, "components.json"), (config) => {
    config.registries = {
      ...config.registries,
      "@audiocn": `http://localhost:${port}/{name}.json`,
    };
  });
  if (fixture.lib) {
    editJson(path.join(app, "tsconfig.json"), (tsconfig) => {
      tsconfig.compilerOptions.lib = fixture.lib;
    });
  }

  await run(
    "pnpm",
    [
      "dlx",
      "shadcn@rc",
      "add",
      ...fixture.names.map((name) => `@audiocn/${name}`),
      "--yes",
      "--overwrite",
    ],
    app
  );
  await run("pnpm", ["exec", "tsc", "--noEmit"], app);
  console.log(
    `\n[${fixture.name}] installed and type-checked ${fixture.names.length} items in ${app}`
  );
  execFileSync("ls", ["components/ui", "hooks", "lib/audio"], {
    cwd: app,
    stdio: "inherit",
  });
};

// One after the other: both fixtures use the same registry server and port.
const installInTurn = async ([next, ...rest]) => {
  if (!next) {
    return;
  }
  await installFixture(next);
  await installInTurn(rest);
};

const selected = fixtures.filter((fixture) => !only || only === fixture.name);
try {
  await installInTurn(selected);
} finally {
  server.close();
}
