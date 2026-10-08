"""Demo data for the Frappe Books Flatpak.

Run it inside the app sandbox against the local site (see
scripts/load-demo-data.sh):

    bench --site site1 console < demo-data.py

It sets up a company (Acme Inc, USD, US) with the standard chart of accounts,
then seeds items, customers, suppliers, six months of sales and purchase
invoices, and payments for the older ones.
"""

import random
from datetime import timedelta

import frappe
from frappe.utils import getdate, nowdate

from frappe_books.coa import STANDARD_CHART
from frappe_books.setup_service import run_setup

frappe.flags.ignore_permissions = True


def gv(doctype, **filters):
    return frappe.db.get_value(doctype, filters, "name")


# --- Company ---------------------------------------------------------------
if not frappe.db.get_single_value("Books Accounting Settings", "setup_complete"):
    wizard = frappe.get_doc(
        {
            "doctype": "Books Setup Wizard",
            "company_name": "Acme Inc",
            "fullname": "Ada Lovelace",
            "email": "hello@acme.test",
            "country": "United States",
            "currency": "USD",
            "time_zone": "America/New_York",
            "bank_name": "First National Bank",
            "chart_of_accounts": STANDARD_CHART,
            "fiscal_year_start": "2026-01-01",
            "fiscal_year_end": "2026-12-31",
        }
    )
    run_setup(wizard)
    print("company set up")

receivable = gv("Books Account", account_type="Receivable", is_group=0)
payable = gv("Books Account", account_type="Payable", is_group=0)
income = "Sales" if frappe.db.exists("Books Account", "Sales") else gv(
    "Books Account", root_type="Income", is_group=0
)
expense = (
    "Cost of Goods Sold"
    if frappe.db.exists("Books Account", "Cost of Goods Sold")
    else gv("Books Account", root_type="Expense", is_group=0)
)
bank = gv("Books Account", account_type="Bank", is_group=0)
print("accounts:", receivable, payable, income, expense, bank)

# --- Items -----------------------------------------------------------------
ITEMS = [
    ("Brand Sprint", "SRV-BRAND", "Service", 4800),
    ("UX Audit", "SRV-UX", "Service", 2400),
    ("Website Design", "SRV-WEB", "Service", 6500),
    ("Design System", "PRD-DSYS", "Product", 3200),
    ("Logo Package", "PRD-LOGO", "Product", 1500),
    ("Monthly Support", "SRV-SUPPORT", "Service", 900),
]
for name, code, itype, rate in ITEMS:
    if not frappe.db.exists("Books Item", name):
        frappe.get_doc(
            {
                "doctype": "Books Item",
                "name": name,
                "item_code": code,
                "item_usage": "Both",
                "item_type": itype,
                "unit": "Unit",
                "rate": rate,
                "income_account": income,
                "expense_account": expense,
            }
        ).insert()

CUSTOMERS = ["Northwind Traders", "Contoso Ltd", "Fabrikam Inc", "Adventure Works", "Litware"]
SUPPLIERS = ["CloudHost Inc", "Paper & Pixel", "Freelance Collective"]
for name in CUSTOMERS:
    if not frappe.db.exists("Books Party", name):
        frappe.get_doc(
            {"doctype": "Books Party", "name": name, "role": "Customer", "default_account": receivable}
        ).insert()
for name in SUPPLIERS:
    if not frappe.db.exists("Books Party", name):
        frappe.get_doc(
            {"doctype": "Books Party", "name": name, "role": "Supplier", "default_account": payable}
        ).insert()
print("items and parties created")

# --- Invoices over six months ----------------------------------------------
random.seed(7)
today = getdate(nowdate())


def create_invoice(doctype, party, account, item, item_account, when, qty, rate):
    doc = frappe.get_doc(
        {
            "doctype": doctype,
            "party": party,
            "account": account,
            "date": when,
            "items": [
                {
                    "item": item,
                    "account": item_account,
                    "rate": rate,
                    "quantity": qty,
                    "unit_conversion_factor": 1,
                }
            ],
            "make_auto_payment": 0,
            "make_auto_stock_transfer": 0,
        }
    )
    doc.insert()
    doc.submit()
    return doc


invoices = []
for month in range(6):
    base = today - timedelta(days=30 * month)
    for _ in range(4):
        day = base - timedelta(days=random.randint(0, 25))
        name, _code, _t, rate = random.choice(ITEMS)
        cust = random.choice(CUSTOMERS)
        qty = random.choice([1, 1, 2, 3])
        inv = create_invoice("Books Sales Invoice", cust, receivable, name, income, day, qty, rate)
        invoices.append(("Books Sales Invoice", inv.name, inv.grand_total, cust))
    for _ in range(2):
        day = base - timedelta(days=random.randint(0, 25))
        name, _code, _t, rate = random.choice(ITEMS)
        sup = random.choice(SUPPLIERS)
        qty = random.choice([1, 2])
        inv = create_invoice(
            "Books Purchase Invoice", sup, payable, name, expense, day, qty, int(rate * 0.4)
        )
        invoices.append(("Books Purchase Invoice", inv.name, inv.grand_total, sup))
print("invoices:", len(invoices))

# --- Payments for the older invoices ---------------------------------------
paid_sales = 0
paid_purchases = 0
for dt, name, amount, party in invoices:
    if not amount:
        continue
    if dt == "Books Sales Invoice" and paid_sales < 16:
        receive = True
        paid_sales += 1
    elif dt == "Books Purchase Invoice" and paid_purchases < 6:
        receive = False
        paid_purchases += 1
    else:
        continue
    when = getdate(frappe.db.get_value(dt, name, "date")) + timedelta(days=random.randint(3, 20))
    payment = frappe.get_doc(
        {
            "doctype": "Books Payment",
            "party": party,
            "date": when,
            "payment_type": "Receive" if receive else "Pay",
            "account": receivable if receive else payable,
            "payment_account": bank,
            "payment_method": "Bank",
            "reference_id": f"TRF-{2000 + paid_sales + paid_purchases}",
            "amount": amount,
            "payment_references": [
                {"reference_type": dt, "reference_name": name, "amount": amount}
            ],
        }
    )
    payment.insert()
    payment.submit()
print("payments:", paid_sales + paid_purchases)

frappe.db.commit()
print("SEED DONE")
