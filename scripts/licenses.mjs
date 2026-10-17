import { readFileSync, writeFileSync, mkdirSync } from 'node:fs'
import { execFileSync } from 'node:child_process'
mkdirSync('dist', { recursive: true })
const metadata = JSON.parse(
  execFileSync(
    process.env.CARGO || 'cargo',
    [
      'metadata',
      '--locked',
      '--manifest-path',
      'src-tauri/Cargo.toml',
      '--filter-platform',
      'x86_64-pc-windows-msvc',
      '--format-version',
      '1',
    ],
    { encoding: 'utf8', maxBuffer: 16 * 1024 * 1024 },
  ),
)
writeFileSync(
  'dist/tauri-dependency-licenses.json',
  JSON.stringify(
    metadata.packages.map(({ name, version, license }) => ({ name, version, license })),
    null,
    2,
  ),
)
const lock = JSON.parse(readFileSync('package-lock.json', 'utf8'))
writeFileSync(
  'dist/tauri-frontend-licenses.json',
  JSON.stringify(
    Object.entries(lock.packages)
      .filter(([path, pkg]) => path && !pkg.dev)
      .map(([path, pkg]) => ({
        name: pkg.name || path.split('node_modules/').at(-1),
        version: pkg.version,
        license: pkg.license || '见包内许可证',
      })),
    null,
    2,
  ),
)
