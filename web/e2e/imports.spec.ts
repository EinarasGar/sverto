import type { Page } from "@playwright/test"

import { deleteImportSession, isApiReachable } from "./support/api"
import { expect, gotoRoute, test } from "./support/fixtures"
import { IMPORTS } from "./support/routes"

const SESSION_PATH = /\/imports\/([0-9a-f-]{36})$/

function csv(name: string) {
  return {
    name,
    mimeType: "text/csv",
    buffer: Buffer.from(`date,description,amount\n2026-01-02,${name},-4.20\n`),
  }
}

function fileList(page: Page) {
  return page.getByRole("list", { name: "Files in this import" })
}

async function startImport(page: Page): Promise<string> {
  await page.getByRole("button", { name: "New import" }).first().click()
  await expect(page).toHaveURL(SESSION_PATH)
  const sessionId = SESSION_PATH.exec(page.url())?.[1]
  if (sessionId === undefined) throw new Error("no session id in the URL")
  return sessionId
}

test.describe("imports", () => {
  const created: string[] = []

  test.beforeEach(async ({ page }) => {
    test.skip(
      !(await isApiReachable()),
      "the API is unreachable, so no import session can be created"
    )
    await gotoRoute(page, IMPORTS)
  })

  test.afterEach(async () => {
    await Promise.all(created.splice(0).map(deleteImportSession))
  })

  test("a new import collects files, drops one, and is deleted", async ({
    page,
    consoleLog,
  }) => {
    const sessionId = await startImport(page)
    created.push(sessionId)

    const header = page.locator('[data-slot="page-header"]')
    await expect(header.getByText("Waiting for files")).toBeVisible()
    await expect(page.getByText("Add the files to import")).toBeVisible()

    await page
      .getByLabel("Choose files")
      .setInputFiles([csv("e2e-january.csv"), csv("e2e-february.csv")])

    await expect(fileList(page).getByText("e2e-january.csv")).toBeVisible()
    await expect(fileList(page).getByText("e2e-february.csv")).toBeVisible()
    await expect(
      fileList(page).getByText("Waiting to be read").first()
    ).toBeVisible()
    await expect(header.getByText("Reading files")).toBeVisible()

    const detached = page.waitForResponse(
      (response) =>
        response.request().method() === "DELETE" &&
        response.url().includes(`/import-sessions/${sessionId}/files/`)
    )
    await page.getByRole("button", { name: "Remove e2e-january.csv" }).click()
    await expect(fileList(page).getByText("e2e-january.csv")).toHaveCount(0)
    expect((await detached).status()).toBe(204)

    await page.reload()
    await expect(fileList(page).getByText("e2e-february.csv")).toBeVisible()
    await expect(fileList(page).getByText("e2e-january.csv")).toHaveCount(0)

    await page.getByRole("link", { name: "Imports" }).click()
    const row = page.locator(`a[href="/imports/${sessionId}"]`)
    await expect(row).toBeVisible()
    await expect(row.getByText("Reading files")).toBeVisible()

    await row.click()
    await page.getByRole("button", { name: "Delete import" }).click()
    await page
      .getByRole("alertdialog")
      .getByRole("button", { name: "Delete import" })
      .click()

    await expect(page).toHaveURL(/\/imports$/)
    await expect(page.locator(`a[href="/imports/${sessionId}"]`)).toHaveCount(0)
    await expect(page.getByText("Import deleted")).toBeVisible()

    expect(consoleLog.errors).toEqual([])
  })
})
