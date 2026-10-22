import { LandingFooter } from "../landing/landing-footer"
import { LandingHeader } from "../landing/landing-header"

import "./privacy-policy.css"

const POLICY_SECTIONS = [
  { id: "scope", label: "Who we are" },
  { id: "data", label: "Information we process" },
  { id: "purposes", label: "How we use information" },
  { id: "providers", label: "Providers and recipients" },
  { id: "ai", label: "AI features and on-device suggestions" },
  { id: "locations", label: "Processing locations" },
  { id: "retention", label: "Retention and deletion" },
  { id: "security", label: "Security and device storage" },
  { id: "rights", label: "Your choices and rights" },
  { id: "changes", label: "Updates and contact" },
] as const

export function PrivacyPolicy() {
  return (
    <div className="min-h-dvh bg-background text-ink">
      <a className="privacy-skip-link" href="#privacy-policy">
        Skip to privacy policy
      </a>
      <LandingHeader />
      <main className="privacy-policy" id="privacy-policy">
        <header className="privacy-heading">
          <p className="privacy-eyebrow">Privacy at Sverto</p>
          <h1>Privacy policy</h1>
          <p className="privacy-lead">
            How we handle your account, financial records and device
            information.
          </p>
          <p className="privacy-date">
            Last updated: <time dateTime="2026-10-22">22 October 2026</time> ·
            MB Sverto
          </p>
        </header>
        <div className="privacy-layout">
          <nav
            className="privacy-contents"
            aria-labelledby="privacy-contents-title"
          >
            <h2 id="privacy-contents-title">In this policy</h2>
            <ol>
              {POLICY_SECTIONS.map(({ id, label }) => (
                <li key={id}>
                  <a href={`#${id}`}>{label}</a>
                </li>
              ))}
            </ol>
            <a className="privacy-deletion-link" href="#delete-account">
              Request account deletion <span aria-hidden="true">↗</span>
            </a>
          </nav>
          <article className="privacy-copy" aria-label="Sverto privacy policy">
            <section id="scope">
              <h2>1. Who we are and what this policy covers</h2>
              <p>
                <strong>MB Sverto</strong>, registered at{" "}
                <strong>Naugarduko g. 68-3, Vilnius, Lithuania</strong>,
                operates Sverto’s hosted service and is the controller of the
                personal information we process to provide that service. Contact
                us at <a href="mailto:privacy@sverto.com">privacy@sverto.com</a>{" "}
                with questions or requests concerning your information.
              </p>
              <p>
                This policy covers the Sverto website, hosted application and
                mobile apps when used with our hosted service. Sverto helps you
                record and understand your financial activity, investments and
                net worth.
              </p>
              <h3>Adults only</h3>
              <p>
                Sverto is intended for people aged <strong>18 and over</strong>{" "}
                and is not directed at children. If you believe someone under 18
                has provided personal information to our hosted service, contact
                us using the address above so we can investigate and arrange
                appropriate deletion.
              </p>
              <h3>Self-hosted installations</h3>
              <p>
                If you connect to a self-hosted server, the operator of that
                server controls the financial records and other information sent
                to it. Records sent only to that server are not received by MB
                Sverto. The server operator’s configuration and policies
                determine its hosting, authentication, AI processing and
                retention.
              </p>
              <p>
                Some mobile SDKs can still communicate with their providers
                independently of the selected server. In particular, if you turn
                on Android’s optional on-device AI, its ML Kit SDK can send
                technical and usage telemetry to Google. Contacting our support
                or visiting our website also involves processing separate from
                your self-hosted server.
              </p>
            </section>

            <section id="data">
              <h2>2. Information we process</h2>
              <ul>
                <li>
                  <strong>Account information:</strong> account identifiers,
                  email address, profile name when supplied, authentication and
                  session information, and preferences such as your reference
                  currency and whether AI features are turned on.
                </li>
                <li>
                  <strong>Financial records:</strong> accounts, balances,
                  transactions, dates, descriptions, categories, holdings,
                  purchase lots, prices, fees, financial institution details and
                  account identifiers you enter or import, such as IBANs or
                  account numbers.
                </li>
                <li>
                  <strong>Trading 212 connection:</strong> if you choose to
                  connect your brokerage account, the API credentials, account
                  details and investment activity needed to import its records.
                  If you choose to save credentials for continued access, we
                  retain them in the configured secret store until the
                  connection or account is deleted.
                </li>
                <li>
                  <strong>Uploads and AI interactions:</strong> receipts,
                  images, documents, filenames, Myra messages, responses, tool
                  results and your decisions about proposed changes.
                </li>
                <li>
                  <strong>Search requests:</strong> search terms sent to our
                  service to locate transactions, assets or other information.
                  Searches made in Myra can also form part of your saved
                  conversation.
                </li>
                <li>
                  <strong>Technical and usage information:</strong> request
                  times and routes, response status, errors, service
                  performance, account/resource identifiers included in
                  diagnostics, and information processed by authentication and
                  device SDKs.
                </li>
                <li>
                  <strong>Authentication security information:</strong> Clerk
                  processes IP addresses and device/session information and can
                  derive approximate city/country information from the IP
                  address. This does not require GPS access.
                </li>
                <li>
                  <strong>Optional Android ML Kit telemetry:</strong> after you
                  turn on on-device AI, app and device information, installation
                  and other identifiers, feature-use events, performance metrics
                  and errors relating to that feature.
                </li>
                <li>
                  <strong>Communications:</strong> information you choose to
                  send when contacting us about support, privacy or other
                  requests.
                </li>
              </ul>
              <p>
                You choose which financial records and attachments to provide
                and whether to use optional features. Some account and technical
                information is needed to operate and protect the hosted service.
              </p>
            </section>

            <section id="purposes">
              <h2>3. Why we use information</h2>
              <p>
                We process information to authenticate users; store, synchronise
                and display their records; calculate balances and investment
                performance; search information; operate requested imports,
                connections and AI features; prevent abuse; diagnose failures;
                and respond to requests.
              </p>
              <p>
                Where the GDPR applies, the lawful bases for these activities
                are:
              </p>
              <ul>
                <li>
                  <strong>Providing the requested service:</strong> processing
                  necessary to perform our agreement with you, including account
                  access, financial tracking and features you request.
                </li>
                <li>
                  <strong>Legitimate interests:</strong> keeping the service
                  secure and reliable, preventing misuse, investigating problems
                  and handling communications, subject to your rights and
                  reasonable expectations.
                </li>
                <li>
                  <strong>Legal obligations:</strong> processing required to
                  meet an applicable legal requirement.
                </li>
                <li>
                  <strong>Consent:</strong> where required for a particular
                  optional activity, including sending information to Google’s
                  Gemini API for AI features (section 5). We explain that
                  activity when requesting consent; you can withdraw it at any
                  time without affecting earlier lawful processing.
                </li>
              </ul>
              <h3>Advertising and sale of data</h3>
              <p>
                Sverto does not display third-party advertising. We do not sell
                or rent your personal information, share it with advertisers or
                data brokers, or use your financial records to build advertising
                profiles.
              </p>
            </section>

            <section id="providers">
              <h2>4. Providers and other recipients</h2>
              <p>
                Information is processed by providers used to deliver Sverto:
              </p>
              <dl className="privacy-details">
                <div>
                  <dt>Railway</dt>
                  <dd>
                    Application hosting, databases, caches and file storage,
                    including account records, financial data, uploads and
                    operational logs.
                  </dd>
                </div>
                <div>
                  <dt>Axiom</dt>
                  <dd>
                    Operational logs, traces, errors and performance information
                    used to diagnose problems and maintain the service.
                    Diagnostic records can include account or resource
                    identifiers.
                  </dd>
                </div>
                <div>
                  <dt>Clerk</dt>
                  <dd>
                    Hosted authentication, account identity, sessions and
                    sign-in security information.
                  </dd>
                </div>
                <div>
                  <dt>Google Gemini API</dt>
                  <dd>
                    Cloud AI processing for Myra, receipt/document processing
                    and semantic search, only after you turn on AI features,
                    including relevant text, attachments, financial context and
                    generated output.
                  </dd>
                </div>
                <div>
                  <dt>Google ML Kit</dt>
                  <dd>
                    Optional on-device AI on Android, currently used to suggest
                    categories, and the SDK’s technical and usage telemetry
                    after you turn it on.
                  </dd>
                </div>
                <div>
                  <dt>Trading 212</dt>
                  <dd>
                    Optional retrieval of investment account history using your
                    API credentials, including orders, dividends and cash
                    movements. Your broker continues to process its own records
                    under your relationship with it.
                  </dd>
                </div>
                <div>
                  <dt>Google Workspace</dt>
                  <dd>
                    Email delivery and storage for our support/privacy address,
                    including message content, attachments, sender/recipient
                    details and delivery metadata.
                  </dd>
                </div>
              </dl>
              <p>
                We may also disclose information when required by law or
                necessary to protect legal rights, investigate abuse or respond
                to a valid authority request. Any disclosure must have an
                applicable legal basis.
              </p>
              <p>
                These providers process information to deliver the services
                described above. Providers that process personal information on
                our behalf do so under written terms, such as data processing
                agreements, that require them to protect it with safeguards at
                least equal to those described in this policy. Some also use
                specific information for their own purposes, such as account
                administration, security or SDK telemetry, as explained in their
                privacy notices.
              </p>
            </section>

            <section id="ai">
              <h2>5. AI features and on-device suggestions</h2>
              <p>
                Myra chat, receipt and document scanning, and the semantic
                search that supports them use Google’s Gemini API. These AI
                features are off until you turn them on. Sverto asks during
                setup or when you first try to use one of them, and explains
                what will be sent to Google before you choose. You can turn them
                on or off at any time in Settings → AI features, and you can use
                the rest of Sverto without them.
              </p>
              <p>
                When AI features are on, Myra sends your request and the
                relevant context needed to answer it to the Gemini API. Context
                can include conversation history, attachments and financial
                records retrieved by Myra’s tools, such as account, transaction,
                holding and category details. Receipts and documents you scan
                are sent to the Gemini API to extract transaction details. Your
                conversations and proposed actions are stored so you can return
                to them.
              </p>
              <p>
                While AI features are on, semantic search also uses the Gemini
                API to turn text, such as transaction descriptions and search
                queries, into numerical representations. This happens when
                descriptions are saved or updated, separately from opening a
                Myra conversation. While AI features are off, descriptions are
                not sent to Google for this purpose.
              </p>
              <p>
                Turning AI features off stops Sverto from sending new
                information to the Gemini API, including processing that is
                still waiting to run. It does not delete conversations, scan
                results or search representations already stored in your Sverto
                account. You can delete conversations separately, and account
                deletion removes the rest. Information Google already received
                is subject to the safety retention described below.
              </p>
              <p>
                The hosted service uses the Gemini API under paid-service data
                terms. Google states that paid-service prompts and responses are
                not used to improve its products. Separate safety processing
                still applies: its published abuse-monitoring policy describes
                retaining prompts, context and outputs for 55 days, with
                restricted human review for suspected misuse. See{" "}
                <a href="https://ai.google.dev/gemini-api/terms">
                  Gemini API terms
                </a>{" "}
                and{" "}
                <a href="https://ai.google.dev/gemini-api/docs/usage-policies">
                  abuse monitoring
                </a>
                .
              </p>
              <p>
                On-device AI on Android is off by default and can be turned on
                in Settings → On-device AI. It currently suggests categories for
                new transactions. The choice is stored on this installation and
                is excluded from backup restoration. You can keep choosing
                categories yourself without it. It runs on the device using ML
                Kit. This local feature does not itself send its input or
                generated output to Google servers, but the SDK sends technical
                and usage metrics for performance, debugging, maintenance, API
                improvement and abuse prevention. This is governed by{" "}
                <a href="https://developers.google.com/ml-kit/terms">
                  ML Kit’s terms
                </a>{" "}
                and{" "}
                <a href="https://policies.google.com/privacy">
                  Google’s Privacy Policy
                </a>
                , separately from paid Gemini processing. Saving a financial
                record to your hosted account is a separate server operation.
                Turning on-device AI off stops new feature requests and cancels
                pending suggestion work. It does not delete telemetry Google
                already received or guarantee cancellation of metrics already
                queued by the SDK.
              </p>
            </section>

            <section id="locations">
              <h2>6. Where information is processed</h2>
              <p>
                Our primary Railway application services, PostgreSQL database,
                Redis service and uploaded-file bucket are hosted in EU West
                (Amsterdam). Our Axiom logs and traces are stored in US East, so
                some information is processed outside the EU.
              </p>
              <p>
                Authentication, AI and other providers can process information
                in additional countries under their service arrangements.
                Self-hosted processing locations depend on the server operator’s
                choices.
              </p>
              <p>
                International transfer arrangements depend on the provider,
                service and destination. Axiom’s published data-processing terms
                include EU standard contractual clauses. Clerk’s and Google’s
                published terms describe the EU–US Data Privacy Framework where
                applicable and standard contractual clauses for relevant
                transfers. These mechanisms address international transfers;
                they do not mean processing stays within the EU. You can request
                information about the safeguards relevant to your data at{" "}
                <a href="mailto:privacy@sverto.com">privacy@sverto.com</a>.
              </p>
              <p>
                Provider terms:{" "}
                <a href="https://axiom.co/docs/legal/data-processing">
                  Axiom DPA
                </a>
                , <a href="https://clerk.com/legal/dpa">Clerk DPA</a>,{" "}
                <a href="https://business.safety.google/processorterms/">
                  Google processor terms
                </a>
                .
              </p>
            </section>

            <section id="retention">
              <h2>7. Retention and deletion</h2>
              <p>
                Different types of information have different lifetimes.
                Deleting a record from the active service does not necessarily
                erase operational logs, backups or provider safety records at
                the same moment.
              </p>
              <dl className="privacy-details">
                <div>
                  <dt>Hosted account and financial records</dt>
                  <dd>
                    We retain your account and financial records while your
                    account is active. Account deletion removes your profile and
                    associated financial and conversation records from the
                    active application database. The separate retention periods
                    below apply to operational records and provider copies.
                  </dd>
                </div>
                <div>
                  <dt>Conversations, uploads and exports</dt>
                  <dd>
                    Conversations can be deleted. Deleting a conversation does
                    not itself delete its separately stored files. Account
                    deletion removes the associated database records and
                    requests removal of stored uploads, thumbnails and exports.
                    Storage failures can prevent file removal.
                  </dd>
                </div>
                <div>
                  <dt>Clerk identity and connector secrets</dt>
                  <dd>
                    Account deletion requests removal of your authentication
                    identity and saved connection credentials during the
                    deletion request. Failed external deletions are not
                    automatically retried. You can contact us at{" "}
                    <a href="mailto:privacy@sverto.com">privacy@sverto.com</a>{" "}
                    to ask us to investigate incomplete deletion.
                  </dd>
                </div>
                <div>
                  <dt>Background job records</dt>
                  <dd>
                    Background processing records can include transaction
                    descriptions or correction text. They do not have an
                    automatic expiry and are not automatically removed when a
                    task finishes or an account is deleted. They remain until
                    separately deleted. You can ask us to include these records
                    in a deletion request using the contact details below.
                  </dd>
                </div>
                <div>
                  <dt>Axiom logs and traces</dt>
                  <dd>
                    Logs and traces are retained for 30 days in our Axiom
                    dataset.
                  </dd>
                </div>
                <div>
                  <dt>Railway logs</dt>
                  <dd>
                    Our hosting plan provides access to the most recent 7 days
                    of logs. This is an access window, not a guaranteed deletion
                    deadline: Railway may retain older logs under its own
                    service arrangements.
                  </dd>
                </div>
                <div>
                  <dt>Redis operational data</dt>
                  <dd>
                    AI usage counters and cached per-user limits expire within
                    32 days after the relevant update. They can remain after
                    account deletion until their expiry.
                  </dd>
                </div>
                <div>
                  <dt>Railway volume backups</dt>
                  <dd>
                    Backup copies can remain after information is deleted from
                    the active service. Their retention depends on the
                    applicable backup settings and the provider’s recovery
                    arrangements. Active account deletion does not immediately
                    erase these copies, and we do not promise a fixed erasure
                    deadline for them.
                  </dd>
                </div>
                <div>
                  <dt>Uploaded-file bucket</dt>
                  <dd>
                    Files have no automatic age-based expiry. Account deletion
                    requests removal of uploads, thumbnails and exports, with
                    repeated cleanup as described above.
                  </dd>
                </div>
                <div>
                  <dt>Gemini safety records</dt>
                  <dd>
                    Google’s published abuse-monitoring policy specifies 55 days
                    for prompts, contextual information and outputs. These
                    records are separate from conversations stored by Sverto.
                  </dd>
                </div>
                <div>
                  <dt>Support and privacy correspondence</dt>
                  <dd>
                    Ordinary support correspondence is deleted 12 months after
                    resolution. Minimal records needed to demonstrate handling
                    of a privacy request are retained for up to 3 years after
                    closure. Unnecessary identity documents and attachments are
                    removed earlier. Information needed for an actual dispute or
                    applicable legal obligation is retained for that need, then
                    deleted.
                  </dd>
                </div>
              </dl>
              <div className="privacy-deletion" id="delete-account">
                <h3>How to delete your Sverto account</h3>
                <p>
                  You can request account deletion in Sverto’s Settings. You can
                  also email{" "}
                  <a href="mailto:privacy@sverto.com?subject=Sverto%20account%20deletion">
                    privacy@sverto.com
                  </a>{" "}
                  with the subject “Sverto account deletion” and the email
                  address associated with your account. You do not need the app
                  installed to make this request.
                </p>
                <p>
                  We may need to verify that you control the account. Please do
                  not send passwords or authentication tokens. Deletion covers
                  your hosted account and its associated records, subject to the
                  separate retention and deletion arrangements explained above.
                </p>
              </div>
            </section>

            <section id="security">
              <h2>8. Security and information on your device</h2>
              <p>
                The hosted service uses HTTPS for public app/API connections and
                authenticated access to user records. Provider credentials are
                handled through server-side secret storage. File access uses
                authenticated operations or time-limited links.
              </p>
              <p>
                The apps can keep local preferences, session information and
                cached records to support their operation, including offline
                access where available. Information retained only on your device
                is distinct from information sent to our service. A self-hosted
                server’s transport security and access controls depend on its
                configuration.
              </p>
              <p>
                No system can guarantee absolute security. Contact us if you
                suspect unauthorised access to your information.
              </p>
            </section>

            <section id="rights">
              <h2>9. Your choices and rights</h2>
              <p>
                Depending on applicable law, you may have rights to access,
                correct, delete or receive a portable copy of your information;
                restrict or object to processing; and withdraw consent where
                processing relies on it. Some rights have legal exceptions.
              </p>
              <p>
                Contact{" "}
                <a href="mailto:privacy@sverto.com">privacy@sverto.com</a> to
                exercise a right or raise a concern. You can complain to
                Lithuania’s{" "}
                <a href="https://vdai.lrv.lt/en/">
                  State Data Protection Inspectorate (VDAI)
                </a>
                , or to the data-protection authority in the country where you
                live, work or believe an infringement occurred.
              </p>
              <p>
                For records held exclusively by a self-hosted installation,
                contact that installation’s operator. We can address information
                MB Sverto itself processes but cannot administer a server we do
                not operate.
              </p>
            </section>

            <section id="changes">
              <h2>10. Updates and contact</h2>
              <p>
                We will update this policy when our processing practices change
                and update the date at the top of this page. Material changes
                will be communicated as required by applicable law.
              </p>
              <address>
                <strong>MB Sverto</strong>
                <br />
                Naugarduko g. 68-3
                <br />
                Vilnius, Lithuania
                <br />
                <a href="mailto:privacy@sverto.com">privacy@sverto.com</a>
              </address>
            </section>
          </article>
        </div>
      </main>
      <LandingFooter />
    </div>
  )
}
