#!/usr/bin/env node
/**
 * AICrawl Playwright bridge.
 *
 * Renders one URL in headless Chromium and prints a single JSON line on stdout:
 *   { "ok": true, "html": "...", "final_url": "...", "status": 200, "title": "..." }
 * or on failure:
 *   { "ok": false, "error": "..." }
 *
 * This process is spawned by the Rust engine for `--render` requests only.
 * It does not persist cookies, does not solve CAPTCHAs and does not attempt to
 * bypass authentication, paywalls or bot protections.
 */
import { chromium } from 'playwright';

function parseArgs(argv) {
  const args = { url: null, timeout: 30000, waitUntil: 'domcontentloaded' };
  for (let i = 0; i < argv.length; i += 1) {
    const key = argv[i];
    if (key === '--url') args.url = argv[i + 1];
    else if (key === '--timeout') args.timeout = Number(argv[i + 1]);
    else if (key === '--wait-until') args.waitUntil = argv[i + 1];
  }
  return args;
}

function emit(payload, exitCode = 0) {
  process.stdout.write(`${JSON.stringify(payload)}\n`, () => process.exit(exitCode));
}

const args = parseArgs(process.argv.slice(2));
if (!args.url) {
  emit({ ok: false, error: 'missing --url' }, 2);
}

let browser;
try {
  browser = await chromium.launch({
    headless: true,
    args: ['--disable-dev-shm-usage', '--no-first-run'],
  });
  const context = await browser.newContext({
    userAgent:
      'Mozilla/5.0 (compatible; AICrawlBot/1.0; +https://github.com/aicrawl/aicrawl)',
    javaScriptEnabled: true,
    ignoreHTTPSErrors: false,
  });
  const page = await context.newPage();
  const response = await page.goto(args.url, {
    waitUntil: args.waitUntil,
    timeout: Math.max(5000, args.timeout),
  });
  await page.waitForLoadState('networkidle', { timeout: 5000 }).catch(() => {});
  const html = await page.content();
  const title = (await page.title()) || undefined;
  emit({
    ok: true,
    html,
    final_url: page.url(),
    status: response ? response.status() : 0,
    title,
  });
} catch (error) {
  emit({ ok: false, error: String(error && error.message ? error.message : error) }, 1);
} finally {
  if (browser) {
    await browser.close().catch(() => {});
  }
}
