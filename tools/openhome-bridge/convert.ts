// Converts one Pokemon between game formats using OpenHome's code (github.com/andrewbenington/OpenHome, GPL-3.0).
// mon-bridge calls this for moves with no real transfer between the games, and PKHeX checks whatever comes out.
//
//   node convert.cjs <from format> <input file> <to format> <output file>     formats: pk1 pk2 pk3 pk4 pk5
//
// Exits 0 with the output file written, otherwise prints why and exits non-zero. Licensed under the GNU GPL v3 or later.
import { OHPKM } from '@openhome-core/pkm/OHPKM'
import { PK1, PK2, PK3, PK4, PK5 } from '@openhome-core/pkm'
import { R } from '@openhome-core/util/functional'
import init, { ConvertStrategies } from '@pkm-rs/pkg'
import { readFileSync, writeFileSync } from 'node:fs'
import { join } from 'node:path'

const formats: Record<string, any> = { pk1: PK1, pk2: PK2, pk3: PK3, pk4: PK4, pk5: PK5 }

async function main() {
  const [from, input, to, output] = process.argv.slice(2)
  const source = formats[from]
  const target = formats[to]
  if (!source || !target || !input || !output) throw new Error('usage: <from format> <input> <to format> <output>')
  await init({ module_or_path: readFileSync(join(__dirname, 'pkm_rs_bg.wasm')) })
  const bytes = new Uint8Array(readFileSync(input))
  const mon = source.fromBytes(bytes.buffer)
  const home = OHPKM.fromMonUnknownSave(mon)
  const converted = R.assert(target.fromOhpkm(home, ConvertStrategies.getDefault()))
  converted.refreshChecksum?.()
  writeFileSync(output, new Uint8Array(converted.toBytes()))
}

main().catch((e) => {
  console.error(e instanceof Error ? e.message : String(e))
  process.exit(1)
})
