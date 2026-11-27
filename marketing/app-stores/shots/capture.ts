import { $ } from "bun"
import { existsSync, mkdirSync, rmSync } from "node:fs"
import { homedir } from "node:os"

const SHOTS = import.meta.dir
const ROOT = `${SHOTS}/../../..`
const RAW = `${SHOTS}/raw`
const SDK = `${homedir()}/Library/Android/sdk`
const ADB = `${SDK}/platform-tools/adb`
const MOBILECLI = `${SHOTS}/node_modules/.bin/mobilecli`
const APP = "com.sverto.app.dev"
const CONVERSATION = "Groceries last month"
const BRAND_SEED = "FF1F8A65"

type Platform = "android" | "ios"
type Device = "phone" | "tablet" | "fold"
type Config = { id: string; platform: Platform; device: Device; wide: boolean; folded?: boolean; rotation?: number; listTop?: number }

const CONFIGS: Config[] = [
  { id: "android-phone", platform: "android", device: "phone", wide: false },
  { id: "android-tablet", platform: "android", device: "tablet", wide: true, rotation: 0 },
  { id: "android-fold-open", platform: "android", device: "fold", wide: true, rotation: 0 },
  { id: "android-fold-closed", platform: "android", device: "fold", wide: false, folded: true },
  { id: "ios-iphone", platform: "ios", device: "phone", wide: false },
  { id: "ios-ipad", platform: "ios", device: "tablet", wide: true },
  { id: "ios-duo-open", platform: "ios", device: "fold", wide: true },
  { id: "ios-duo-closed", platform: "ios", device: "fold", wide: false, folded: true, listTop: 0.01 },
]

type El = { label: string; type: string; x: number; y: number; w: number; h: number }

interface Driver {
  elements(): Promise<El[]>
  tap(x: number, y: number): Promise<void>
  drag(x: number, y1: number, y2: number): Promise<void>
  size(): Promise<{ w: number; h: number }>
  hideKeyboard(): Promise<void>
  fresh(): Promise<void>
  png(): Promise<Buffer>
}

const sleep = (ms: number) => new Promise((r) => setTimeout(r, ms))
const env = (developerDir = "") => ({ ...process.env, ANDROID_HOME: SDK, ...(developerDir ? { DEVELOPER_DIR: developerDir } : {}) })

async function makeVar(name: string, device: Device) {
  return (await $`make -s -f Makefile -f ${SHOTS}/vars.mk ${name} DEVICE=${device}`.cwd(ROOT).text()).trim()
}

async function make(target: string, device: Device) {
  return (await $`make ${target} DEVICE=${device}`.cwd(ROOT).env(env()).nothrow()).exitCode === 0
}

function pngFrom(buf: Buffer) {
  return buf.subarray(buf.indexOf(Buffer.from([0x89, 0x50, 0x4e, 0x47])))
}

function android(serial: string, display: () => string | undefined): Driver {
  const adb = (...args: string[]) => $`${ADB} -s ${serial} ${args}`.quiet()
  return {
    async elements() {
      await adb("shell", "rm", "-f", "/sdcard/ui.xml").nothrow()
      await adb("shell", "uiautomator", "dump", "/sdcard/ui.xml").nothrow()
      const xml = (await adb("exec-out", "cat", "/sdcard/ui.xml").nothrow()).stdout.toString()
      const els: El[] = []
      for (const m of xml.matchAll(/<node [^>]*>/g)) {
        const attr = (k: string) => m[0].match(new RegExp(` ${k}="([^"]*)"`))?.[1] ?? ""
        const label = (attr("text") || attr("content-desc"))
          .replaceAll("&amp;", "&")
          .replaceAll("&quot;", '"')
          .replaceAll("&#39;", "'")
          .replaceAll("&apos;", "'")
        const b = attr("bounds").match(/\d+/g)?.map(Number)
        if (label.trim() && b) els.push({ label: label.trim(), type: attr("class"), x: b[0], y: b[1], w: b[2] - b[0], h: b[3] - b[1] })
      }
      return els
    },
    async tap(x, y) {
      await adb("shell", "input", "tap", `${Math.round(x)}`, `${Math.round(y)}`)
    },
    async drag(x, y1, y2) {
      await adb("shell", "input", "swipe", `${Math.round(x)}`, `${Math.round(y1)}`, `${Math.round(x)}`, `${Math.round(y2)}`, "1200")
      await sleep(700)
    },
    async size() {
      const png = await this.png()
      return { w: png.readUInt32BE(16), h: png.readUInt32BE(20) }
    },
    async hideKeyboard() {
      if ((await adb("shell", "dumpsys", "input_method")).stdout.toString().includes("mInputShown=true"))
        await adb("shell", "input", "keyevent", "BACK")
      await sleep(500)
    },
    async fresh() {
      await adb("shell", "am", "force-stop", APP)
      await adb("shell", "am", "start", "-n", `${APP}/com.sverto.app.MainActivity`)
    },
    async png() {
      const d = display()
      return pngFrom((await adb("exec-out", "screencap", "-p", ...(d ? ["-d", d] : []))).stdout)
    },
  }
}

function ios(udid: string, developerDir: string, display: () => string | undefined): Driver {
  const cli = (...args: string[]) => $`${MOBILECLI} ${args} --device ${udid}`.env(env(developerDir)).quiet()
  let scale = 0
  const driver: Driver = {
    async elements() {
      const { w, h } = await driver.size()
      for (let i = 0; i < 4; i++) {
        try {
          const data = JSON.parse((await cli("dump", "ui", "--full").nothrow()).stdout.toString()).data
          const els: El[] = []
          const walk = (ns: any[]) => {
            for (const n of ns ?? []) {
              const label = (n.label || n.name || "").trim()
              if (label) els.push({ label, type: n.type, x: n.rect.x, y: n.rect.y, w: n.rect.width, h: n.rect.height })
              walk(n.children)
            }
          }
          walk(data.elements)
          return els.filter((e) => e.x + e.w / 2 >= 0 && e.y + e.h / 2 >= 0 && e.x + e.w / 2 <= w && e.y + e.h / 2 <= h)
        } catch {
          await sleep(1500)
        }
      }
      return []
    },
    async tap(x, y) {
      await cli("io", "tap", `${Math.round(x)},${Math.round(y)}`)
    },
    async drag(x, y1, y2) {
      await cli("io", "drag", `${Math.round(x)},${Math.round(y1)},${Math.round(x)},${Math.round(y2)}`, "--duration", "1200")
      await sleep(900)
    },
    async size() {
      scale ||= JSON.parse((await cli("device", "info")).stdout.toString()).data.device.screenSize.scale
      const png = await driver.png()
      return { w: png.readUInt32BE(16) / scale, h: png.readUInt32BE(20) / scale }
    },
    async hideKeyboard() {
      const done = (await driver.elements()).find((e) => e.label === "Done" && e.type === "Button")
      if (done) await driver.tap(done.x + done.w / 2, done.y + done.h / 2)
      await sleep(500)
    },
    async fresh() {
      await $`xcrun simctl terminate ${udid} ${APP}`.env(env(developerDir)).quiet().nothrow()
      await $`xcrun simctl launch ${udid} ${APP}`.env(env(developerDir)).quiet()
    },
    async png() {
      const file = `${RAW}/.ios-shot.png`
      const d = display()
      await $`xcrun simctl io ${udid} screenshot ${d ? [`--display=${d}`] : []} ${file}`.env(env(developerDir)).quiet()
      return Buffer.from(await Bun.file(file).arrayBuffer())
    },
  }
  return driver
}

type Find = { exact?: boolean; type?: string; last?: boolean }

function matches(els: El[], label: string | RegExp, f: Find = {}) {
  const ofType = f.type ? els.filter((e) => e.type === f.type) : els
  const found =
    label instanceof RegExp
      ? ofType.filter((e) => label.test(e.label))
      : ofType.filter((e) => e.label === label).length || f.exact
        ? ofType.filter((e) => e.label === label)
        : ofType.filter((e) => e.label.toLowerCase().includes(label.toLowerCase()))
  return f.last ? found.reverse() : found
}

async function waitFor(d: Driver, label: string | RegExp, f: Find = {}, timeout = 15000): Promise<El> {
  const end = Date.now() + timeout
  while (Date.now() < end) {
    const el = matches(await d.elements(), label, f)[0]
    if (el) return el
    await sleep(700)
  }
  throw new Error(`"${label}" did not appear`)
}

const centre = (d: Driver, el: El) => d.tap(el.x + el.w / 2, el.y + el.h / 2)

async function tap(d: Driver, label: string | RegExp, f: Find = {}) {
  await centre(d, await waitFor(d, label, f))
  await sleep(1000)
}

async function scrollTo(d: Driver, label: string | RegExp, f: Find = {}, tries = 15, x = 0.75) {
  const { w, h } = await d.size()
  for (let i = 0; i < tries; i++) {
    const el = matches(await d.elements(), label, f).find((e) => e.y + e.h < h * 0.88)
    if (el) return el
    await d.drag(w * x, h * 0.75, h * 0.3)
  }
  throw new Error(`"${label}" not found after scrolling`)
}

async function scrollTap(d: Driver, label: string, f: Find = {}) {
  await centre(d, await scrollTo(d, label, f))
  await sleep(1000)
}

async function toTop(d: Driver, el: El, top = 0.14) {
  const { h } = await d.size()
  for (let i = 0; i < 3; i++) {
    const dy = el.y - h * top
    if (Math.abs(dy) < h * 0.08) return
    const from = dy > 0 ? Math.min(h * 0.85, el.y + h * 0.1) : h * 0.3
    await d.drag(el.x + el.w / 2, from, from - dy)
    const moved = matches(await d.elements(), el.label, { exact: true }).sort((a, b) => Math.abs(a.y - h * top) - Math.abs(b.y - h * top))[0]
    if (!moved) return
    el = moved
  }
}

async function settle(d: Driver) {
  let last = await d.png()
  for (let i = 0; i < 15; i++) {
    await sleep(700)
    const next = await d.png()
    if (next.equals(last)) return next
    last = next
  }
  return last
}

const missed: string[] = []
const DATE = /^(Today|Yesterday|(Mon|Tue|Wed|Thu|Fri|Sat|Sun)\w*,? .*|\w{3,9} \d{1,2}(, \d{4})?)$/

async function walk(c: Config, d: Driver) {
  const dir = `${RAW}/${c.id}`
  rmSync(dir, { recursive: true, force: true })
  mkdirSync(dir, { recursive: true })
  const a = c.platform === "android"
  const tab = (name: string) => tap(d, name, a ? { exact: true } : { exact: true, type: "Button" })
  const fresh = async () => {
    await d.fresh()
    await waitFor(d, "Holdings", {}, 30000)
    await sleep(1500)
  }
  const shot = async (name: string, steps: () => Promise<void>) => {
    for (let attempt = 1; attempt <= 2; attempt++) {
      try {
        await steps()
        await Bun.write(`${dir}/${name}.png`, await settle(d))
        console.log(`  ${c.id}/${name}`)
        return
      } catch (e) {
        console.log(`  ${attempt === 1 ? "retrying" : "MISSED"} ${c.id}/${name}: ${(e as Error).message}`)
        if (attempt === 2) missed.push(`${c.id}/${name}: ${(e as Error).message}`)
      }
    }
  }
  const assetBuyDay = async () => {
    await tab("Transactions")
    const buy = await scrollTo(d, "Asset Buy", { exact: true }, 60, c.wide ? 0.22 : 0.75)
    const day = matches(await d.elements(), DATE)
      .filter((e) => e.y < buy.y)
      .sort((a, b) => b.y - a.y)[0]
    await toTop(d, day ?? buy, day ? (c.listTop ?? 0.14) : 0.45)
  }
  const openAssetBuy = async () => {
    await assetBuyDay()
    for (let i = 0; i < 3; i++) {
      await tap(d, "Asset Buy", { exact: true })
      if (await waitFor(d, "Edit", { exact: true }, 6000).then(() => true, () => false)) return
    }
    throw new Error("the Asset Buy detail did not open")
  }
  const openMyra = async () => {
    await tab("Myra")
    if (!c.wide) await tap(d, a ? "Menu" : "Conversations")
    await tap(d, CONVERSATION)
    await waitFor(d, "Shop")
  }

  await shot("statement-import", async () => {
    await fresh()
    await tap(d, a ? "Profile and settings" : "Settings", a ? {} : { type: "Button" })
    await scrollTap(d, "Import statements")
    await tap(d, "Lloyds Current Account")
    await waitFor(d, /changes/)
  })
  await shot("receipt-quick-upload", async () => {
    await fresh()
    await tab("Transactions")
    await tap(d, "Tesco Extra")
    await waitFor(d, "Fruit and vegetables")
  })
  if (!c.wide) {
    await shot("portfolio-top", async () => {
      await fresh()
      await tap(d, "1Y", { exact: true })
    })
    await shot("transactions-list", async () => {
      await fresh()
      await assetBuyDay()
    })
    await shot("currency-page", async () => {
      await fresh()
      await tap(d, await scrollTo(d, "Euro", { exact: true }).then((e) => e.label), { exact: true })
      await waitFor(d, "Revolut")
      await tap(d, "3M", { exact: true })
    })
    await shot("currency-usd", async () => {
      await fresh()
      await tap(d, await scrollTo(d, "US Dollar", { exact: true }).then((e) => e.label), { exact: true })
      await waitFor(d, "Coinbase")
      await tap(d, "1Y", { exact: true })
    })
    await shot("holding-detail", async () => {
      await fresh()
      await tap(d, await scrollTo(d, "Vanguard FTSE").then((e) => e.label))
      await toTop(d, await scrollTo(d, "My Position"), 0.2)
    })
    await shot("property", async () => {
      await fresh()
      await tap(d, await scrollTo(d, "Flat 12").then((e) => e.label))
      await waitFor(d, "Equity")
    })
    await shot("myra", async () => {
      await fresh()
      await openMyra()
    })
    await shot("myra-approval", async () => {
      await fresh()
      await tab("Myra")
      await tap(d, a ? "Menu" : "Conversations")
      await tap(d, "Lidl bill for yesterday")
      await waitFor(d, /Approval|Approve/)
      const { w, h } = await d.size()
      await d.drag(w * 0.5, h * 0.3, h * 0.42)
    })
    await shot("transaction-buy", async () => {
      await fresh()
      await openAssetBuy()
    })
    await shot("accounts", async () => {
      await fresh()
      await tab("Accounts")
      await waitFor(d, "Lloyds Current Account")
    })
    await shot("portfolio-all", async () => {
      await fresh()
      await tap(d, "ALL", { exact: true })
    })
  } else {
    await shot("two-pane-portfolio", async () => {
      await fresh()
      await tap(d, "1Y", { exact: true })
    })
    await shot("two-pane-transaction-buy", async () => {
      await fresh()
      await openAssetBuy()
    })
    await shot("two-pane-currency", async () => {
      await fresh()
      await tap(d, await scrollTo(d, "Euro", { exact: true }).then((e) => e.label), { exact: true })
      await waitFor(d, "Revolut")
      await tap(d, "3M", { exact: true })
    })
    await shot("two-pane-currency-usd", async () => {
      await fresh()
      await tap(d, await scrollTo(d, "US Dollar", { exact: true }).then((e) => e.label), { exact: true })
      await waitFor(d, "Coinbase")
      await tap(d, "1Y", { exact: true })
    })
    await shot("two-pane-account", async () => {
      await fresh()
      await tab("Accounts")
      await tap(d, await scrollTo(d, "Trading 212 ISA", {}, 15, 0.22).then((e) => e.label))
      await waitFor(d, /Cost Basis/i)
      await tap(d, "1Y", { exact: true })
    })
    await shot("two-pane-property", async () => {
      await fresh()
      await tab("Accounts")
      await tap(d, await scrollTo(d, "Flat 12", {}, 15, 0.22).then((e) => e.label))
      await waitFor(d, "Equity")
    })
    await shot("two-pane-myra", async () => {
      await fresh()
      await openMyra()
    })
  }
}

async function captureAndroid(device: Device, configs: Config[]) {
  const wasRunning = !!(await makeVar("avd-serial", device))
  if (!(await make("android-run", device))) {
    console.log("Install failed; cold-booting the emulator and retrying")
    const stale = await makeVar("avd-serial", device)
    if (stale) await $`${ADB} -s ${stale} emu kill`.nothrow()
    while (await makeVar("avd-serial", device)) await sleep(1000)
    const avd = await makeVar("avd-name", device)
    Bun.spawn([`${SDK}/emulator/emulator`, "-avd", avd, "-no-snapshot-load"], { stdout: "ignore", stderr: "ignore" }).unref()
    await sleep(5000)
    if (!(await make("android-run", device))) throw new Error(`make android-run DEVICE=${device} failed`)
  }
  const serial = await makeVar("avd-serial", device)
  const adb = (...args: string[]) => $`${ADB} -s ${serial} ${args}`.quiet()
  try {
    await adb("shell", "am", "force-stop", "io.appium.uiautomator2.server").nothrow()
    await adb("shell", "cmd", "uimode", "night", "yes")
    await adb("shell", "settings", "delete", "secure", "theme_customization_overlay_packages")
    await sleep(3000)
    await adb(
      "shell",
      `settings put secure theme_customization_overlay_packages '{"android.theme.customization.system_palette":"${BRAND_SEED}","android.theme.customization.accent_color":"${BRAND_SEED}","android.theme.customization.color_source":"preset","android.theme.customization.theme_style":"TONAL_SPOT"}'`,
    )
    await adb("shell", "settings", "put", "global", "sysui_demo_allowed", "1")
    const demo = (...args: string[]) => adb("shell", "am", "broadcast", "-a", "com.android.systemui.demo", "-e", "command", ...args)
    await demo("exit")
    await sleep(1000)
    await demo("enter")
    await sleep(1000)
    await demo("clock", "-e", "hhmm", "0941")
    await demo("battery", "-e", "level", "100", "-e", "plugged", "false")
    await demo("network", "-e", "wifi", "show", "-e", "level", "4", "-e", "fully", "true")
    await demo("network", "-e", "mobile", "show", "-e", "datatype", "3g", "-e", "level", "4", "-e", "fully", "true")
    await demo("notifications", "-e", "visible", "false")
    const notifications = (await adb("shell", "dumpsys", "notification", "--noredact")).stdout.toString()
    for (const key of new Set([...notifications.matchAll(/NotificationRecord\([^)]* key=(\S+?):? /g)].map((m) => m[1])))
      await adb("shell", `cmd notification snooze --for 86400000 '${key}'`).nothrow()
    await adb("shell", "settings", "put", "system", "accelerometer_rotation", "0")
    await adb("shell", "pm", "clear", APP)
    await sleep(5000)
    let display: string | undefined
    const d = android(serial, () => display)
    if (device === "fold") {
      await adb("emu", "unfold")
      display = await foldDisplay(serial, false)
    }
    if (configs[0].rotation !== undefined) await adb("shell", "settings", "put", "system", "user_rotation", `${configs[0].rotation}`)
    await d.fresh()
    await tap(d, "Next", { exact: true })
    await waitFor(d, "Holdings", {}, 30000)
    for (const c of configs) {
      if (device === "fold") {
        await adb("emu", c.folded ? "fold" : "unfold")
        await sleep(3000)
        await adb("shell", "input", "keyevent", "KEYCODE_WAKEUP")
        await adb("shell", "wm", "dismiss-keyguard")
        display = await foldDisplay(serial, !!c.folded)
      }
      if (c.rotation !== undefined) await adb("shell", "settings", "put", "system", "user_rotation", `${c.rotation}`)
      await sleep(2000)
      await walk(c, d)
    }
    if (device === "fold") await adb("emu", "unfold")
  } finally {
    if (!wasRunning) await adb("emu", "kill").nothrow()
  }
}

async function foldDisplay(serial: string, folded: boolean) {
  const out = (await $`${ADB} -s ${serial} shell dumpsys SurfaceFlinger --display-id`.quiet()).stdout.toString()
  for (const id of [...out.matchAll(/Display (\d+)/g)].map((m) => m[1])) {
    const png = pngFrom((await $`${ADB} -s ${serial} exec-out screencap -p -d ${id}`.quiet()).stdout)
    if ((png.readUInt32BE(16) === 1080) === folded) return id
  }
  throw new Error("fold display not found")
}

async function captureIos(device: Device, configs: Config[]) {
  const developerDir = await makeVar("developer-dir", device)
  const sh = (strings: TemplateStringsArray, ...values: any[]) => $(strings, ...values).env(env(developerDir)).quiet()
  const before = await makeVar("sim-udid", device)
  const wasBooted = !!before && (await sh`xcrun simctl list devices booted`.text()).includes(before)
  if (!(await make("ios-simulator", device))) throw new Error(`make ios-simulator DEVICE=${device} failed`)
  const udid = await makeVar("sim-udid", device)
  const agent = async () => {
    for (let i = 0; i < 3; i++) {
      await sh`${MOBILECLI} agent install --device ${udid}`.nothrow()
      if ((await sh`${MOBILECLI} device info --device ${udid}`.nothrow()).exitCode === 0) return true
      await sleep(10000)
    }
    return false
  }
  try {
    await sh`xcrun simctl uninstall ${udid} ${APP}`.nothrow()
    await sh`${MOBILECLI} daemon stop`.nothrow()
    if (!(await agent())) {
      console.log("  The mobilecli agent did not start; erasing the simulator and retrying")
      await sh`${MOBILECLI} daemon stop`.nothrow()
      await sh`xcrun simctl shutdown ${udid}`.nothrow()
      await sh`xcrun simctl erase ${udid}`
      if (!(await make("ios-simulator", device)) || !(await agent())) throw new Error("the mobilecli agent did not start after erasing the simulator")
    }
    if (!(await make("ios-run", device))) throw new Error(`make ios-run DEVICE=${device} failed`)
    await sh`xcrun simctl ui ${udid} appearance dark`
    await sh`xcrun simctl status_bar ${udid} override --time 9:41 --dataNetwork wifi --wifiMode active --wifiBars 3 --cellularMode active --cellularBars 4 --batteryState discharging --batteryLevel 100`
    let display: string | undefined
    const d = ios(udid, developerDir, () => display)
    if (device === "fold") {
      await duo(udid, developerDir, "angle", "180")
      await duo(udid, developerDir, "orient", "portrait")
      display = "3"
    }
    if (device === "tablet")
      for (let i = 0; ; i++) {
        if ((await sh`${MOBILECLI} device orientation set landscape --device ${udid}`.nothrow()).exitCode === 0) break
        if (i === 2) throw new Error("could not rotate the iPad to landscape")
        await sh`${MOBILECLI} daemon stop`.nothrow()
        await agent()
      }
    await d.elements()
    await d.fresh()
    await tap(d, "Next", { exact: true })
    await waitFor(d, "Holdings", {}, 30000)
    for (const c of configs) {
      if (device === "fold") {
        await duo(udid, developerDir, "angle", c.folded ? "0" : "180")
        display = c.folded ? "1" : "3"
      }
      await walk(c, d)
    }
    if (device === "fold") await duo(udid, developerDir, "angle", "180")
    if (device === "tablet") await $`${MOBILECLI} device orientation set portrait --device ${udid}`.env(env(developerDir)).quiet().nothrow()
  } finally {
    await sh`${MOBILECLI} agent uninstall --device ${udid}`.nothrow()
    await sh`${MOBILECLI} daemon stop`.nothrow()
    if (!wasBooted) await sh`xcrun simctl shutdown ${udid}`.nothrow()
  }
}

async function duo(udid: string, developerDir: string, ...args: string[]) {
  const helper = `${RAW}/duo_helper`
  if (!existsSync(helper))
    await $`xcrun --sdk iphonesimulator clang -fobjc-arc -mios-simulator-version-min=27.0 -framework Foundation -framework IOKit ${SHOTS}/duo_helper.m -o ${helper}`.env(
      env(developerDir),
    )
  await $`xcrun simctl spawn ${udid} ${helper} ${args}`.env(env(developerDir)).quiet()
  await sleep(4000)
}

const only = process.argv.slice(2)
const selected = CONFIGS.filter((c) => only.length === 0 || only.includes(c.id))
mkdirSync(RAW, { recursive: true })
for (const platform of ["android", "ios"] as Platform[]) {
  // The iPad's UI-testing agent fails to start right after the iPhone's session on the same iOS runtime, so it goes first.
  for (const device of (platform === "ios" ? ["tablet", "phone", "fold"] : ["phone", "tablet", "fold"]) as Device[]) {
    const configs = selected.filter((c) => c.platform === platform && c.device === device)
    if (!configs.length) continue
    for (let attempt = 1; attempt <= 2; attempt++) {
      console.log(`Capturing ${configs.map((c) => c.id).join(", ")}${attempt > 1 ? " (second attempt)" : ""}`)
      try {
        await (platform === "android" ? captureAndroid(device, configs) : captureIos(device, configs))
        break
      } catch (e) {
        const message = `${(e as Error).message} ${(e as any).stderr?.toString().trim() ?? ""}`.trim()
        console.log(`  FAILED ${platform} ${device}: ${message}`)
        await $`${MOBILECLI} daemon stop`.quiet().nothrow()
        if (attempt === 2) for (const c of configs) missed.push(`${c.id}: ${message}`)
      }
    }
  }
}
await $`${MOBILECLI} daemon stop`.quiet().nothrow()
await Bun.write(`${RAW}/missed.txt`, missed.join("\n"))
if (missed.length) console.log(`Missed ${missed.length} captures (raw/missed.txt):\n${missed.join("\n")}`)
