import { COMPANY } from "../landing/company"
import { LandingFooter } from "../landing/landing-footer"
import { LandingHeader } from "../landing/landing-header"
import { LANDING_LICENCE_URL, LANDING_SOURCE_URL } from "../landing/links"

import "../legal/legal-document.css"

const TERMS_SECTIONS = [
  { id: "scope", label: "Who we are" },
  { id: "account", label: "Your account" },
  { id: "service", label: "The service and its price" },
  { id: "advice", label: "Not financial advice" },
  { id: "records", label: "Your records and connections" },
  { id: "ai", label: "Myra and AI features" },
  { id: "use", label: "Acceptable use" },
  { id: "source", label: "Open source and self-hosting" },
  { id: "ending", label: "Suspension and ending" },
  { id: "liability", label: "Our responsibility to you" },
  { id: "changes", label: "Changes to these terms" },
  { id: "law", label: "Governing law and complaints" },
  { id: "contact", label: "Contact" },
] as const

export function TermsOfService() {
  return (
    <div className="min-h-dvh bg-background text-ink">
      <a className="legal-skip-link" href="#terms-of-service">
        Skip to terms of service
      </a>
      <LandingHeader />
      <main className="legal-document" id="terms-of-service">
        <header className="legal-heading">
          <p className="legal-eyebrow">Terms at Sverto</p>
          <h1>Terms of service</h1>
          <p className="legal-lead">
            The agreement between you and us when you use Sverto’s hosted
            service.
          </p>
          <p className="legal-date">
            Last updated: <time dateTime="2026-11-25">25 November 2026</time> ·{" "}
            {COMPANY.name}
          </p>
        </header>
        <div className="legal-layout">
          <nav
            className="legal-contents"
            aria-labelledby="terms-contents-title"
          >
            <h2 id="terms-contents-title">In these terms</h2>
            <ol>
              {TERMS_SECTIONS.map(({ id, label }) => (
                <li key={id}>
                  <a href={`#${id}`}>{label}</a>
                </li>
              ))}
            </ol>
          </nav>
          <article className="legal-copy" aria-label="Sverto terms of service">
            <section id="scope">
              <h2>1. Who we are and what these terms cover</h2>
              <p>
                Sverto’s hosted service is provided by{" "}
                <strong>{COMPANY.name}</strong>, a {COMPANY.legalForm} with
                company code <strong>{COMPANY.code}</strong> and its registered
                office at <strong>{COMPANY.address}</strong>. Data about the
                company is held in the {COMPANY.register}. You can reach us at{" "}
                <a href={`mailto:${COMPANY.contactEmail}`}>
                  {COMPANY.contactEmail}
                </a>
                .
              </p>
              <p>
                These terms cover the hosted service: the Sverto website, the
                hosted application and the mobile apps when they are used with
                an account on our servers. They form an agreement between you
                and {COMPANY.name}, which you accept when you create an account
                or continue to use the hosted service.
              </p>
              <p>
                Our <a href="/privacy">privacy policy</a> explains how we handle
                your personal information. It is a separate notice, not a
                contractual term.
              </p>
            </section>

            <section id="account">
              <h2>2. Your account</h2>
              <p>
                You must be <strong>18 or over</strong> to hold an account.
              </p>
              <p>
                Keep your sign-in details to yourself and tell us promptly if
                you think someone else has accessed your account. You are
                responsible for what is done through your account unless it
                results from our failure to keep the service secure.
              </p>
            </section>

            <section id="service">
              <h2>3. The service and its price</h2>
              <p>
                Sverto lets you record and understand your financial activity,
                investments and net worth. The hosted service is currently{" "}
                <strong>free of charge</strong>.
              </p>
              <p>
                We expect to introduce an optional paid subscription later. We
                will tell you before that happens, and you will not be charged
                anything unless you expressly agree to a price shown to you at
                that time.
              </p>
              <p>
                We develop Sverto continuously, so features can be added,
                changed or removed, and the service can be unavailable during
                maintenance or because of faults. We do not promise
                uninterrupted availability.
              </p>
            </section>

            <section id="advice">
              <h2>4. Not financial advice</h2>
              <p>
                Sverto is a record-keeping and analysis tool.{" "}
                <strong>
                  We are not a bank, a broker, an investment adviser or a tax
                  adviser
                </strong>
                , and nothing in the service is a personal recommendation to
                buy, sell or hold any asset.
              </p>
              <p>
                Balances, cost basis, gains, performance and net worth are
                calculated from the records you provide and from market prices
                and exchange rates supplied by third parties. Market prices are
                end-of-day and can be late, missing or wrong, and a calculation
                can only be as accurate as the records behind it. Check figures
                against your original statements before relying on them, in
                particular for tax returns and investment decisions, and keep
                those statements: Sverto does not replace them.
              </p>
            </section>

            <section id="records">
              <h2>5. Your records and connected accounts</h2>
              <p>
                The records you put into Sverto remain yours. You give us
                permission to store and process them only as needed to provide
                the service to you.
              </p>
              <p>
                You are responsible for the records you enter, import or upload,
                and for having the right to provide them, including any
                information about other people that they contain.
              </p>
              <p>
                If you connect an account you hold with another provider, such
                as a broker, you confirm that you are entitled to do so and you
                authorise us to retrieve its records on your behalf. Sverto only
                reads from connected accounts; it never moves money or places
                orders. Your relationship with that provider stays subject to
                its own terms, including any rules about API access.
              </p>
            </section>

            <section id="ai">
              <h2>6. Myra and AI features</h2>
              <p>
                Myra and the other AI features are optional and stay off until
                you turn them on. The privacy policy explains what is sent to
                the AI provider when they are on.
              </p>
              <p>
                AI output can be incomplete or wrong. Myra proposes entries and
                changes for you to approve, and statement and receipt reading
                produces drafts for you to review. You are responsible for
                checking them before you accept them.
              </p>
              <p>
                We can apply usage limits to AI features to keep the service
                available to everyone.
              </p>
            </section>

            <section id="use">
              <h2>7. Acceptable use</h2>
              <p>When using the hosted service, you must not:</p>
              <ul>
                <li>use it for anything unlawful;</li>
                <li>
                  try to access other people’s accounts or data, or probe,
                  bypass or weaken the security of the service;
                </li>
                <li>
                  upload malicious code, or content you have no right to
                  provide;
                </li>
                <li>
                  place an unreasonable load on the service, for example with
                  automated requests beyond normal use of the apps;
                </li>
                <li>
                  use AI features to generate unlawful or abusive content, or to
                  get around the AI provider’s usage rules;
                </li>
                <li>
                  resell the hosted service or offer it to others as your own.
                </li>
              </ul>
              <p>
                If you find a security problem, please report it to{" "}
                <a href={`mailto:${COMPANY.contactEmail}`}>
                  {COMPANY.contactEmail}
                </a>{" "}
                rather than making use of it.
              </p>
            </section>

            <section id="source">
              <h2>8. Open source and self-hosting</h2>
              <p>
                The Sverto software is{" "}
                <a href={LANDING_SOURCE_URL}>published as open source</a> under
                the <a href={LANDING_LICENCE_URL}>AGPL-3.0 licence</a>. Nothing
                in these terms limits the rights that licence gives you in the
                software.
              </p>
              <p>
                If you run Sverto on your own server, your use of the software
                is governed by that licence, not by these terms. We do not
                operate, support or have access to self-hosted installations,
                and the software is provided without warranty as the licence
                states.
              </p>
            </section>

            <section id="ending">
              <h2>9. Suspension and ending the agreement</h2>
              <p>
                You can stop using Sverto and delete your account at any time in
                Settings or by writing to us. The privacy policy explains what
                deletion removes and how to obtain a copy of your information.
              </p>
              <p>
                We can suspend or close an account that breaks these terms or
                puts the service or other people at risk. We will tell you why
                and, unless the law or the urgency of the risk prevents it, give
                you notice and the opportunity to take your records with you
                first.
              </p>
              <p>
                If we decide to stop offering the hosted service, we will give
                you advance notice by email so that you can take your records
                with you.
              </p>
            </section>

            <section id="liability">
              <h2>10. Our responsibility to you</h2>
              <p>
                We provide the hosted service with reasonable care and skill. It
                is provided free of charge, and beyond that commitment we do not
                promise that it will be free of errors or that every figure it
                shows will be correct.
              </p>
              <p>
                Nothing in these terms excludes or limits our liability for
                intent or gross negligence, for death or personal injury, or for
                anything else that cannot be excluded or limited by law. If you
                are a consumer, nothing in these terms takes away rights that
                mandatory consumer law gives you.
              </p>
              <p>Subject to the paragraph above, we are not liable for:</p>
              <ul>
                <li>
                  losses arising from financial, investment or tax decisions
                  made in reliance on figures shown in Sverto;
                </li>
                <li>
                  market prices, exchange rates or account records from third
                  parties that are inaccurate, late or missing;
                </li>
                <li>
                  interruptions caused by third-party providers or by events
                  outside our reasonable control;
                </li>
                <li>
                  loss of profit or opportunity, or losses that were not a
                  foreseeable result of our breach of these terms.
                </li>
              </ul>
            </section>

            <section id="changes">
              <h2>11. Changes to these terms</h2>
              <p>
                We can change these terms when the service, its price or the law
                changes. We will update the date at the top of this page and,
                for a change that materially affects you, tell you by email or
                in the app before it takes effect. If you do not agree with a
                change, you can delete your account before it applies;
                continuing to use the service afterwards means you accept the
                new terms.
              </p>
            </section>

            <section id="law">
              <h2>12. Governing law and complaints</h2>
              <p>
                These terms are governed by the law of the Republic of
                Lithuania. If you are a consumer, you also keep the protection
                of the mandatory rules of the country where you live, and you
                can bring a claim in the courts of that country.
              </p>
              <p>
                If something is wrong, write to{" "}
                <a href={`mailto:${COMPANY.contactEmail}`}>
                  {COMPANY.contactEmail}
                </a>{" "}
                first; we will reply within 14 days. If you are a consumer and
                we cannot settle your complaint, you can apply to Lithuania’s{" "}
                <a href="https://vvtat.lrv.lt/en/">
                  State Consumer Rights Protection Authority (VVTAT)
                </a>
                , which resolves consumer disputes out of court, or to the
                consumer dispute body in the country where you live.
              </p>
            </section>

            <section id="contact">
              <h2>13. Contact</h2>
              <address>
                <strong>{COMPANY.name}</strong>
                <br />
                Company code {COMPANY.code}
                <br />
                {COMPANY.address}
                <br />
                <a href={`mailto:${COMPANY.contactEmail}`}>
                  {COMPANY.contactEmail}
                </a>
              </address>
            </section>
          </article>
        </div>
      </main>
      <LandingFooter />
    </div>
  )
}
