# 💱 XLM Local Swap

> A Soroban smart contract on the Stellar blockchain that lets anyone swap **XLM for local currency tokens** at rates set transparently on-chain — no hidden spreads, no middlemen.

---

## 📖 Project Description

**XLM Local Swap** bridges the Stellar network to local economies. In many emerging markets, people need to convert XLM into digital representations of their local currency (INR, NGN, BRL, KES, etc.) to pay for goods, send to family, or interact with local DeFi apps.

Traditional off-ramps are opaque — fees are buried, exchange rates are manipulated in real time, and users have no way to verify what they actually received. This contract solves that by making every variable **public, auditable, and immutable per transaction**:

- The exchange rate is stored on-chain and emitted in every swap event.
- Fees are expressed in plain basis points — no fine print.
- Any wallet on Stellar can call `quote()` before committing a single stroop.

This contract is built with [Soroban](https://soroban.stellar.org/), Stellar's smart contract platform, and is deployable to Testnet in minutes.

---

## ⚙️ What It Does

### User Flow

```
User Wallet                   XLM Local Swap Contract            Local Token Contract
     │                                  │                                │
     │── approve XLM transfer ─────────▶│                                │
     │── call swap_xlm_to_local() ──────▶│                                │
     │                                  │── pull XLM from user ─────────▶│
     │                                  │── calculate net output          │
     │                                  │── push local tokens ───────────▶│
     │◀─────────────────────── local tokens land in wallet ───────────────│
```

1. **Call `quote(xlm_amount)`** — get the exact output before spending anything.
2. **Approve** the contract to spend your XLM (standard token approval).
3. **Call `swap_xlm_to_local(user, xlm_amount, min_out)`** — tokens arrive atomically.
4. If the output would fall below `min_out` (slippage guard), the transaction reverts.

### Rate Calculation

```
gross_local = xlm_amount × rate / 1_000_000
fee_amount  = gross_local × fee_bps / 10_000
net_local   = gross_local − fee_amount
```

Example — 1 XLM = 83.5 INR, fee = 0.30%:
| Input | Gross | Fee (0.30%) | **Net** |
|-------|-------|-------------|---------|
| 10 XLM | 835 INR | 2.505 INR | **832.495 INR** |
| 100 XLM | 8,350 INR | 25.05 INR | **8,324.95 INR** |

---

## ✨ Features

### 🔍 Full Transparency
Every swap emits a `SwapEvent` containing the user address, XLM in, local tokens out, the exact rate applied, and the fee charged. Nothing is hidden.

### 📊 Pre-Swap Quote
The `quote(xlm_amount)` view function returns `(gross, fee, net)` without spending gas or submitting a transaction. Frontends can display exact amounts before the user signs anything.

### 🛡️ Slippage Protection
The `min_out` parameter protects users from rate changes between quote time and execution time. If the actual output falls below `min_out`, the transaction reverts entirely — no partial fills.

### ⚡ Atomic Swaps
Both token transfers (XLM in, local tokens out) happen in a single Soroban invocation. There is no window where one side of the trade can succeed while the other fails.

### 🔒 Pause Switch
The admin can pause the contract instantly in response to oracle errors, rate manipulation attempts, or emergency situations. Swaps revert with a clear `"contract is paused"` error.

### 💰 Configurable Fee
Fees are set in **basis points** (1 bps = 0.01%). The maximum fee is hard-capped at **500 bps (5%)** in the contract code — the admin cannot set a predatory fee without redeploying.

### 🏦 Admin Fee Withdrawal
Accumulated XLM fees stay inside the contract and can only be withdrawn by the admin via `withdraw_xlm(amount)`. The admin key can be transferred or rotated using `transfer_admin()`.

### 🌍 Multi-Currency Ready
The `currency_label` field (e.g. `"INR"`, `"NGN"`, `"BRL"`) is stored on-chain and readable by frontends. Deploy one instance per local currency with its own rate feed.

### 📈 Lifetime Analytics
`total_swapped()` returns the cumulative XLM volume processed by the contract — useful for dashboards, liquidity planning, and audits.

---

## 🚀 Quick Start

### Prerequisites

```bash
# Install Rust
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# Add WASM target
rustup target add wasm32-unknown-unknown

# Install Stellar CLI
cargo install --locked stellar-cli --features opt
```

### Build

```bash
git clone https://github.com/yourname/xlm-local-swap
cd xlm-local-swap

stellar contract build
# Output: target/wasm32-unknown-unknown/release/xlm_local_swap.wasm
```

### Run Tests

```bash
cargo test --features testutils
```

### Deploy to Testnet

```bash
# Get a funded testnet account
stellar keys generate --global alice --network testnet --fund

# Deploy
stellar contract deploy \
  --wasm target/wasm32-unknown-unknown/release/xlm_local_swap.wasm \
  --source alice \
  --network testnet
```

### Initialize

```bash
stellar contract invoke \
  --id <CONTRACT_ID> \
  --source alice \
  --network testnet \
  -- initialize \
  --admin <ADMIN_ADDRESS> \
  --xlm_token <WRAPPED_XLM_ADDRESS> \
  --local_token <LOCAL_TOKEN_ADDRESS> \
  --rate 83500000 \
  --fee_bps 30 \
  --currency_label "INR"
```

---

## 📡 Contract Interface

| Function | Who | Description |
|---|---|---|
| `initialize(...)` | Admin (once) | Deploy and configure the contract |
| `swap_xlm_to_local(user, xlm_amount, min_out)` | Any user | Swap XLM → local tokens |
| `quote(xlm_amount)` | Anyone (view) | Preview output with no gas cost |
| `get_rate()` | Anyone (view) | Current exchange rate |
| `get_fee()` | Anyone (view) | Current fee in basis points |
| `total_swapped()` | Anyone (view) | Lifetime XLM volume |
| `currency_label()` | Anyone (view) | Local currency symbol |
| `is_paused()` | Anyone (view) | Contract pause status |
| `set_rate(new_rate)` | Admin | Update the exchange rate |
| `set_fee(new_fee_bps)` | Admin | Update the fee |
| `set_paused(paused)` | Admin | Pause or unpause swaps |
| `withdraw_xlm(amount)` | Admin | Withdraw XLM fees |
| `transfer_admin(new_admin)` | Admin | Rotate the admin key |

---

## 🗺️ Roadmap

- [ ] Reverse swap: local tokens → XLM
- [ ] Oracle integration (Reflector / Band Protocol) for auto-updating rates
- [ ] Multi-hop: XLM → USDC → local token
- [ ] Rate history stored in contract ledger entries
- [ ] Frontend dApp (React + Freighter wallet)

---

## 📄 License

MIT © 2025. See [LICENSE](LICENSE) for details.

---

## ⚠️ Disclaimer

This contract is provided for educational and development purposes. It has not been audited. Do not deploy to Mainnet with real funds without a professional security audit.



wallet address: GCHJJVDBXFGCWBITER4NWGXPVRZ6WOZ5PRN46ZRSJVLUAX3S3CV25KXG 

contract address: CAAKRNN6FBEM64KAIFESC7LQY2ZLOTVMKKOO23ZZ3IHSJCLN2BCD7CZY

https://stellar.expert/explorer/testnet/contract/CAAKRNN6FBEM64KAIFESC7LQY2ZLOTVMKKOO23ZZ3IHSJCLN2BCD7CZY

<img width="1280" height="713" alt="image" src="https://github.com/user-attachments/assets/0c6ec567-4694-43a3-a39a-483f3acc2d22" />
