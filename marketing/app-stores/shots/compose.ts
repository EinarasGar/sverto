import { existsSync, mkdirSync, readdirSync, rmSync, statSync } from "node:fs"
import { chromium } from "playwright"

const SHOTS = import.meta.dir
const STORES = `${SHOTS}/..`
const ROOT = `${SHOTS}/../../..`
const RAW = `${SHOTS}/raw`

type Slot = { size: [number, number]; layout: string; frame: string; capture?: string; captures?: string[]; web?: string }
type Shot = { slot: string; name: string; screen: string; eyebrow: string; headline: string[]; subhead: string; placement: string }
const { slots, shots } = (await Bun.file(`${SHOTS}/shots.json`).json()) as { slots: Record<string, Slot>; shots: Shot[] }

function language(store: "ios" | "android") {
  const dir = `${STORES}/${store}/metadata`
  const found = existsSync(dir)
    ? readdirSync(dir).filter((name) => name !== "review_information" && statSync(`${dir}/${name}`).isDirectory())
    : []
  if (found.length !== 1) {
    console.error(`Expected exactly one language folder in ${store}/metadata, found ${found.length ? found.join(", ") : "none"}.`)
    process.exit(1)
  }
  return found[0]
}
const ios = language("ios")
const android = language("android")
const play = `${STORES}/android/metadata/${android}/images`

const nn = (n: number) => String(n).padStart(2, "0")
function output(shot: Shot, index: number) {
  switch (shot.slot) {
    case "iphone":
      return `${STORES}/ios/screenshots/${ios}/${nn(1 + index)}-iphone-${shot.name}.png`
    case "ipad":
      return `${STORES}/ios/screenshots/${ios}/${nn(11 + index)}-ipad-${shot.name}.png`
    case "duo-outer":
      return `${STORES}/ios/screenshots-duo/${ios}/${nn(21 + index)}-duo-outer-${shot.name}.png`
    case "duo-inner":
      return `${STORES}/ios/screenshots-duo/${ios}/${nn(31 + index)}-duo-inner-${shot.name}.png`
    case "phone":
      return `${play}/phoneScreenshots/${nn(1 + index)}-${shot.name}.png`
    case "tablet-7":
      return `${play}/sevenInchScreenshots/${nn(1 + index)}-${shot.name}.png`
    case "tablet-10":
      return `${play}/tenInchScreenshots/${nn(1 + index)}-${shot.name}.png`
    case "feature-graphic":
      return `${play}/featureGraphic.png`
    case "readme-ios":
    case "readme-android":
    case "readme-hero":
      return `${ROOT}/.github/assets/${shot.name}.png`
    default:
      return `${STORES}/android/foldable/${android}/${1 + shots.filter((s) => s.slot.startsWith("foldable-")).indexOf(shot)}-${shot.slot.replace("foldable-", "")}.png`
  }
}

const jobs = shots.map((shot) => {
  const index = shots.filter((s) => s.slot === shot.slot).indexOf(shot)
  const slot = slots[shot.slot]
  const raws = (slot.captures ?? [slot.capture]).map((capture) => `${RAW}/${capture}/${shot.screen}.png`)
  return { shot, slot, raws, out: output(shot, index) }
})
const missing = [...new Set(jobs.flatMap((j) => j.raws.filter((raw) => !existsSync(raw)).map((raw) => raw.replace(`${SHOTS}/`, ""))))]
if (missing.length) {
  console.error(`Missing raw captures (run make store-shots):\n${missing.join("\n")}`)
  process.exit(1)
}

const clear = (dir: string, keep?: string) => {
  if (!existsSync(dir)) return
  for (const name of readdirSync(dir)) {
    if (keep && name !== keep) rmSync(`${dir}/${name}`, { recursive: true, force: true })
  }
}
clear(`${STORES}/ios/screenshots`, ios)
clear(`${STORES}/ios/screenshots-duo`, ios)
clear(`${STORES}/android/foldable`, android)
for (const lang of existsSync(`${STORES}/android/metadata`) ? readdirSync(`${STORES}/android/metadata`) : [])
  if (lang !== android) rmSync(`${STORES}/android/metadata/${lang}/images`, { recursive: true, force: true })
for (const dir of [
  `${STORES}/ios/screenshots/${ios}`,
  `${STORES}/ios/screenshots-duo/${ios}`,
  `${STORES}/android/foldable/${android}`,
  `${play}/phoneScreenshots`,
  `${play}/sevenInchScreenshots`,
  `${play}/tenInchScreenshots`,
]) {
  rmSync(dir, { recursive: true, force: true })
  mkdirSync(dir, { recursive: true })
}
rmSync(`${play}/featureGraphic.png`, { force: true })

const browser = await chromium.launch()
const page = await browser.newPage({ deviceScaleFactor: 1 })
await page.goto(`file://${SHOTS}/template.html`)
await page.evaluate(() => document.fonts.ready)
for (const { shot, slot, raws, out } of jobs) {
  const images = await Promise.all(
    raws.map(async (raw) => {
      const png = Buffer.from(await Bun.file(raw).arrayBuffer())
      return { src: `file://${raw}`, w: png.readUInt32BE(16), h: png.readUInt32BE(20) }
    }),
  )
  const web = slot.web && `file://${ROOT}/${slot.web}`
  await page.setViewportSize({ width: slot.size[0], height: slot.size[1] || 3 * slot.size[0] })
  await page.evaluate(([shot, slot, images, web]) => (window as any).render(shot, slot, images, web), [shot, slot, images, web] as const)
  await page.waitForFunction(() => [...document.images].every((i) => i.complete) && document.fonts.status === "loaded")
  const readme = shot.slot.startsWith("readme-")
  if (readme) await page.locator("#canvas").screenshot({ path: out, omitBackground: true })
  else await page.screenshot({ path: out })
  const file = Buffer.from(await Bun.file(out).arrayBuffer())
  const [ow, oh, colorType] = [file.readUInt32BE(16), file.readUInt32BE(20), file[25]]
  const heightOk = slot.layout === "bare" || oh === slot.size[1]
  if (ow !== slot.size[0] || !heightOk || colorType !== (readme ? 6 : 2) || file.length > 8_000_000)
    throw new Error(`${out}: ${ow}x${oh}, colour type ${colorType}, ${file.length} bytes`)
  console.log(`  ${out.replace(`${STORES}/`, "")}`)
}
await browser.close()
console.log(`Rendered ${jobs.length} images.`)
