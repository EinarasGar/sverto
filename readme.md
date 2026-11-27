<p align="center">
  <img src="web/public/sverto.svg" alt="Sverto" width="120" />
</p>

<h1 align="center">Sverto</h1>

<p align="center">
  <strong>The open source personal finance app that puts you in control.</strong><br />
  Track your entire financial life — spending, investments, and net worth — all in one place.<br />
  <a href="https://sverto.com"><strong>Try it at sverto.com</strong></a>
</p>

<p align="center">
  <a href="https://apps.apple.com/app/sverto/id6819424663"><img src="marketing/landing/public/badges/app-store.svg" alt="Download on the App Store" height="40" /></a>
  &nbsp;
  <a href="https://play.google.com/store/apps/details?id=com.sverto.app"><img src="marketing/landing/public/badges/google-play.svg" alt="Get it on Google Play" height="40" /></a>
</p>

<p align="center">
  <a href="https://github.com/EinarasGar/sverto/stargazers"><img src="https://img.shields.io/github/stars/EinarasGar/sverto?style=social" alt="Stars" /></a>
  <a href="https://github.com/EinarasGar/sverto/commits/main"><img src="https://img.shields.io/github/last-commit/EinarasGar/sverto" alt="Last commit" /></a>
  <img src="https://img.shields.io/badge/self--hosted-ready-green" alt="Self-hosted ready" />
</p>

<p align="center">
  <img src=".github/assets/hero.png" alt="Sverto on web, iOS, and Android — native portfolio screens overlapping the current web dashboard, shown in dark mode with demo data" width="940" />
</p>

---

## Your finances, your way

Most finance apps force you into a box. Investment trackers ignore your daily spending. Budgeting apps can't handle your portfolio. You end up juggling three different tools, manually reconciling numbers, and never seeing the full picture.

**Sverto is different.** It's the only open source app that tracks everything in one unified view:

- **Daily spending** — every purchase, subscription, and bill
- **Investment portfolio** — stocks, ETFs, crypto, commodities
- **Net worth** — real-time and historical, across all your accounts

No vendor lock-in. No data mining. Your financial data stays on your machine.

---

## What you can do with Sverto

### Track everything

Record purchases, sales, dividends, transfers, and fees — from your morning coffee to an ETF dividend reinvestment.

### See your full financial picture

A real-time net worth chart across all accounts. Drill down by time range — last 24 hours, last month, or your entire history.

### Manage your investment portfolio

See every holding across every account. Track cost basis, unrealized and realized gains, dividends, and fees. Drill into individual positions.

### Ask your finances anything

Sverto comes with **Myra**, a built-in AI assistant that understands your data — ask questions in plain English or have it draft transactions for you. [More on Myra below.](#myra--ai-that-works-for-you)

### Organize your way

Create custom categories, manage multiple accounts (bank, brokerage, wallet, joint), and track assets with real-time market rates.

---

## Myra — AI that works for you

The AI assistant has full context of your financial data through a set of tools:

- **Semantic search** — find transactions by meaning, not just keywords
- **Aggregation** — ask "what did I spend by category?" and get real answers
- **Transaction creation** — the AI can draft transactions for your approval
- **Portfolio questions** — inquire about holdings, gains, and asset performance

The AI assistant uses Gemini and stores embeddings in your PostgreSQL database. When self-hosting, you'll need your own Gemini API key. Hosted users get AI and market data included.

> *Example: "Can you add a £24 Lidl bill to my joint account for yesterday?"*

<p align="center">
  <img src=".github/assets/android-myra.png" alt="Myra resolving the account, category, and currency, then asking for approval before saving the transaction" width="340" />
</p>

---

## Available everywhere

- **[Web](https://app.sverto.com)** — Full-featured web app. Run it on your own server or access it from any browser.
- **[Android](https://play.google.com/store/apps/details?id=com.sverto.app)** — Native Kotlin app with Jetpack Compose, powered by the shared Rust core.
- **[iOS](https://apps.apple.com/app/sverto/id6819424663)** — Native SwiftUI app, powered by the shared Rust core.

### Web

Net worth, accounts, and recent activity in one view.

<p align="center">
  <img src="marketing/landing/public/shots/ledger-dark.png" alt="Current Sverto web dashboard in dark mode, with net worth history, accounts, recent transactions, and investments" width="940" />
</p>

<details>
  <summary>Explore transactions and investments on the web</summary>
  <p align="center">
    <img src=".github/assets/web-transactions.png" alt="Current web transactions ledger in dark mode, with daily activity and transaction details" width="940" />
  </p>
  <p align="center">
    <img src=".github/assets/web-portfolio.png" alt="Current web investment portfolio in dark mode, with holdings, asset allocation, and performance" width="940" />
  </p>
</details>

### iOS

Portfolio history, everyday transactions, and individual purchase lots in the native iOS app.

<p align="center">
  <img src=".github/assets/ios-portfolio.png" alt="Native iOS portfolio with net worth history and holdings" width="30%" />
  <img src=".github/assets/ios-transactions.png" alt="Native iOS transaction feed with activity grouped by day and categorized entries" width="30%" />
  <img src=".github/assets/ios-asset-lots.png" alt="Native iOS asset detail with purchase lots and performance" width="30%" />
</p>

### Android

<p align="center">
  <img src=".github/assets/android-portfolio.png" alt="Net worth and holdings on Android" width="30%" />
  <img src=".github/assets/android-transactions.png" alt="Transactions feed on Android" width="30%" />
  <img src=".github/assets/android-asset-lots.png" alt="Asset detail with purchase lots and per-lot P&L on Android" width="30%" />
</p>

<p align="center"><sub>Screens show demo data. iOS and Android screenshots are generated by `make store-shots`; web screenshots captured in October 2026.</sub></p>

---

## Powered by modern technology

<p align="center">
  <img src="https://img.shields.io/badge/Rust-000000?style=for-the-badge&logo=rust&logoColor=white" alt="Rust" />
  <img src="https://img.shields.io/badge/React-20232A?style=for-the-badge&logo=react&logoColor=61DAFB" alt="React" />
  <img src="https://img.shields.io/badge/TypeScript-007ACC?style=for-the-badge&logo=typescript&logoColor=white" alt="TypeScript" />
  <img src="https://img.shields.io/badge/PostgreSQL-316192?style=for-the-badge&logo=postgresql&logoColor=white" alt="PostgreSQL" />
  <img src="https://img.shields.io/badge/OpenTelemetry-FFD23F?style=for-the-badge&logo=opentelemetry&logoColor=black" alt="OpenTelemetry" />
  <img src="https://img.shields.io/badge/Android-3DDC84?style=for-the-badge&logo=android&logoColor=white" alt="Android" />
  <img src="https://img.shields.io/badge/Swift-F05138?style=for-the-badge&logo=swift&logoColor=white" alt="Swift" />
</p>

The backend is written in Rust for speed and reliability. The web frontend uses React with TypeScript. Android uses Kotlin + Jetpack Compose, and iOS uses SwiftUI. Both native apps share the Rust core through UniFFI.

---

## Why open source matters for your finances

- **Your data stays yours.** Self-host and your financial data never leaves your server.
- **No subscriptions.** No monthly fees, no premium tiers. All features are free.
- **Transparent and auditable.** Every line of code is public — you can verify exactly what happens with your data.
- **Free for everyone.** AGPLv3 licensed — download it, run it, modify it however you want.
- **Community-driven roadmap.** Features are built based on what users actually need, not what maximizes revenue.

---

## Want the easy way?

Running open source software yourself can be rewarding, but it comes with friction — especially when you need market data feeds and AI API keys. **[sverto.com](https://sverto.com)** is the managed version of Sverto, with everything handled for you: automatic updates, live market data, and AI integration built in.

No setup. No API keys to manage. And every subscription directly supports Sverto's development.

The app collects only the telemetry needed to keep the service fast and reliable — error traces and performance metrics, nothing more. The website counts visits with cookie-free [Plausible](https://plausible.io/sverto.com), and the stats are public. Your financial data is never sold, shared, or mined for advertising.

→ [**Try it at sverto.com**](https://sverto.com)

---

→ [**Star on GitHub**][stars] &nbsp;|&nbsp; → [**Report an issue**][issues] &nbsp;|&nbsp; → [**Contribute**][pulls]

[stars]: https://github.com/EinarasGar/sverto/stargazers
[issues]: https://github.com/EinarasGar/sverto/issues
[pulls]: https://github.com/EinarasGar/sverto/pulls
