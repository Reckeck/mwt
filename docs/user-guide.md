# User guide

How to use Multi Wallet Tracker (MWT). Sections are added together with features.

## Wallets and sub-accounts

Wallets in the sidebar are grouped by type: **Cash · Banks · Crypto · Exchanges ·
Property · Loans**. Empty groups are hidden; a group shows its total and a "+" for a new
wallet of that type. A right click on a wallet gives quick actions: transaction,
sub-account, settings, automations, archive.

- **Cash** is created together with the vault and there is always just one: a second
  cash wallet is not needed, and it cannot be archived or deleted. Different currencies are
  "pocket" sub-accounts.
- **Banks, crypto wallets, exchanges, property, loans** are added by you.

### Create a wallet

**"Add wallet"** at the bottom of the list (or the "+" of a group). First you choose a type —
each has a one-line explanation — then only the fields needed: for an exchange, which
exchange it is; for a crypto wallet, hot or cold; for property, what it is;
for a loan, the terms and collateral. The optional **"Opening balance"** field is how much is already there: it is recorded
as a single "Opening balance" transaction, which is neither income nor spending.

### Sub-accounts: a strict binding

A sub-account is a part of a wallet of the same type that holds **exactly what it is
bound to**. The templates depend on the wallet type:

| Wallet | Sub-accounts | Binding |
|---|---|---|
| Cash | Pocket | currency |
| Bank | Card, Account, Deposit, Savings, Credit card | currency |
| Crypto | Token | token and network |
| Exchange | Spot, Futures, Earn, Trading pair | market or pair |

A trading pair (for example, **BTCUSDT**) holds both coins and exchanges them in both
directions. A transaction with a foreign asset will not go through: "The transaction does not match the
sub-account's binding". Nesting is one level: a sub-account has no sub-accounts of its own.

A sub-account is created in **Wallet settings → "Sub-accounts and envelopes"** or with a right
click on the wallet. It can be moved only to another wallet of the same type.

### Envelopes

An envelope sets part of the money aside for a goal **without moving it**: the money stays in
the wallet but is marked as taken. "Put aside" takes from the free balance,
"Release" returns to it. If a spend would dip into the money set aside, the transaction form
warns you and the envelope shows the shortfall. An envelope can be linked to a goal — its
progress equals the amount in the envelope.

### What the money is for (instead of "Count in the budget")

In wallet settings you choose **what this money is for**:

| Purpose | What it means | Example |
|---|---|---|
| Everyday | spending and income show in the daily budget | cash, a salary card, a credit card for purchases |
| Savings | moving money here is "put aside", not "spent" | a deposit, a savings account |
| Investment | trades and price swings stay out of the budget but count in net worth | an exchange, a crypto wallet, a flat |
| Not mine / transit | neither in the budget nor in net worth | a sole-proprietor account for taxes, a shared family account, a friend's money |

The purpose is filled in by itself from the type and template (a deposit — savings), and
rarely needs changing. Loans are never part of the budget.

### Wallet settings

The gear on the wallet page opens its settings. The tabs depend on the
type: **General** (name, color, purpose, contact), **Sub-accounts and envelopes**,
**Automations**, **Exchange** (choose an exchange, then keys and the import interval),
**Property** and **Utilities** (for property), **Terms** (for a loan or credit card),
**Danger zone** (archive and deletion).

### Exchange and terminal

First the exchange is chosen in settings (for now Bybit is supported, the others
are marked "soon") — only after that can API keys be pasted. An exchange has
three sections on its page: **Overview**, **Terminal**, **Import**. The terminal is part of
that exchange: the coin list, chart drawings, timeframe and indicators
are stored in it, so two exchanges have two independent terminals. There is no separate
"Trading" menu item: without an exchange a terminal is not needed.

Right click on the chart: **an alert at the cursor price** (the "above" or
"below" condition is set relative to the current price), drawing tools,
copying the price and deleting the coin's drawings. Alerts created in the terminal
belong to that exchange and are visible under the coin list; the Calendar shows all of them.

The **"Exchange check"** widget can be added to the dashboard. It is in the catalog
only while the vault has an exchange with a chosen provider. If there are several exchanges,
the catalog asks which one to show.

### Archive and deletion

- **Archiving** a wallet moves it and all its sub-accounts to the archive. A sub-account
  cannot be restored while its parent is archived — restore the parent first.
- Only an empty wallet can be **deleted**: no transactions and no sub-accounts. A wallet with
  history should be archived. Cash cannot be archived or deleted.

## Transactions

The **"Transaction"** button on a wallet page opens the form; the wallet is already
filled in, the asset is the main one for that wallet (in an empty bank — the base
currency). The **"+ Transaction"** button in the header works from any section: the first
field is the wallet (the last one is remembered).

| Kind | When |
|---|---|
| **Top up** | Money came from outside: salary, cashback, a gift. For a "Property" wallet — "Acquire" |
| **Pay** | Money is gone for good: a purchase, a bill, loan interest |
| **Transfer** | One asset between two places, at least one of them yours: between your own wallets, to a person, to an exchange |
| **Convert** | You give one thing and get another: currency exchange at a bank or an exchanger |
| Spot trade, Futures, DeFi, Sell property | Special — only for suitable wallets |

- "Top up" and "Pay" require a category. If you do not choose one,
  the app takes it from the rules (Settings → "Category rules"), for example
  "taxi" → "Taxi".
- The fee is deducted from the wallet separately; the recipient of a transfer receives exactly
  the amount stated.
- A transfer between sub-accounts of one wallet is marked **"internal"**: it is neither
  income nor spending.
- Any transaction can be edited in full — the pencil icon in the list.
- The price is fixed **at the rate on the transaction date**: under the "Price" field you can see which
  day's rate will be recorded. If you enter a price by hand, it does not change.
- The **"without a rate"** mark — no rate was found for the transaction; at the next
  rate refresh the price is filled in by itself and the mark disappears.

## Contacts

**Transactions → Contacts and debts** — people, companies and exchangers with whom money
moves. Everything is edited right in the card, without windows:

- **New contact** — type the name in the field above the list and press Enter.
- **Fields** — click and edit; it saves by itself when the field loses focus.
- **A requisite** — just paste it: a card, IBAN, phone, link or
  crypto address. The type and network (Ethereum, Tron, Bitcoin, Solana, TON…) are detected
  by themselves and can be corrected.
- **The filter** above the list — "All / Owe me / I owe"; next to the name — the amount of
  debt.
- A right click on a contact — transfer, "Lent", "Borrowed", copying a
  requisite, archive.
- A contact that has transactions is not deleted — it can be archived.

### Debts with people

The **"Debts"** tab in the card: how much is owed to you and how much you owe, per
currency. The buttons **"Lent / Borrowed / Repaid / Got it back"**
open a transfer with the right kind of debt. The amount is computed from those transactions, and for
each debt you can set a **return date** and a note — an overdue debt
is visible in the Calendar.

## Loans and debts

- **A loan, mortgage, credit card** — a wallet of the "Loan" type. You received a loan
  on a card — "Transfer" from the loan wallet to the card; a purchase with a credit card —
  "Pay" from it; a payment — "Transfer" from the card to the loan wallet. The debt
  is visible as a negative balance and in "Debts" on the Dashboard.
- **Debts with people** — an ordinary transfer to a contact with the "Is it a debt?" mark: lent,
  borrowed, repaid, got it back. The amount appears in the contact card
  ("owes me / I owe") and in net worth, but not in income and spending.
- **Property** — a "Property" wallet: "Acquire" with a unit price, later
  a value change in the positions list. The valuation applies to the whole history.

## Categories and budget

Settings → **"Categories"**: each category has a group — income,
mandatory, everyday, savings or bookkeeping. The group decides where a spend
lands in the budget; bookkeeping ones (trading, transfers, exchange) are not part of the budget.
This is also where the **savings percentage**, the **budget currency** and the **daily budget mode** are.

The daily budget includes wallets with the purpose **"Everyday"** — see "What the money
is for" above.

A category in the transaction form is chosen with a suggestion field: start typing —
the list is filtered. If there is no such category, the **"Create «…»"** item creates
it at once, without a trip to settings.

## Planning

The **"Calendar"** section is an overview: upcoming payments, limits, goals, reminders and
alerts. **Automations are created in a wallet**: Wallet settings →
"Automations". It also has ready-made templates for the wallet type: salary and a card
payment for a bank, DCA for an exchange and a crypto wallet, rent and tax for
property, a loan payment for a loan. A template picks the transaction kind and category by itself —
what is left is the amount and the date.
Days and months are counted in the time zone from Settings → "Categories and budget"
(by default — as in the system); the budget month is the calendar month.

### Salary automation, a limit and the daily budget

1. **Automations → "New automation"**: the name "Salary", the wallet — a
   card, "Every month", day 10, kind "Top up", an amount and the category
   "Salary". The transaction will be created by itself on the 10th at 9:00.
2. Create "Rent" in the same way (kind "Pay", category "Rent / mortgage").
   **Before an expense is created the balance is checked** of the wallet on its date. If there is not
   enough, the transaction is not created, and a notification arrives "Rent 15.10 was not
   created: on Tinkoff 8,000 of 25,000". The run waits for a decision:
   **"Retry"** (the balance is checked now, the transaction gets today's date)
   or **"Skip"**. Rejected runs are not retried by themselves.
   "Upcoming" marks an expense in advance with "Not enough: N".
3. **Limits → "New limit"**: for example, "Groceries" for a month, 30,000 RUB. A limit
   can be in any currency — spending is converted at the rate on the transaction
   date. A badge appears in the transaction form if a spend exceeds the limit.
4. **The daily budget** on the Dashboard is how much can be spent today. There are two modes
   (Settings → "Daily budget"):
   - **"Month, carrying over the rest"** (default): `(income − mandatory −
     planned savings) / days in the month`. Income and mandatory are the larger of
     the actual and the automations' plan. Underspending is carried to the following
     days, overspending reduces them; from the 1st the balance starts anew.
   - **"Until the next income"**: the free balance of wallets in the budget minus
     mandatory payments before the next salary, divided by the days until it.
     With no salary automation — until the end of the month.

   Only wallets with the purpose "Everyday" are counted. A transfer from an exchange
   to a card tops up the budget, it is not income.

### Goals: how much to put aside and is it realistic

**Goals → "New goal"** (or "⋯ → Make a goal" on a wallet page): amount,
currency, start, deadline, importance ⭐1–5, "can put aside a month" and
the wallets where the savings are. In the card:

- **Saved** — the current value of those wallets; **put in by you** — how much
  you put there at the rate of the day of the contribution; **rate difference** — the difference
  between them (for crypto it rises and falls with the rate).
- **Needed a month** — `(amount − saved) / remaining months`.
- **✅ Realistic / ⚠️ Revise** — with a reason: more is needed than you are willing to
  put aside, or all goals together require more than usually
  remains for you in a month (the average of the last 6 non-empty months).
- **At this pace** — when the goal will be reached at the pace of the last 90
  days; "not growing" if the pace is not rising.

If two goals share a wallet, its balance is split in proportion to the planned
contributions; no goal gets more than its amount. Below the cards is a
**savings calendar**: how much needed to be put aside in each month and how much
was put aside.

### Month totals

A card on the Dashboard: income, mandatory and everyday spending, savings
as planned (`income × savings percentage`) and actual (`income − mandatory −
everyday`), the share put aside. Debt transfers and transfers between your own
wallets in the budget are not part of the totals. It separately shows how many automation
runs were not executed.

### Reminders and alerts

A reminder and a price alert arrive as a system notification while the app is
open and the vault is unlocked. An alert fires once; after editing
it is armed again.

## Analytics

The **"Analytics"** section opens with an **overview**: four cards for the month —
**Income and spending**, **What I have**, **Investments** (since the start of the year) and
**Year**. Each has the main figures, a conclusion in words ("Put aside 25 % of income",
"Spending is 12 % higher than last month") and a **"More"** link to the
right section. An empty card suggests the first step.

In detail — three sections:

- **Income and spending** with a **By month / Year / Categories** switch.
  By month — income, spending and what remains; transfers between your own wallets
  and debts do not appear here. Year — like an annual budget: savings as planned and
  actual, deviation, the best and worst month. Categories — the five biggest
  spends with their share and a "category × month" matrix with the change against the same previous
  period; the average is computed over months where there was spending.
- **What I have** — net worth (money, property, "owed to me", loans
  and "I owe") by month and a table **by wallet**: value at the start and
  end, how much came in and went out. The **"In today's money"** switch
  recomputes past months for inflation.
- **Investments** — what the portfolio consists of, trade results and fees, losses
  on exchanges with a list of chains.

Above each report is a conclusion in words, under the chart — **"Month by month"**.
Hover a column name — an explanation appears. A month, category or
chart bar opens **Transactions** with that filter. On a narrow screen
the table shows the main columns, the rest are behind the arrow of the row.

The filter at the top is shared by all sections: a quick period (12 months, this
month, 3 months, year to date, last year) or your own months, one or
several wallets, a currency (by default the budget currency). Amounts
are converted at the rate on the date of each transaction.

The **"Export CSV"** button saves what is on the screen; where there are several reports,
it opens a menu. The file opens in Excel and LibreOffice without setting up
the encoding.

### Find a transaction

**Transactions → List** and a wallet's history are filtered by kind, contact,
period, category and text (comment, category, asset, wallet, contact,
hash).
The shared list also has a wallet choice — with sub-accounts or without — and
**"Hide internal"**: transfers between sub-accounts of one wallet disappear.
A click on a row opens the transaction panel.

### Quick entry from the keyboard

Settings → **"Quick entry"**: press the desired combination in the field,
for example Ctrl+Shift+Space, and "Save". The combination opens "+ Transaction"
even from another program. Off by default: on Hyprland and other
tiling managers many combinations are already taken. If the combination is taken or
global keys are unavailable in the system (some Wayland environments), a
message appears — choose another one.

## Calendar

**Transactions → Calendar**: spending by days of the month. The color of a day compares
everyday spending with the daily budget: green — within it, yellow — almost all of it,
red — overspending. A click on a day shows its transactions (a click on a
transaction opens its panel); on the right —
upcoming expenses and receipts. A mini calendar and "Last 7 days" are also on the
Dashboard.

## Inflation

Settings → **"Inflation (reference)"**: annual inflation by currency. It is
needed only for analytics — the "In today's money" switch in the
**What I have** section and the checkbox in the "Income and spending" filter. The default figures
are approximate — adjust them to yourself. Inflation does not affect the amounts in
wallets.

## Rates

**Settings → Money → Rates**: all currencies against the base one — how much of the base currency a unit of
the currency costs and how much of it per unit of the base, when the rate was updated. Currencies
you use are in bold: a rate history is stored for them. On the right is a
**converter** between any two currencies, at today's rate or on a chosen
date.

## Losses and chains: RUB → USDT → THB via skycapital

An example: rubles are exchanged for USDT at an exchanger, USDT for baht.

1. Create the contact **skycapital** (Transactions → Contacts and debts, kind "exchanger").
2. On the ruble card: **Convert** — you give 10,000 RUB from the card, you get 98 USDT
   into a wallet, the contact is skycapital. The deal rate can be entered by hand — it
   is saved as is.
3. Open the transaction in the list (click the row) → **"Continue the chain"**: a new
   transaction opens from the wallet where the USDT arrived and joins the same chain.
   Exchange 98 USDT for 3,400 THB.

A transaction's **loss** = the fee + the difference between what was given and received at the
**market** rate at the moment of the transaction. If the market gave 100 USD for 10,000 RUB
and you got 98 USDT, the loss is 2 USD. An exchange between your own sub-accounts is not spending,
but its fee and rate difference are still counted as a loss.

- In the transaction list a row with a loss shows "loss N".
- The transaction panel: amounts, deal rate, fee, loss, chain; the buttons
  "Continue the chain", "Link with…" (add already recorded transactions),
  "Remove from chain".
- **Transactions → Losses and chains**: losses for the year by transaction kind, wallet and
  contact; "gave → got" chains with the loss of each link.
- **Analytics → Losses** — the same by month, with the shared filter and CSV.

## Blockchain look-up

If a transaction has a transaction hash, the app can find its network fee and status
in a public explorer. This is **off** until you
allow it: the question appears at the first transaction with a hash, the switch is in
Settings. Only the hash and the network are sent. What is found is shown in the transaction
panel: **"Apply"** adds the fee if there was none, **"Dismiss"**
leaves the transaction as it is.

## Utilities

Utilities live **inside property**: add a flat or a house (a "Property"
wallet), open its settings → **"Utilities"**. This is also where the property summary is:
value, mortgage debt (a loan secured by this property), your share and
monthly costs. Three kinds of bills:

- **By meter** (electricity, water, gas): a unit and a tariff. Once a month you enter a
  reading — the amount = (reading − previous reading) × tariff. A meter only
  grows: the app will not accept a reading lower than the previous one.
- **Fixed** (internet, phone): the amount as last time, at the first
  payment — the given one.
- **Rent**: the amount per month, the payment day and a deposit — it is added to the first
  payment. If an automation pays the rent, link it so the payment is not
  counted twice in the plan.

The card shows the monthly amount, the change against the previous bill ▲/▼, the 6-month average,
the forecast for next month and an **"unusually high"** mark if a bill is
more than one and a half times the average. **"Record the payment"** opens the "Pay" form
with the amount, category and bill name already in the comment —
you only have to check and save. The **"Fixed costs"** widget on the Dashboard
shows the forecast for each bill; a row leads to the Utilities of its property.

## Questions and answers

**Why does "Record the payment" for a meter say "no reading"?** First enter the
reading for this month: the amount is computed from the difference with the previous one.

**Why is the trend of a fixed bill empty?** It is built from payments made
through "Record the payment" (they are recognized by the bill name in the comment and the category).
Payments recorded by hand with a different comment are not in the trend.

**Rent is already in the automations — will it be counted twice?** No, if this automation is
selected in the rent card.

**Why does an old vault not open?** The data schema changed once on the
move to the new transaction model; old files are not converted.

## Dashboard, settings and hints

- **The dashboard** is a grid of widgets. Drag a widget by its title, resize it by the
  bottom right corner; empty spaces fill themselves. "Hide title"
  pins a widget — it neither moves nor resizes. The round **"+"** button
  opens the catalog: a description and a live preview of each widget not yet added.
- **Settings** open as a window over the page: sections on the left, subsections under the
  active section — hovering or clicking scrolls to them.
- **What will happen** — the transaction form shows how the balances of the
  affected wallets will change (was → will be), where the transaction lands in the budget, and
  warns about a negative balance, a credit card limit or spending from envelopes.
- **"?"** in the header shows the onboarding of the current screen; on the first sign-in it
  starts by itself. "Show again" — Settings → General → Tours.
- **Right click** on a wallet, widget, contact, transaction, coin or a chart in the
  terminal — quick actions. Dangerous items are red and last, unavailable
  ones show the reason.
