-- Listed assets and FX have no market data locally: a deterministic random walk (setseed) from 2024-11-01 to now.
SELECT setseed(0.42);
CREATE TEMP TABLE _spec(ticker1 text, ticker2 text, p_start numeric, p_end numeric, vol numeric);
INSERT INTO _spec VALUES
  ('VWRP.LSE','GBP', 96, 134, 0.14),
  ('AAPL.NASDAQ','USD', 174, 236, 0.25),
  ('BTC','USD', 58000, 91500, 0.55),
  ('ETH','USD', 3150, 3020, 0.65),
  ('USD','GBP', 0.79, 0.762, 0.06),
  ('EUR','GBP', 0.84, 0.871, 0.05);
DO $$
DECLARE s record; pid int; d int; days int; w numeric := 0; frac numeric; g numeric; lp numeric; wend numeric; ws numeric[];
BEGIN
  days := (date_trunc('day', now())::date - date '2024-11-01');
  FOR s IN SELECT * FROM _spec LOOP
    SELECT p.id INTO pid FROM asset_pairs p JOIN assets a1 ON a1.id=p.pair1 JOIN assets a2 ON a2.id=p.pair2
      WHERE a1.ticker=s.ticker1 AND a2.ticker=s.ticker2 AND a1.user_id IS NULL;
    IF pid IS NULL THEN RAISE EXCEPTION 'no pair % -> %', s.ticker1, s.ticker2; END IF;
    ws := ARRAY[0]; w := 0;
    FOR d IN 1..days LOOP
      g := sqrt(-2*ln(GREATEST(random(),1e-12)))*cos(2*pi()*random());
      w := w + (s.vol/sqrt(365.0))*g; ws := ws || w;
    END LOOP;
    wend := ws[days+1];
    INSERT INTO asset_history(pair_id, rate, recorded_at) VALUES (pid, s.p_start, '2021-01-01T00:00:00Z') ON CONFLICT DO NOTHING;
    FOR d IN 0..days-1 LOOP
      frac := d::numeric/days;
      lp := ln(s.p_start) + frac*(ln(s.p_end)-ln(s.p_start)) + (ws[d+1] - frac*wend);
      INSERT INTO asset_history(pair_id, rate, recorded_at)
        VALUES (pid, round(exp(lp)::numeric, 6), (date '2024-11-01' + d)::timestamptz) ON CONFLICT DO NOTHING;
    END LOOP;
    INSERT INTO asset_history(pair_id, rate, recorded_at) VALUES (pid, s.p_end, date_trunc('hour', now())) ON CONFLICT DO NOTHING;
    RAISE NOTICE 'pair % (% -> %) seeded', pid, s.ticker1, s.ticker2;
  END LOOP;
END $$;

-- A euro life for the multi-currency shot: a Revolut account topped up from savings (a GBP transfer, then a GBP to EUR
-- trade at the day's rate, the same shapes the showcase seed uses), spent on a Paris weekend, a Lisbon fortnight and a
-- few recent online orders. Also a supermarket shop on the day of the latest ETF buy, so the transactions shot shows both.
DO $$
DECLARE
  uid uuid := '00000000-0000-0000-0000-000000000000';
  savings uuid := (SELECT id FROM account WHERE user_id = uid AND account_name = 'Marcus Savings');
  lloyds uuid := (SELECT id FROM account WHERE user_id = uid AND account_name = 'Lloyds Current Account');
  revolut uuid := gen_random_uuid();
  eur int := (SELECT id FROM assets WHERE ticker = 'EUR' AND user_id IS NULL);
  pair int;
  cat jsonb := (SELECT jsonb_object_agg(category, id) FROM transaction_categories WHERE user_id IS NULL OR user_id = uid);
  r record;
  t uuid;
  at timestamptz;
BEGIN
  SELECT id INTO pair FROM asset_pairs WHERE pair1 = eur AND pair2 = 45;
  INSERT INTO account (id, user_id, account_name, account_type, liquidity_type, ownership_share) VALUES (revolut, uid, 'Revolut', 1, 1, 1.0);

  FOR r IN SELECT * FROM (VALUES (150, 800), (121, 400), (91, 400), (60, 1200), (31, 400), (6, 400)) v(days, gbp) LOOP
    at := date_trunc('day', now()) - make_interval(days => r.days) + interval '9 hours 12 minutes';
    INSERT INTO transaction (user_id, type_id, date_transacted) VALUES (uid, 13, at) RETURNING id INTO t;
    INSERT INTO entry (transaction_id, asset_id, account_id, quantity, category_id) VALUES
      (t, 45, savings, -r.gbp, 79), (t, 45, revolut, r.gbp, 79);
    INSERT INTO transaction (user_id, type_id, date_transacted) VALUES (uid, 7, at + interval '1 minute') RETURNING id INTO t;
    INSERT INTO entry (transaction_id, asset_id, account_id, quantity, category_id) VALUES
      (t, 45, revolut, -r.gbp, 12),
      (t, eur, revolut, round(r.gbp / (SELECT rate FROM asset_history WHERE pair_id = pair AND recorded_at <= at ORDER BY recorded_at DESC LIMIT 1), 2), 12);
    INSERT INTO transaction_descriptions (transaction_id, description) VALUES (t, 'Exchanged GBP to EUR');
  END LOOP;

  -- Trips are anchored to calendar months (Paris three months back, Lisbon two) so last month never has euro groceries
  -- and Myra's pound-only grocery answer stays complete; the online orders are counted back from today.
  FOR r IN SELECT * FROM (VALUES
      (3, 14, '10:40', 'Monoprix', 'Groceries', 23.80), (3, 14, '13:05', 'Bouillon Chartier', 'Restaurants', 38.40),
      (3, 14, '16:20', 'RATP', 'Public Transport', 16.90), (3, 15, '09:15', 'Café de Flore', 'Cafes & Coffee', 11.50),
      (3, 15, '18:30', 'Franprix', 'Groceries', 12.35),
      (2, 9, '14:00', 'Lisbon flat - rent share', 'Rent', 640.00), (2, 9, '19:45', 'Bolt', 'Ride Sharing & Taxis', 8.90),
      (2, 10, '09:10', 'Fábrica Coffee Roasters', 'Cafes & Coffee', 4.80), (2, 10, '11:30', 'Navegante metro pass', 'Public Transport', 40.00),
      (2, 10, '18:20', 'Pingo Doce', 'Groceries', 34.60), (2, 11, '12:15', 'Continente', 'Groceries', 56.40),
      (2, 11, '20:30', 'Taberna da Rua das Flores', 'Restaurants', 42.50), (2, 12, '10:05', 'Pastéis de Belém', 'Cafes & Coffee', 6.30),
      (2, 12, '17:50', 'Pingo Doce', 'Groceries', 18.25), (2, 13, '15:40', 'CP Comboios Sintra', 'Public Transport', 4.60),
      (2, 14, '09:30', 'Copenhagen Coffee Lab', 'Cafes & Coffee', 5.20), (2, 14, '13:10', 'Time Out Market', 'Restaurants', 27.80),
      (2, 14, '18:45', 'Pingo Doce', 'Groceries', 41.10), (2, 16, '10:20', 'A Brasileira', 'Cafes & Coffee', 3.90),
      (2, 16, '21:00', 'Cervejaria Ramiro', 'Restaurants', 58.20), (2, 17, '17:35', 'Pingo Doce', 'Groceries', 22.80),
      (2, 18, '07:50', 'Bolt', 'Ride Sharing & Taxis', 11.40)
    ) v(months_back, day, hm, description, category, amount) LOOP
    at := date_trunc('month', now()) - make_interval(months => r.months_back, days => 1 - r.day) + r.hm::time;
    INSERT INTO transaction (user_id, type_id, date_transacted) VALUES (uid, 1, at) RETURNING id INTO t;
    INSERT INTO entry (transaction_id, asset_id, account_id, quantity, category_id) VALUES (t, eur, revolut, -r.amount, (cat->>r.category)::int);
    INSERT INTO transaction_descriptions (transaction_id, description) VALUES (t, r.description);
  END LOOP;

  FOR r IN SELECT * FROM (VALUES
      (14, '20:10', 'Booking.com - Porto', 'Accommodation', 186.00), (9, '12:30', 'Zalando', 'Clothing & Accessories', 59.95),
      (4, '19:05', 'Vinted', 'Clothing & Accessories', 18.50)
    ) v(days, hm, description, category, amount) LOOP
    at := date_trunc('day', now()) - make_interval(days => r.days) + r.hm::time;
    INSERT INTO transaction (user_id, type_id, date_transacted) VALUES (uid, 1, at) RETURNING id INTO t;
    INSERT INTO entry (transaction_id, asset_id, account_id, quantity, category_id) VALUES (t, eur, revolut, -r.amount, (cat->>r.category)::int);
    INSERT INTO transaction_descriptions (transaction_id, description) VALUES (t, r.description);
  END LOOP;

  SELECT date_trunc('day', max(date_transacted)) + interval '18 hours 5 minutes' INTO at FROM transaction WHERE user_id = uid AND type_id = 9;
  INSERT INTO transaction (user_id, type_id, date_transacted) VALUES (uid, 1, at) RETURNING id INTO t;
  INSERT INTO entry (transaction_id, asset_id, account_id, quantity, category_id) VALUES (t, 45, lloyds, -42.18, (cat->>'Groceries')::int);
  INSERT INTO transaction_descriptions (transaction_id, description) VALUES (t, 'Sainsbury''s');
END $$;

-- The assistant shot shows a fixed Myra conversation instead of calling a model. The history is rig's session log
-- (rig-session/4) as the ai crate saves it; every figure comes from this ledger, grouped the way aggregate_transactions does.
DO $$
DECLARE
  uid uuid := '00000000-0000-0000-0000-000000000000';
  month_start date := date_trunc('month', now() - interval '1 month')::date;
  month_end date := (date_trunc('month', now()) - interval '1 day')::date;
  rows jsonb;
  total numeric;
  n int;
  shops text;
  answer text;
  history text;
  cid uuid;
BEGIN
  CREATE TEMP TABLE _month ON COMMIT DROP AS
    SELECT COALESCE(td.description, tg.description, 'No description') AS shop, tc.category, e.quantity, t.id
    FROM entry e
    JOIN transaction t ON e.transaction_id = t.id
    LEFT JOIN transaction_categories tc ON e.category_id = tc.id
    LEFT JOIN transaction_descriptions td ON td.transaction_id = t.id
    LEFT JOIN transaction_group tg ON tg.id = t.group_id
    WHERE t.user_id = uid AND e.asset_id = 45 AND t.date_transacted >= month_start AND t.date_transacted < month_end + 1;

  SELECT jsonb_agg(jsonb_build_object('group_name', shop, 'total_amount', amount, 'transaction_count', count) ORDER BY amount)
  INTO rows FROM (SELECT shop, sum(quantity) AS amount, count(DISTINCT id) AS count FROM _month GROUP BY shop) g;

  SELECT -sum(quantity), count(DISTINCT id) INTO total, n FROM _month WHERE category = 'Groceries';

  SELECT string_agg(format('| %s | £%s |', shop, to_char(spent, 'FM999,999,990.00')), E'\n' ORDER BY spent DESC)
  INTO shops
  FROM (SELECT shop, -sum(quantity) AS spent FROM _month WHERE category = 'Groceries' GROUP BY shop ORDER BY spent DESC LIMIT 3) top;

  answer := format(E'You spent **£%s** on groceries in %s, across %s transactions.\n\n| Shop | Spent |\n|---|---|\n%s',
                   to_char(total, 'FM999,999,990.00'), trim(to_char(month_start, 'Month')), n, shops);

  history := concat_ws(E'\n',
    '{"seq":0,"n":0,"t":@@T@@,"type":"header","format":"rig-session/4","log":"history","lineage":13821117240934033555}',
    '{"seq":1,"n":1,"t":@@T@@,"type":"push","entries":[{"message":{"role":"user","content":[{"type":"text","text":"How much did I spend on groceries last month?"}]},"id":0}]}',
    '{"seq":2,"n":2,"t":@@T@@,"type":"push","entries":[{"message":{"role":"assistant","content":[{"type":"toolcall","id":{"provider":"call_1"},"function":{"name":"aggregate_transactions","arguments":{"group_by":"description","date_from":"@@FROM@@","date_to":"@@TO@@"}}}]},"id":1}]}',
    '{"seq":3,"n":3,"t":@@T@@,"type":"push","entries":[{"message":{"role":"user","content":[{"type":"toolresult","call":{"provider":"call_1"},"name":"aggregate_transactions","content":[{"type":"text","text":"@@RESULT@@"}]}]},"id":2}]}',
    '{"seq":4,"n":4,"t":@@T@@,"type":"push","entries":[{"message":{"role":"assistant","content":[{"type":"text","text":"@@ANSWER@@"}]},"id":3}]}',
    '');
  history := replace(history, '@@T@@', (extract(epoch FROM now()) * 1000)::bigint::text);
  history := replace(history, '"@@FROM@@"', to_json(month_start::text)::text);
  history := replace(history, '"@@TO@@"', to_json(month_end::text)::text);
  history := replace(history, '"@@RESULT@@"', to_json(jsonb_build_object('currency', 'GBP', 'rows', rows)::text)::text);
  history := replace(history, '"@@ANSWER@@"', to_json(answer)::text);

  INSERT INTO ai_conversations (user_id, history) VALUES (uid, convert_to(history, 'UTF8')) RETURNING id INTO cid;
  INSERT INTO ai_chat (conversation_id, title) VALUES (cid, 'Groceries last month');
END $$;

-- A second Myra conversation that stops at an approval: the user asks for a Lidl bill on the joint account, Myra looks
-- up the account, category and currency, then calls create_transaction, a gated tool. The stream pauses there and the
-- history ends in that call with no result, which is what the apps turn back into the "Approval required" card.
DO $$
DECLARE
  uid uuid := '00000000-0000-0000-0000-000000000000';
  joint uuid := (SELECT id FROM account WHERE user_id = uid AND account_name = 'Joint Bills - Starling');
  groceries int := (SELECT id FROM transaction_categories WHERE user_id IS NULL AND category = 'Groceries');
  accounts jsonb;
  categories jsonb;
  assets jsonb;
  history text;
  cid uuid;
BEGIN
  SELECT jsonb_agg(jsonb_build_object('account_id', a.id, 'account_name', a.account_name, 'account_type', t.account_type_name,
                                      'liquidity_type', l.liquidity_type_name, 'ownership_share', a.ownership_share, 'identifiers', '[]'::jsonb)
                   ORDER BY a.account_name)
  INTO accounts
  FROM account a JOIN account_types t ON t.id = a.account_type JOIN account_liquidity_types l ON l.id = a.liquidity_type
  WHERE a.user_id = uid;
  SELECT jsonb_agg(jsonb_build_object('id', c.id, 'category', c.category, 'category_type', ct.category_type_name, 'icon', c.icon))
  INTO categories
  FROM transaction_categories c JOIN transaction_category_type ct ON ct.id = c.category_type
  WHERE c.user_id IS NULL AND c.category IN ('Groceries', 'Alcohol & Bars', 'Home & Household');
  SELECT jsonb_build_object('matches', jsonb_agg(jsonb_build_object('id', id, 'asset_name', asset_name, 'ticker', ticker)), 'similar', '[]'::jsonb)
  INTO assets
  FROM assets WHERE id = 45;

  history := concat_ws(E'\n',
    '{"seq":0,"n":0,"t":@@T@@,"type":"header","format":"rig-session/4","log":"history","lineage":13821117240934033556}',
    '{"seq":1,"n":1,"t":@@T@@,"type":"push","entries":[{"message":{"role":"user","content":[{"type":"text","text":"Can you add a £24 Lidl bill to my joint account for yesterday?"}]},"id":0}]}',
    '{"seq":2,"n":2,"t":@@T@@,"type":"push","entries":[{"message":{"role":"assistant","content":['
      || '{"type":"toolcall","id":{"provider":"call_1"},"function":{"name":"list_accounts","arguments":{}}},'
      || '{"type":"toolcall","id":{"provider":"call_2"},"function":{"name":"search_categories","arguments":{"query":"groceries"}}},'
      || '{"type":"toolcall","id":{"provider":"call_3"},"function":{"name":"search_assets","arguments":{"query":"GBP"}}}]},"id":1}]}',
    '{"seq":3,"n":3,"t":@@T@@,"type":"push","entries":[{"message":{"role":"user","content":['
      || '{"type":"toolresult","call":{"provider":"call_1"},"name":"list_accounts","content":[{"type":"text","text":"@@ACCOUNTS@@"}]},'
      || '{"type":"toolresult","call":{"provider":"call_2"},"name":"search_categories","content":[{"type":"text","text":"@@CATEGORIES@@"}]},'
      || '{"type":"toolresult","call":{"provider":"call_3"},"name":"search_assets","content":[{"type":"text","text":"@@ASSETS@@"}]}]},"id":2}]}',
    '{"seq":4,"n":4,"t":@@T@@,"type":"push","entries":[{"message":{"role":"assistant","content":['
      || '{"type":"toolcall","id":{"provider":"call_4"},"function":{"name":"create_transaction","arguments":@@CREATE@@}}]},"id":3}]}',
    '');
  history := replace(history, '@@T@@', (extract(epoch FROM now() - interval '1 hour') * 1000)::bigint::text);
  history := replace(history, '"@@ACCOUNTS@@"', to_json(accounts::text)::text);
  history := replace(history, '"@@CATEGORIES@@"', to_json(categories::text)::text);
  history := replace(history, '"@@ASSETS@@"', to_json(assets::text)::text);
  history := replace(history, '@@CREATE@@', jsonb_build_object('date', (now()::date - 1)::text, 'description', 'Lidl', 'amount', -24,
                                                               'account_id', joint, 'category_id', groceries, 'asset_id', 45)::text);

  INSERT INTO ai_conversations (user_id, history, created_at, updated_at)
  VALUES (uid, convert_to(history, 'UTF8'), now() - interval '1 hour', now() - interval '1 hour') RETURNING id INTO cid;
  INSERT INTO ai_chat (conversation_id, title, updated_at) VALUES (cid, 'Lidl bill for yesterday', now() - interval '1 hour');
END $$;

-- A receipt quick upload at "ready", in the shape the quick_upload job saves (proposal_type transaction_group).
DO $$
DECLARE
  uid uuid := '00000000-0000-0000-0000-000000000000';
  account text := (SELECT id::text FROM account WHERE user_id = uid AND account_name = 'Lloyds Current Account');
  day text := now()::date::text;
  file uuid;
  cid uuid;
  cat jsonb := (SELECT jsonb_object_agg(category, id) FROM transaction_categories WHERE user_id IS NULL OR user_id = uid);
BEGIN
  INSERT INTO user_files (user_id, original_name, mime_type, size_bytes, status, storage_key)
  VALUES (uid, 'receipt.jpg', 'image/jpeg', 182344, 'ready', 'store-shots/receipt.jpg') RETURNING id INTO file;
  INSERT INTO ai_conversations (user_id) VALUES (uid) RETURNING id INTO cid;
  INSERT INTO ai_workflow_quick_upload (conversation_id, status, source_file_id, proposal_type, proposal_data)
  VALUES (cid, 'proposal_ready', file, 'transaction_group', jsonb_build_object(
    'description', 'Tesco Extra',
    'date', day,
    'category_id', cat->'Groceries',
    'transactions', jsonb_build_array(
      jsonb_build_object('description', 'Fruit and vegetables', 'date', day, 'account_id', account, 'amount', '-14.60', 'asset_id', 45, 'category_id', cat->'Groceries'),
      jsonb_build_object('description', 'Milk, bread and eggs', 'date', day, 'account_id', account, 'amount', '-7.85', 'asset_id', 45, 'category_id', cat->'Groceries'),
      jsonb_build_object('description', 'Washing-up liquid and bin bags', 'date', day, 'account_id', account, 'amount', '-6.40', 'asset_id', 45, 'category_id', cat->'Home & Household'),
      jsonb_build_object('description', 'Paracetamol', 'date', day, 'account_id', account, 'amount', '-2.15', 'asset_id', 45, 'category_id', cat->'Pharmacy & Medication'),
      jsonb_build_object('description', 'Bottle of red wine', 'date', day, 'account_id', account, 'amount', '-9.50', 'asset_id', 45, 'category_id', cat->'Alcohol & Bars')
    )));
END $$;

-- A statement import at "Ready to review", in the shape the parse and classify jobs leave it: one parsed Lloyds CSV
-- covering the last few days, its account and currency mapped, every line classified. Most lines are already in the
-- ledger, four are new, and the biggest grocery shop has its pence digits swapped against the ledger (51.50 / 51.05),
-- a typo the matcher offers to amend.
DO $$
DECLARE
  uid uuid := '00000000-0000-0000-0000-000000000000';
  lloyds uuid := (SELECT id FROM account WHERE user_id = uid AND account_name = 'Lloyds Current Account');
  cat jsonb := (SELECT jsonb_object_agg(category, id) FROM transaction_categories WHERE user_id IS NULL OR user_id = uid);
  ref text := 'Lloyds 30-94-57 12345678';
  first_day date := now()::date - 2;
  amended uuid;
  correction numeric;
  cid uuid;
  sid uuid;
  src uuid;
  file uuid;
  run uuid;
  line record;
  lid uuid;
BEGIN
  SELECT t.id, ((pence % 10) * 10 + pence / 10 - pence) / -100.0 INTO amended, correction
  FROM entry e JOIN transaction t ON t.id = e.transaction_id JOIN transaction_categories tc ON tc.id = e.category_id,
       LATERAL (SELECT (-e.quantity * 100)::int % 100 AS pence) p
  WHERE e.account_id = lloyds AND tc.category = 'Groceries' AND t.date_transacted >= first_day - 4 AND pence % 10 <> pence / 10
  ORDER BY e.quantity LIMIT 1;
  first_day := LEAST(first_day, (SELECT date_transacted::date - 1 FROM transaction WHERE id = amended));
  INSERT INTO ai_conversations (user_id) VALUES (uid) RETURNING id INTO cid;
  INSERT INTO import_sessions (user_id, conversation_id) VALUES (uid, cid) RETURNING id INTO sid;
  INSERT INTO import_sources (session_id, kind) VALUES (sid, 'file') RETURNING id INTO src;
  INSERT INTO user_files (user_id, original_name, mime_type, size_bytes, status, storage_key)
    VALUES (uid, 'lloyds-statement.csv', 'text/csv', 4182, 'ready', 'store-shots/lloyds-statement.csv') RETURNING id INTO file;
  INSERT INTO import_files (session_id, file_id, conversation_id, status, source_id) VALUES (sid, file, cid, 'parsed', src);
  INSERT INTO import_account_mappings (session_id, source_id, account_ref, account_id) VALUES (sid, src, ref, lloyds);
  INSERT INTO import_asset_mappings (session_id, asset_ref, asset_id) VALUES (sid, 'GBP', 45);
  INSERT INTO import_classification_runs (session_id, classified_at) VALUES (sid, now()) RETURNING id INTO run;
  FOR line IN
    SELECT t.date_transacted AS day,
           upper(COALESCE(td.description, 'Transfer to ' || other.account_name)) AS description,
           CASE WHEN t.id = amended THEN e.quantity + correction ELSE e.quantity END AS amount,
           CASE WHEN t.type_id <> 13 THEN t.type_id WHEN e.quantity < 0 THEN 2 ELSE 3 END AS type_id,
           CASE WHEN t.type_id <> 13 THEN e.category_id END AS category_id
    FROM entry e
    JOIN transaction t ON t.id = e.transaction_id
    LEFT JOIN transaction_descriptions td ON td.transaction_id = t.id
    LEFT JOIN LATERAL (SELECT a.account_name FROM entry o JOIN account a ON a.id = o.account_id
                       WHERE o.transaction_id = t.id AND o.account_id <> lloyds LIMIT 1) other ON true
    WHERE e.account_id = lloyds AND e.asset_id = 45 AND t.date_transacted >= first_day
    UNION ALL
    SELECT now()::date, v.description, v.amount, 1, (cat->>v.category)::int
    FROM (VALUES ('AMAZON MKTPLACE PMTS', -23.99, 'Shopping'), ('TFL TRAVEL CH', -8.40, 'Public Transport'),
                 ('NETFLIX.COM', -10.99, 'Subscriptions'), ('BRITISH GAS', -86.00, 'Electricity')) v(description, amount, category)
  LOOP
    INSERT INTO import_parsed_lines (session_id, source_id, account_ref, date_transacted, description)
      VALUES (sid, src, ref, line.day, line.description) RETURNING id INTO lid;
    INSERT INTO import_parsed_legs (parsed_line_id, asset_ref, quantity) VALUES (lid, 'GBP', line.amount);
    INSERT INTO import_line_classifications (run_id, parsed_line_id, type_id, category_id) VALUES (run, lid, line.type_id, line.category_id);
  END LOOP;
  INSERT INTO import_assertions (session_id, source_id, account_ref, asset_ref, kind, as_of_date, quantity)
    SELECT sid, src, ref, 'GBP', 'closing', now()::date, sum(quantity) - 23.99 - 8.40 - 10.99 - 86.00 + correction
    FROM entry WHERE account_id = lloyds AND asset_id = 45;
END $$;
